use std::os::windows::fs::MetadataExt;
use std::path::Path;
use bytes::{Bytes, BytesMut};
use tokio::fs::{self, File};
use tokio::io::{AsyncWriteExt,AsyncReadExt};
use tokio_stream::StreamExt;

use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream,Stream};

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
    async fn append_to_file(&self,content:&[u8],path:&Path)->Result<(),DomainError> {
        let mut handle=File::options().append(true).write(true).open(path).await.map_err(|e|DomainError::StorageError(e.to_string()))?;
        handle.write_all(&content).await.map_err(|e|DomainError::StorageError(e.to_string()))?;
        Ok(())
    }
    
    ///synchronous method to read the content of a file as continuous streams.
    fn read_file(&self,path:&Path)->impl Stream<Item=Result<Bytes,DomainError>>{
        
        let( tx,rx)=mpsc::channel::<Result<Bytes,DomainError>>(1024);
        let path_for_fut=path.to_path_buf(); // made this cause read_file_stream has to own it own data.
       let read_file_stream=async move { //open a file handle
        let handle=fs::OpenOptions::new().read(true).open(path_for_fut).await.map_err(|e|DomainError::StorageError(e.to_string()));
            match handle{
                Ok(mut file)=>{
                    let mut buffer = BytesMut::with_capacity(1024);
                    while let Ok(bytes_read) =file.read_exact(&mut buffer).await{ 
                        buffer.truncate(bytes_read);
                    let _=tx.send(Ok(buffer.clone().freeze())).await.map_err(|e|DomainError::StorageError(e.to_string()));
                    
                    }
                }

                Err(error)=>{
                    eprintln!("{}",format!("Error {} occurred while making file handle at line {} at module ",error,line!(),));
                }
            }
            
        };

        tokio::spawn(read_file_stream);
        ReceiverStream::new(rx)
        

    }
    /// Asynchronous method to write a buffer content to a file
    async fn write_file(&self,content:&[u8],path:&Path)->Result<(),DomainError> {
        fs::write(path, content).await.map_err(|e|DomainError::StorageError(e.to_string()))
    }

    ///Asynchronous method to create a file.
    async fn create_file(&self, path: &Path) -> Result<(),DomainError> {
        File::create(path).await.map_err(|e|DomainError::StorageError(e.to_string()))?;
        Ok(())

    }

    ///Asynchronous method to check if a file exists.
    async fn file_exists(&self,path:&Path)->Result<bool,DomainError>{
        fs::try_exists(path).await.map_err(|e|DomainError::StorageError(e.to_string()))

    }
    
    ///Asynchronous method to remove a file.
    async fn remove_file(&self, path: &Path) -> Result<(),DomainError> {
        fs::remove_file(path).await.map_err(|e|DomainError::StorageError(e.to_string()))
    }
    
    ///Asynchronous method to get a file info.
    async fn file_info<'a>(&'a self,path:&'a Path)->Result<FileInfo<'a>,DomainError> {
        let metadata=fs::metadata(path).await.map_err(|e|DomainError::StorageError(format!("Can't get file metadata:{}",e.to_string())))?;
        let size=metadata.file_size() as usize;
        let os_str_name=path.file_name().unwrap_or_default();
        let str_name=os_str_name.to_string_lossy().into_owned();
        Ok(FileInfo::new(path, size, str_name))
    }
    

    

}

impl DirIO for DiskFileSystem{
    ///Asynchronous method to recursively create directories.
    async fn create_dir_all(&self, path: &Path) -> Result<(),DomainError> {
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
    async fn remove_dir_all(&self, path: &Path) -> Result<(),DomainError> {
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
    async fn dir_exists(&self,path:&Path)->Result<bool,DomainError> {
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
                println!("-- {:?}",string);
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
            println!("name: {},path: {:?}, size: {}",name,path,size);
            
            
        },
        Err(err)=>{
            println!("can't get file info: {}",err.to_string());
        }
    }
    

}