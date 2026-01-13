use bytes::Bytes; 
use anyhow::Result;
use tokio_stream::Stream;
use url::Url;
use crate::shared::errors::CliantError;

#[cfg(feature="local")]
pub mod http;

pub mod factory;

pub trait DataTransport:Send+Sync{
    async fn receive_data(&self,source:Url) -> Result<impl Stream<Item = Result<Bytes,CliantError>>+Unpin,CliantError>;
    async fn total_bytes(&self,source:Url)->Result<Option<usize>,CliantError> ;
}

