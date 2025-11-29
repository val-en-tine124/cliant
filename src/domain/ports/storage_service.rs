use anyhow::Result;
use std::path::{Path, PathBuf};
use futures::stream::Stream;
use bytes::Bytes;
use crate::domain::models::file_info::FileInfo;

/// A trait defining common file system operations.
///
/// This trait allows for abstracting file system interactions, making it easier
/// to test and potentially swap out different file system implementations.
pub trait FileIO {
    /// Creates a new file at the specified path ignore if it exists.
    async fn create_file(&self, path: &Path) -> Result<()
    >;
    //Read a file content to a stream.
    fn read_file(&self,path:&Path)-> Result<impl Stream<Item=Result<Bytes
    >>>;
    //Write a buffer content to a file truncating the file(overwrite previous content).
    async fn write_file(&self,content:&[u8],path:&Path)->Result<()
    >;
    /// Removes a file at the specified path.
    async fn remove_file(&self, path: &Path) -> Result<()
    >;
    //append a buffer content to a file.
    async fn append_to_file(&self,content:& [u8],path:&Path)->Result<()
    >;
    //check if a path exists on the file system.
    async fn file_exists(&self,path:&Path)->Result<bool
    >;
    //Get file information or metadata.
    async fn file_info<'a>(&'a self,path:&'a Path)->Result<FileInfo>;

    //Set a predefined file length.
    async fn set_len(&self,path:&Path,file_size:usize,)->Result<()>;
    //Remember to add write_at method that will Seek to a position in a file and write a buffer to that location.
    


}

pub trait DirIO{
     /// Creates a directory and all its parent directories if they do not exist.
    async fn create_dir_all(&self, path: &Path) -> Result<()
    >;
    /// Recursively removes a directory and all its contents.
    async fn remove_dir_all(&self, path: &Path) -> Result<()
    >;
    //check if a path exists on the file system.
    async fn dir_exists(&self,path:&Path)->Result<bool
    >;
}
