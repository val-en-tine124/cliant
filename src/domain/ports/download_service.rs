use crate::domain::errors::DomainError;
use crate::domain::models::download_info::DownloadInfo;
use async_trait::async_trait;
use bytes::Bytes;
use std::pin::Pin;
use tokio_stream::Stream;
use url::Url;

///# DownloadService
/// trait for downloading file from server.
/// ### Parameters:
/// * url : the download url.
/// * range : slice of integers for protocols that support multipart downloading.
/// * buffer_size : size of the in-memory buffer.

pub trait MultiPartDownload {
    fn get_bytes_range(
        &mut self,
        url: Url,
        range: &[usize; 2],
        buffer_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes,DomainError>> + Send + 'static>>,DomainError>;
}

pub trait SimpleDownload{
    fn get_bytes(
        &mut self,
        url: Url,
        buffer_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes,DomainError>> + Send + 'static>>,DomainError>;
}

///trait for gracefully shutting deown protocols.
pub trait ShutdownDownloadService{
    async fn shutdown(&mut self){

    }
}

///tait for fetching download name from server.
/// ### Parameters:
/// * url : the download url.
#[async_trait]
pub trait DownloadInfoService {
    ///Try to get the download file name from the server Content-Dispositon header.
    async fn get_info(&self, url: Url) -> Result<DownloadInfo, DomainError>;
}
