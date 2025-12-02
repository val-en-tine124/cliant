use std::io::SeekFrom;
use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex as TokioMutex;

use tokio::sync::mpsc;

use tokio_stream::{Stream,StreamExt, wrappers::ReceiverStream};
use tracing::{debug, error, instrument};

use crate::domain::models::file_info::FileInfo;
use crate::domain::ports::storage_service::{DirIO,SeekAndWrite,FileStatus,CreateDelete,ReadWrite};

const MAX_HANDLE_CACHE: usize = 64;

// A concrete implementation of `FileSystemIO` for interacting with the disk.
#[allow(unused)]
pub struct DiskFileSystem{
    handle_cache: HashMap<PathBuf, Arc<TokioMutex<File>>>,
}

#[allow(unused)]
impl DiskFileSystem {
    pub fn new() -> Self {
        let handle_cache=HashMap::new();
        Self{handle_cache}
    }
}

enum WriteMode{
    Write,
    Append,
    Create,
}

impl DiskFileSystem{
    async fn buf_writer_handle(&mut self, path: &Path, mode: WriteMode) -> Result<Arc<TokioMutex<File>>> {
        if !self.handle_cache.contains_key(path) {
            let mut options = OpenOptions::new();
            match mode {
                WriteMode::Write => { options.read(true).write(true); }
                WriteMode::Append => { options.read(true).append(true); }
                WriteMode::Create => { options.read(true).create(true).write(true); }
            }
            let handle = options.open(path).await?;
            // Evict if cache is full
            if self.handle_cache.len() >= MAX_HANDLE_CACHE {
                if let Some(old_key) = self.handle_cache.keys().next().cloned() {
                    self.handle_cache.remove(&old_key);
                }
            }
            let arc = Arc::new(TokioMutex::new(handle));
            self.handle_cache.insert(path.to_path_buf(), arc.clone());
            return Ok(arc);
        }

        Ok(self.handle_cache.get(path).unwrap().clone())
    }

    async fn buf_reader_handle(&mut self, path: &Path) -> Result<Arc<TokioMutex<File>>> {
        if !self.handle_cache.contains_key(path) {
            let handle = OpenOptions::new().read(true).open(path).await?;
            // Evict if cache is full
            if self.handle_cache.len() >= MAX_HANDLE_CACHE {
                if let Some(old_key) = self.handle_cache.keys().next().cloned() {
                    self.handle_cache.remove(&old_key);
                }
            }
            let arc = Arc::new(TokioMutex::new(handle));
            self.handle_cache.insert(path.to_path_buf(), arc.clone());
            return Ok(arc);
        }

        Ok(self.handle_cache.get(path).unwrap().clone())
    }
   
}

