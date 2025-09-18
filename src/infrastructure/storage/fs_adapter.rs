use std::os::windows::fs::MetadataExt;
use std::path::Path;
use bytes::{Bytes, BytesMut};
use tokio::fs::{self, File};
use tokio::io::{AsyncWriteExt,AsyncReadExt};

use tokio::sync::mpsc;

use tokio_stream::{wrappers::ReceiverStream,StreamExt,Stream};
use tracing::{debug,error,info,instrument};

use crate::domain::errors::DomainError;
use crate::domain::ports::storage_service::{FileIO,DirIO};
use crate::domain::models::file_info::FileInfo;

// A concrete implementation of `FileSystemIO` for interacting with the disk.
#[derive(Clone)]
pub struct DiskFileSystem;

impl DiskFileSystem {
    pub fn new()->Self{
        Self
    }
}

impl FileIO for DiskFileSystem {
    ///Asynchronous method for appending a buffer to a file.
    #[instrument(name="disk_fs_append_to_file",skip(self, content),fields(path=path.to_str()))]
    async fn append_to_file(&self,content:&[u8],path:&Path)->Result<(),DomainError> {
        debug!(name:"append_file_handle","Initializing file append handle for path {}.",path.display());
        let mut handle=File::options().append(true).write(true).open(path).await.map_err(|e|DomainError::StorageError(e.to_string()))?;
        debug!(name:"append_file_handle","Writing bytes to file handle {}.",path.display());
        handle.write_all(&content).await.map_err(|e|DomainError::StorageError(e.to_string()))?;
        Ok(())
    }
    
    ///synchronous method to read the content of a file as continuous streams.
    #[instrument(name="disk_fs_read_file",skip(self),fields(path=path.to_str()))]
    fn read_file(&self,path:&Path)->impl Stream<Item=Result<Bytes,DomainError>>{
        debug!(name:"stream_channel","Initializing stream channel for reading file:{} .",path.display());
        let( tx,rx)=mpsc::channel::<Result<Bytes,DomainError>>(1024);
        let path_for_fut=path.to_path_buf(); // made this cause read_file_stream has to own it own data.
       let read_file_stream=async move { //open a file handle
        debug!(name:"read_file_handle","Initializing file read handle for path:{}",&path_for_fut.display());
        let handle=fs::OpenOptions::new().read(true).open(path_for_fut).await.map_err(|e|DomainError::StorageError(e.to_string()));
            match handle{
                Ok(mut file)=>{
                    let mut buffer = BytesMut::with_capacity(1024);
                    debug!(name:"read_to_buffer","Reading file content to buffer...");
                    while let Ok(bytes_read) =file.read_exact(&mut buffer).await{ 
                        buffer.truncate(bytes_read);
                    let _=tx.send(Ok(buffer.clone().freeze())).await.map_err(|e|DomainError::StorageError(e.to_string()));
                    
                    }
                    debug!(name:"read_to_buffer","Reading file content to buffer completed.");
                }

                Err(error)=>{
                    error!(error = %error, "Error occurred while making file handle");
                }
            }
            
        };

        tokio::spawn(read_file_stream);
        ReceiverStream::new(rx)
        

    }
    /// Asynchronous method to write a buffer content to a file
    #[instrument(name="disk_fs_write_file",skip(self, content),fields(path=path.to_str()))]
    async fn write_file(&self,content:&[u8],path:&Path)->Result<(),DomainError> {
        debug!(name:"write_file_handle","Initializing write file handle for path :{}.",path.display());
        fs::write(path, content).await.map_err(|e|DomainError::StorageError(e.to_string()))
    }

    ///Asynchronous method to create a file.
    #[instrument(name="disk_fs_create_file",skip(self),fields(path=path.to_str()))]
    async fn create_file(&self, path: &Path) -> Result<(),DomainError> {
        debug!(name:"create_file_handle","Initializing create file handle:{}.",path.display());
        File::create(path).await.map_err(|e|DomainError::StorageError(e.to_string()))?;
        Ok(())

    }

    ///Asynchronous method to check if a file exists.
    #[instrument(name="disk_fs_file_exists",skip(self),fields(path=path.to_str()))]
    async fn file_exists(&self,path:&Path)->Result<bool,DomainError>{
        debug!(name:"check_file_exists","Checking existence of file :{}",path.display());
        fs::try_exists(path).await.map_err(|e|DomainError::StorageError(e.to_string()))

    }
    
