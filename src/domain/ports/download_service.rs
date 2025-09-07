use crate::domain::errors::DomainError;
use crate::domain::models::download_info::DownloadInfo;
use async_trait::async_trait;
use bytes::Bytes;
use std::pin::Pin;
use tokio_stream::Stream;
use url::Url;
///trait for downloading file from server.

pub trait DownloadService {
    fn get_bytes(
        &mut self,
        url: Url,
        range: &[u64; 2],
        buffer_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes,DomainError>> + Send + 'static>>,DomainError>;
}

pub trait ShutdownDownloadService{
    async fn shutdown(&mut self){

    }
}

///tait for fetching download name from server.
#[async_trait]
pub trait DownloadInfoService {
    ///Try to get the download file name from the server Content-Dispositon header.
    async fn get_info(&self, url: Url) -> Result<DownloadInfo, DomainError>;
}
