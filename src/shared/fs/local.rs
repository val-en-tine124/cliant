#![allow(unused)]
use bytes::Bytes;
use opendal::{Operator, Writer, services};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tracing::error;

use crate::shared::{errors::CliantError, fs::FsOps};

pub struct LocalFsBuilder {
    root_path: Option<PathBuf>,
    path: Option<PathBuf>,
}
impl LocalFsBuilder {
    pub fn new() -> Self {
        Self { root_path: None, path: None }
    }
    ///Root directory for all write operations.
    pub fn root_path(mut self, value: PathBuf) -> Self {
        self.root_path = Some(value);
        self
    }
    ///File path for write operation.
    pub fn path(mut self, value: PathBuf) -> Self {
        self.path = Some(value);
        self
    }
    pub async fn build(self) -> Result<LocalFs, CliantError> {
        let path = self.path.ok_or(CliantError::ParseError(
            "Path to file must be provided.".into(),
        ))?;
        let root_path = self.root_path.ok_or(CliantError::ParseError(
            "Root directory must be provided.".into(),
        ))?;
        let root_path_as_str=root_path.to_str().ok_or(CliantError::ParseError(format!("Can't get valid path object from path {}, invalid path",root_path.display())))?;
        let builder = services::Fs::default().root(root_path_as_str);
        let path_as_str =
            path.to_str().ok_or(CliantError::ParseError(format!(
                "Can't get valid path object from path {}, invalid path",path.display()
            )))?;
        let op = Operator::new(builder)
            .map_err(|err| CliantError::Io(err.into()))?
            .finish();
        let writer = op
            .writer_with(path_as_str)
            .chunk(4 * 1024 * 1024) // Make write buffer 4mb per write syscall.
            .append(true)
            .await
            .map_err(|err| CliantError::Io(err.into()))?;

        Ok(LocalFs { writer: Arc::new(Mutex::new(writer)) })
    }
}

pub struct LocalFs {
    writer: Arc<Mutex<Writer>>,
}

impl FsOps for LocalFs {
    ///This method takes a bytes and append it to a given file.
    ///
    /// **NB:** If  this method is called in different threads only one thread can write to the
    /// in-memory buffer at a time while other threads block.
    /// Rememeber to call `close_fs` after appending every chunk of bytes.
    async fn append_bytes(&self, bytes: Bytes) -> Result<(), CliantError> {
        let _ = self.writer.lock().await.write(bytes).await;

        Ok(())
    }
}

impl LocalFs {
    ///Call this method after appending every chunk of bytes.
    ///this method will flush the in-memory buffer to the File system.
    pub async fn close_fs(&self) {
        let mut writer = self.writer.lock().await;

        if let Err(err) = writer.close().await {
            error!(
                "Can't close opendal writer while dropping LocalFs an error occurred, writer has been closed already. :{err}"
            );
        }
    }
}

#[tokio::test]
async fn test_local_fs() -> anyhow::Result<()> {
    use tokio::sync::Semaphore;

    let localfs = LocalFsBuilder::new()
        .path(PathBuf::from("non_existent.txt"))
        .root_path(PathBuf::from("/home/val/Documents/"))
        .build()
        .await?;
    let localfs_arc = Arc::new(localfs);
    let semaphore = Arc::new(Semaphore::new(8)); // Create a semaphore of with a limit of 8 so only 8 tasks can run concurrently at a given time.
    let mut tasks_handle = vec![];
    for i in 1..=1000 {
        let sema_clone = semaphore.clone();
        let permit = sema_clone.acquire_owned().await?; // make sure the semaphore is acquired in this loop block and not tokio::spawn so exactly 8 tasks will be spawned. 
        //If the limit has been reached the acquire_owned will yield control back to the executor until a semaphore permit has been dropped.
        let localfs_clone = localfs_arc.clone();
        let handle = tokio::spawn(async move {
            let _ = permit;
            let _ = localfs_clone
                .append_bytes(Bytes::from_owner(format!("writing {i}...")))
                .await;
        });
        tasks_handle.push(handle);
    }
    for task in tasks_handle {
        task.await?;
    }

    localfs_arc.close_fs().await; // close writer and flush buffer after all write tasks have been completed

    Ok(())
}