impl ReadWrite for DiskFileSystem {
     /// Read a file as an async stream of `Bytes`.
    /// If a cached handle exists it will be used (moved into the spawned task),
    /// otherwise the file is opened inside the spawned task.
    fn read_file(&mut self, path: PathBuf) -> impl Stream<Item = Result<Bytes>> {
        debug!(name:"stream_channel","Initializing stream channel for reading file:{} .",path.display());
        let (tx, rx) = mpsc::channel::<Result<Bytes>>(1024);
        let path_for_fut = path.to_path_buf(); // owned for async task
        // If we already have a cached handle, clone and use it; otherwise open inside the task (no cache insert).
        let file_arc_opt = self.handle_cache.get(&path_for_fut).cloned();

        if let Some(file_arc) = file_arc_opt {
            let read_file_stream = async move {
                debug!(name:"read_file_handle","Initializing file read handle for path:{}",path_for_fut.display());
                let mut buffer = [0u8; 2048];
                debug!(name:"read_to_buffer","Reading file content to buffer...");
                // Hold the lock while performing buffered reads using BufReader
                let mut guard = file_arc.lock().await;
                // ensure we start reading from the beginning of the file
                let _ = guard.seek(SeekFrom::Start(0)).await;
                let mut reader = BufReader::new(&mut *guard);
                loop {
                    match reader.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if tx.send(Ok(Bytes::copy_from_slice(&buffer[..n]))).await.is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e.into())).await;
                            return;
                        }
                    }
                }
                debug!(name:"read_to_buffer","Reading file content to buffer completed.");
            };

            tokio::spawn(read_file_stream);
        } else {
            // No cached handle: open the file inside the spawned task and stream from it (won't populate cache).
            let read_file_stream = async move {
                debug!(name:"read_file_handle","Opening file for read: {}",path_for_fut.display());
                match OpenOptions::new().read(true).open(path_for_fut).await {
                    Ok(file) => {
                        let mut buffer = [0u8; 2048];
                        let mut reader = BufReader::new(file);
                        loop {
                            match reader.read(&mut buffer).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    if tx.send(Ok(Bytes::copy_from_slice(&buffer[..n]))).await.is_err() {
                                        return;
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(e.into())).await;
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.into())).await;
                    }
                }
            };

            tokio::spawn(read_file_stream);
        }
        ReceiverStream::new(rx)
    }

    ///Asynchronous method for appending a buffer to a file.
    #[instrument(name="disk_fs_append_to_file",skip(self, content),fields(path=path.to_str()))]
    async fn append_to_file(&mut self, content: &[u8], path: &Path) -> Result<()> {
        debug!(name:"append_file_handle","Initializing file append handle for path {}.",path.display());
        let file_arc = self.buf_writer_handle(path, WriteMode::Append).await?;
        let mut handle = file_arc.lock().await;
        debug!(name:"append_file_handle","Writing bytes to file handle {}.",path.display());
        let mut writer = BufWriter::new(&mut *handle);
        writer.write_all(content).await?;
        writer.flush().await?;
        Ok(())
    }
    /// Asynchronous method to write a buffer content to a file
    #[instrument(name="disk_fs_write_file",skip(self, content),fields(path=path.to_str()))]
    async fn write_file(&mut self, content: &[u8], path: &Path) -> Result<()> {
        debug!(name:"create_file_handle","Initializing create file handle:{}.",path.display());
        let file_arc = self.buf_writer_handle(path, WriteMode::Append).await?;
        let mut handle = file_arc.lock().await;
        let mut writer = BufWriter::new(&mut *handle);
        writer.write_all(content).await?;
        writer.flush().await?;
        Ok(())
    }

}
impl SeekAndWrite for DiskFileSystem{
    #[instrument(name="disk_fs_file_info",skip(self,buf),fields(path=path.to_str(),pos=pos,))]
    async fn write_at(&mut self,path:&Path,pos:usize,buf:&[u8])->Result<()> {
        debug!(name:"create_file_handle","Initializing create file handle:{}.",path.display());
        let file_arc = self.buf_writer_handle(path, WriteMode::Write).await?;
        let mut handle = file_arc.lock().await;
        handle.seek(SeekFrom::Start(pos as u64)).await?; // This will set the offset to the provided number of bytes.Consider SeekFrom::End for broken download.
        let mut writer = BufWriter::new(&mut *handle);
        writer.write_all(buf).await?;
        writer.flush().await?;
        Ok(())
    }

    ///This will preallocate the file length.make sure that each parent directory exist, this method will not create a parent directory.
    #[instrument(name="set_file_length",skip(self),fields(path=path.to_str(),file_size=file_size))]
    async fn set_len(&mut self, path: &Path, file_size: usize) -> Result<()> {
        debug!(name:"create_file_handle","Initializing create file handle:{}.",path.display());
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }
        let handle=OpenOptions::new().truncate(true).write(true).open(path).await?;
        handle.set_len(file_size as u64).await?;
        return Ok(());
        
    }

}
 impl CreateDelete for DiskFileSystem{
    ///Asynchronous method to create a file.
    #[instrument(name="disk_fs_create_file",skip(self),fields(path=path.to_str()))]
    async fn create_file(&mut self, path: &Path) -> Result<()> {
        debug!(name:"create_file_handle","Initializing create file handle:{}.",path.display());
        let _ = self.buf_writer_handle(path,WriteMode::Create).await?;
        Ok(())
    }

     ///Asynchronous method to remove a file.
    #[instrument(name="disk_fs_remove_file",skip(self),fields(path=path.to_str()))]
    async fn remove_file(&mut self, path: &Path) -> Result<()> {
        debug!(name:"remove_file","Removing file:{}",path.display());
        fs::remove_file(path).await?;
        Ok(())
    }
 }   
impl FileStatus for DiskFileSystem{
    ///Asynchronous method to check if a file exists.
    #[instrument(name="disk_fs_file_exists",skip(self),fields(path=path.to_str()))]
    async fn file_exists(&mut self, path: &Path) -> Result<bool> {
        debug!(name:"check_file_exists","Checking existence of file :{}",path.display());
        Ok(fs::try_exists(path).await?)
    }

   

    ///Asynchronous method to get a file info.
    #[instrument(name="disk_fs_file_info",skip(self),fields(path=path.to_str()))]
    async fn file_info(&mut self, path: &Path) -> Result<FileInfo> {
        debug!(name:"file_metadata","Fetching file info:{}",path.display());
        let metadata = fs::metadata(path)
            .await
            .context(format!("Can't get file metadata:{}",path.display()))?;
        let size = metadata.len() as usize;
        let os_str_name = path.file_name().unwrap_or_default();
        let str_name = os_str_name.to_string_lossy();
        Ok(FileInfo::new(
            path.to_path_buf(),
            size,
            str_name.to_string(),
        ))
    }
}
    

    
    