    ///Asynchronous method to remove a file.
    #[instrument(name="disk_fs_remove_file",skip(self),fields(path=path.to_str()))]
    async fn remove_file(&self, path: &Path) -> Result<(),DomainError> {
        debug!(name:"remove_file","Removing file:{}",path.display());
        fs::remove_file(path).await.map_err(|e|DomainError::StorageError(e.to_string()))
    }
    
    ///Asynchronous method to get a file info.
    #[instrument(name="disk_fs_file_info",skip(self),fields(path=path.to_str()))]
    async fn file_info<'a>(&'a self,path:&'a Path)->Result<FileInfo<'a>,DomainError> {
        debug!(name:"file_metadata","Fetching file info:{}",path.display());
        let metadata=fs::metadata(path).await.map_err(|e|DomainError::StorageError(format!("Can't get file metadata:{}",e.to_string())))?;
        let size=metadata.file_size() as usize;
        let os_str_name=path.file_name().unwrap_or_default();
        let str_name=os_str_name.to_string_lossy();
        Ok(FileInfo::new(path, size, str_name))
    }
    

    

}

impl DirIO for DiskFileSystem{
    ///Asynchronous method to recursively create directories.
    #[instrument(name="disk_fs_create_dir_all",skip(self),fields(path=path.to_str()))]
    async fn create_dir_all(&self, path: &Path) -> Result<(),DomainError> {
        debug!(name:"recursive_directory_creation","Creating directories along path:{} recursively",path.display());
        fs::create_dir_all(path).await.map_err(|e| match e.kind(){
            std::io::ErrorKind::PermissionDenied=> DomainError::StorageError(format!("Can't create directory {:?}, permission denied.",path)),
            std::io::ErrorKind::NotFound=>DomainError::StorageError("Directory not found.".into()),
            _=>{
                let err_string=e.to_string();
                DomainError::StorageError(format!("Unknown Error occurred: {}",err_string))
            }
        })
        
    }
    ///Asynchronous method to recursively remove directories.
    #[instrument(name="disk_fs_remove_dir_all",skip(self),fields(path=path.to_str()))]
    async fn remove_dir_all(&self, path: &Path) -> Result<(),DomainError> {
        debug!(name:"recursive_directory_removal","Removing directories along path:{} recursively",path.display());
        fs::remove_dir_all(path).await.map_err(|e|match e.kind(){
            std::io::ErrorKind::NotFound=>DomainError::StorageError ("Directory not found.".into()),
            std::io::ErrorKind::NotADirectory=>DomainError::StorageError (format!("Path {:?} is not a dir.",path)),
            std::io::ErrorKind::PermissionDenied=> DomainError::StorageError(format!("Can't remove directory {:?} recursively, permission denied.",path)),
            _=>{
                let err_string=e.to_string();
                DomainError::StorageError(format!("Unknown Error occurred: {}",err_string))
            }
        })
        
    }

    ///Asynchronous method to check if a path exists.
    #[instrument(name="disk_fs_dir_exists",skip(self),fields(path=path.to_str()))]
    async fn dir_exists(&self,path:&Path)->Result<bool,DomainError> {
        debug!(name:"dir_exists","checking existence of directory path:{}",path.display());
        fs::try_exists(path).await.map_err(|e|DomainError::StorageError(e.to_string()))
    }
}

#[tokio::test]
async fn test_read_file(){
    let path=Path::new(r"C:\Users\Admin\Documents\More on Hexagonal software architecture.txt");
    let disk_fs=DiskFileSystem::new();
    if let Ok(exists)=disk_fs.file_exists(path).await{
        if exists{
            let mut file_stream=disk_fs.read_file(path);
            while let Some(Ok(stream))=file_stream.next().await{
                let string=String::from_utf8_lossy(&stream).into_owned();
                info!("-- {:?}",string);
            }
        }
    }
    
}

#[tokio::test]
async fn test_get_info(){
    
    let path=Path::new(r"C:\Users\Admin\Documents\More on Hexagonal software architecture.txt");
    let disk_fs=DiskFileSystem::new();
    let info: Result<FileInfo, DomainError>=disk_fs.file_info(path).await;
    match info{
        Ok(file_info)=>{
            let (name,path,size)=(file_info.name(),file_info.path(),file_info.size());
            info!("name: {},path: {:?}, size: {}",name,path,size);
            
            
        },
        Err(err)=>{
            error!(error = %err, "can't get file info");
        }
    }
    

}