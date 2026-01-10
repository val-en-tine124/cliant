pub mod local;

use bytes::Bytes;

use crate::shared::errors::CliantError;

pub trait FsOps{
    async fn append_bytes(&self,bytes:Bytes)->Result<(),CliantError>;
} 