impl DirIO for DiskFileSystem {
    ///Asynchronous method to recursively create directories.
    #[instrument(name="disk_fs_create_dir_all",skip(self),fields(path=path.to_str()))]
    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        debug!(name:"recursive_directory_creation","Creating directories along path:{} recursively",path.display());
        fs::create_dir_all(path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                anyhow!("Can't create directory {:?}, permission denied.", path.display())
            }
            std::io::ErrorKind::NotFound => anyhow!("Directory not found."),
            _ => {
                let err_string = e.to_string();
                anyhow!(format!("Unknown Error occurred: {err_string}"))
            }
        })?;
        Ok(())
    }
    ///Asynchronous method to recursively remove directories.
    #[instrument(name="disk_fs_remove_dir_all",skip(self),fields(path=path.to_str()))]
    async fn remove_dir_all(&self, path: &Path) -> Result<()> {
        debug!(name:"recursive_directory_removal","Removing directories along path:{} recursively",path.display());
        fs::remove_dir_all(path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => anyhow!("Directory not found."),
            std::io::ErrorKind::NotADirectory => anyhow!(format!("Path {:?} is not a dir.", path.display())),
            std::io::ErrorKind::PermissionDenied => anyhow!(format!(
                "Can't remove directory {:?} recursively, permission denied.",
                path.display()
            )),
            _ => {
                let err_string = e.to_string();
                anyhow!("Unknown Error occurred: {err_string}")
            }
        })
    }

    ///Asynchronous method to check if a path exists.
    #[instrument(name="disk_fs_dir_exists",skip(self),fields(path=path.to_str()))]
    async fn dir_exists(&self, path: &Path) -> Result<bool> {
        debug!(name:"dir_exists","checking existence of directory path:{}",path.display());
        let exists = fs::try_exists(path).await?;
        Ok(exists)
    }
}

#[tokio::test]
async fn test_read_file() -> Result<()> {
    use tracing::info;
    use tracing_subscriber::EnvFilter;
    EnvFilter::from_default_env();

    let path = Path::new("/home/val").join("acpi_logs_2.txt");
    let mut disk_fs = DiskFileSystem::new();
    if let Ok(exists) = disk_fs.file_exists(&path).await {
        if exists {
            let mut file_stream = disk_fs.read_file(path.to_path_buf());
            while let Some(Ok(stream)) = file_stream.next().await {
                let string = String::from_utf8_lossy(&stream).into_owned();
                info!("-- {:?}", string);
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_get_info() -> Result<()> {
    let path = Path::new("test_file.txt");

    let mut disk_fs = DiskFileSystem::new();

    let _ = disk_fs.create_file(path).await?;
    let file_info: FileInfo = disk_fs.file_info(path).await?;

    let (name, path, size) = (
        file_info.file_name(),
        file_info.path(),
        file_info.file_size(),
    );
    println!("name: {name},path: {:?}, size: {size}", path.display());

    disk_fs.remove_file(path).await?;
    Ok(())
}

#[tokio::test]
async fn test_concurrent_readers() -> Result<()> {
    use tempfile::tempdir;
    use tracing::info;

    let dir = tempdir()?;
    let path = dir.path().join("concurrent.bin");

    let mut disk_fs = DiskFileSystem::new();
    let _ = disk_fs.create_file(&path).await?;

    // Write known data
    let data = vec![0xABu8; 8 * 1024];
    disk_fs.write_file(&data, &path).await?;

    // Create two streams (this will spawn background tasks to read)
    let mut s1 = disk_fs.read_file(path.to_path_buf());
    let mut s2 = disk_fs.read_file(path.to_path_buf());

    // Spawn tasks that consume the streams and return total bytes read
    let h1 = tokio::spawn(async move {
        let mut acc: Vec<u8> = Vec::new();
        while let Some(Ok(chunk)) = s1.next().await {
            acc.extend_from_slice(&chunk);
        }
        acc.len()
    });

    let h2 = tokio::spawn(async move {
        let mut acc: Vec<u8> = Vec::new();
        while let Some(Ok(chunk)) = s2.next().await {
            acc.extend_from_slice(&chunk);
        }
        acc.len()
    });

    let r1 = h1.await?;
    let r2 = h2.await?;

    info!("read sizes: {} {}", r1, r2);
    assert_eq!(r1, data.len());
    assert_eq!(r2, data.len());

    Ok(())
}
