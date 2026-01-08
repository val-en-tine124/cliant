use bytes::Bytes; 
use anyhow::Result;
use tokio_stream::Stream;

use crate::shared::errors::CliantError;
mod http_args;
mod http;
pub mod factory;

#[allow(unused)]
pub trait DataTransport:Send+Sync{
    async fn receive_data(&self,source:url::Url) -> Result<impl Stream<Item = Result<Bytes,CliantError>>,CliantError>;
}

