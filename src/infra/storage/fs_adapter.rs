use anyhow::{Context, Result, anyhow};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;

use bytes::{Bytes, BytesMut};
use std::path::Path;
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use tokio::sync::mpsc;

use tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream};
use tracing::{debug, error, instrument};

use crate::domain::models::file_info::FileInfo;
use crate::domain::ports::storage_service::{DirIO, FileIO};

// A concrete implementation of `FileSystemIO` for interacting with the disk.
#[allow(unused)]
#[derive(Clone)]
pub struct DiskFileSystem;

#[allow(unused)]
impl DiskFileSystem {
    pub fn new() -> Self {
        Self
    }
}

impl FileIO for DiskFileSystem {
    ///Asynchronous method for appending a buffer to a file.
    #[instrument(name="disk_fs_append_to_file",skip(self, content),fields(path=path.to_str()))]
    async fn append_to_file(&self, content: &[u8], path: &Path) -> Result<()> {
        debug!(name:"append_file_handle","Initializing file append handle for path {}.",path.display());
        let mut handle = File::options()
            .create(true)
            .append(true)
            .write(true)
            .open(path)
            .await?;
        debug!(name:"append_file_handle","Writing bytes to file handle {}.",path.display());
        handle.write_all(&content).await?;
        Ok(())
    }

    ///synchronous method to read the content of a file as continuous streams.
    #[instrument(name="disk_fs_read_file",skip(self),fields(path=path.to_str()))]
    fn read_file(&self, path: &Path) -> Result<impl Stream<Item = Result<Bytes>>> {
        debug!(name:"stream_channel","Initializing stream channel for reading file:{} .",path.display());
        let (tx, rx) = mpsc::channel::<Result<Bytes>>(1024);
        let path_for_fut = path.to_path_buf(); // made this cause read_file_stream has to own it own data.
        let read_file_stream = async move {
            //open a file handle
            debug!(name:"read_file_handle","Initializing file read handle for path:{}",&path_for_fut.display());
            let handle = fs::OpenOptions::new().read(true).open(path_for_fut).await;
            match handle {
                Ok(mut file) => {
                    let mut buffer = BytesMut::from(Bytes::from(vec![0u8; 1024]));

                    debug!(name:"read_to_buffer","Reading file content to buffer...");
                    while let Ok(bytes_read) = file.read(&mut buffer).await {
                        buffer.truncate(bytes_read);
                        if bytes_read == 0 {
                            break;
                        }
                        if let Err(err) = tx
                            .send(Ok(buffer.clone().freeze()))
                            .await
                            .context("Can't send buffer data to asynchronous sink.")
                        {
                            error!(error = %err,"Error occurred while sending bytes over channel.");
                            return Err(err);
                        }
                    }
                    debug!(name:"read_to_buffer","Reading file content to buffer completed.");
                }

                Err(error) => {
                    error!(error = %error, "Error occurred while making file handle");
                    return Err(error.into());
                }
            }
            Ok(())
        };

        tokio::spawn(read_file_stream);
        Ok(ReceiverStream::new(rx))
    }
    /// Asynchronous method to write a buffer content to a file
    #[instrument(name="disk_fs_write_file",skip(self, content),fields(path=path.to_str()))]
    async fn write_file(&self, content: &[u8], path: &Path) -> Result<()> {
        debug!(name:"write_file_handle","Initializing write file handle for path :{}.",path.display());
        let _ = fs::write(path, content).await?;
        Ok(())
    }

    ///Asynchronous method to create a file.
    #[instrument(name="disk_fs_create_file",skip(self),fields(path=path.to_str()))]
    async fn create_file(&self, path: &Path) -> Result<()> {
        debug!(name:"create_file_handle","Initializing create file handle:{}.",path.display());
        File::create(path).await?;
        Ok(())
    }

    ///This will preallocate the file length.make sure that each parent directory exist this method will not create a parent directory.
    async fn set_len(&self, path: &Path, file_size: usize) -> Result<()> {
        let file = File::options() //Remeber to add caching of file handles 
            .write(true)
            .create(true)
            .open(path)
            .await?;
        file.set_len(file_size as u64).await?;
        Ok(())
    }

    

    ///Asynchronous method to check if a file exists.
    #[instrument(name="disk_fs_file_exists",skip(self),fields(path=path.to_str()))]
    async fn file_exists(&self, path: &Path) -> Result<bool> {
        debug!(name:"check_file_exists","Checking existence of file :{}",path.display());
        Ok(fs::try_exists(path).await?)
    }

    ///Asynchronous method to remove a file.
    #[instrument(name="disk_fs_remove_file",skip(self),fields(path=path.to_str()))]
    async fn remove_file(&self, path: &Path) -> Result<()> {
        debug!(name:"remove_file","Removing file:{}",path.display());
        let _ = fs::remove_file(path).await;
        Ok(())
    }

    ///Asynchronous method to get a file info.
    #[instrument(name="disk_fs_file_info",skip(self),fields(path=path.to_str()))]
    async fn file_info<'a>(&'a self, path: &'a Path) -> Result<FileInfo> {
        debug!(name:"file_metadata","Fetching file info:{}",path.display());
        let metadata = fs::metadata(path)
            .await
            .context(format!("Can't get file metadata:{:?}", path))?;
        let size = metadata.size() as usize;
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
        let result = fs::create_dir_all(path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                anyhow!("Can't create directory {:?}, permission denied.", path)
            }
            std::io::ErrorKind::NotFound => anyhow!("Directory not found."),
            _ => {
                let err_string = e.to_string();
                anyhow!(format!("Unknown Error occurred: {}", err_string))
            }
        })?;
        Ok(result)
    }
    ///Asynchronous method to recursively remove directories.
    #[instrument(name="disk_fs_remove_dir_all",skip(self),fields(path=path.to_str()))]
    async fn remove_dir_all(&self, path: &Path) -> Result<()> {
        debug!(name:"recursive_directory_removal","Removing directories along path:{} recursively",path.display());
        fs::remove_dir_all(path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => anyhow!("Directory not found."),
            std::io::ErrorKind::NotADirectory => anyhow!(format!("Path {:?} is not a dir.", path)),
            std::io::ErrorKind::PermissionDenied => anyhow!(format!(
                "Can't remove directory {:?} recursively, permission denied.",
                path
            )),
            _ => {
                let err_string = e.to_string();
                anyhow!("Unknown Error occurred: {}", err_string)
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

    let path = Path::new(r"");
    let disk_fs = DiskFileSystem::new();
    if let Ok(exists) = disk_fs.file_exists(path).await {
        if exists {
            let mut file_stream = disk_fs.read_file(path)?;
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

    let disk_fs = DiskFileSystem::new();

    let _ = disk_fs.create_file(path).await?;
    let file_info: FileInfo = disk_fs.file_info(path).await?;

    let (name, path, size) = (
        file_info.file_name(),
        file_info.path(),
        file_info.file_size(),
    );
    println!("name: {},path: {:?}, size: {}", name, path, size);

    let _ = disk_fs.remove_file(path).await?;
    Ok(())
}
