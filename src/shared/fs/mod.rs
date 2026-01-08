pub mod local;
use std::path::Path;

use bytes::Bytes;

use crate::shared::errors::CliantError;

pub trait FsOps{
    async fn append_bytes(&self,bytes:Bytes)->Result<(),CliantError>;
}