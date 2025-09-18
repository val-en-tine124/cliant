#![allow(unused)]
use std::borrow::Cow;
use std::pin::Pin;
use std::{future::Future, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Local;
use fancy_regex::Regex;
use futures::StreamExt;
use reqwest::Response;
use reqwest::{
    Client,
    header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, RANGE},
};
use tokio::sync::mpsc::{self,Sender};
use tokio::task::{AbortHandle, JoinSet};
use tokio::time;
use tokio_stream::{Stream, wrappers::ReceiverStream};
use tracing::{debug, error, info, instrument, span, warn};
use url::Url;

use super::super::config::http_config::HttpConfig;
use crate::domain::{
    errors::DomainError,
    models::download_info::DownloadInfo,
    ports::download_service::{
        DownloadInfoService, MultiPartDownload, ShutdownDownloadService, SimpleDownload,
    },
};
/// http client wrapper for reqwest library.

pub struct RetryConfig {
    max_no_retries: usize,
    retry_delay_secs: usize,
    retry_backoff: f32,
}

impl RetryConfig {
    pub fn new(max_no_retries: usize, retry_delay_secs: usize, retry_backoff: f32) -> Self {
        Self {
            max_no_retries,
            retry_delay_secs,
            retry_backoff,
        }
    }
    pub fn max_no_retries(&self) -> usize {
        self.max_no_retries
    }
    pub fn retry_delay_secs(&self) -> usize {
        self.retry_delay_secs
    }
    pub fn retry_backoff(&self) -> f32 {
        self.retry_backoff
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_no_retries: 10,
            retry_delay_secs: 10,
            retry_backoff: 2.0,
        }
    }
}

// a decorator to give  theMultiPartDownload type retry capability.
struct RetryHttpAdapter<T> {
    inner: T,
    retry_config: RetryConfig,
}
impl<T> RetryHttpAdapter<T> {
    pub fn new(inner: T, retry_config: RetryConfig) -> Self
    where
        T: MultiPartDownload + DownloadInfoService + Send + Sync + 'static,
    {
        Self {
            inner: inner,
            retry_config: retry_config,
        }
    }
    fn can_retry(err: &DomainError) -> bool {
        match err {
            DomainError::NetworkConnectError(_) | DomainError::NetworkTimeoutError(_) => true,
            _ => false,
        }
    }
}

impl<T: ShutdownDownloadService> ShutdownDownloadService for RetryHttpAdapter<T> {
    ///this async method will do proper shutdown.
    /// This method should be the last method of HttpAdapter that should be called.
    async fn shutdown(&mut self) {
        self.inner.shutdown().await;
    }
}

impl<T: MultiPartDownload + Send + Sync + 'static> MultiPartDownload for RetryHttpAdapter<T> {
    ///Synchronous method to get a download bytes as continuous bytes streams.
    /// This method should be called in a seperate thread or tokio::task::spawn_blocking,
    /// or else it will block the current async runtime thread.
    #[instrument(name="retry_reqwest_adapter_get_bytes_range",skip(self),fields(url=url.as_str(),range=format!("{:?}", range),buffer_size=buffer_size))]
    fn get_bytes_range(
        &mut self,
        url: Url,
        range: &[usize; 2],
        buffer_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, DomainError>> + Send + 'static>>, DomainError>
    {
        let max_retries = self.retry_config.max_no_retries();
        let delay = self.retry_config.retry_delay_secs();
        let retry_backoff = self.retry_config.retry_backoff();
        let mut current_retry = 0;

        loop {
            match self.inner.get_bytes_range(url.clone(), range, buffer_size) {
                Ok(bytes_stream) => {
                    return Ok(bytes_stream.boxed());
                }

                Err(err) => {
                    if !Self::can_retry(&err) {
                        return Err(err);
                    } else if current_retry >= max_retries {
                        return Err(DomainError::Other {
                            message: "Retry operation timeout, can't get file".into(),
                        });
                    }

                    warn!(
                        "Retrying get bytes operation,current retry count {}...",
                        current_retry
                    );
                    current_retry += 1;
                    std::thread::sleep(Duration::from_secs(
                        (delay as f32 * retry_backoff.powf(current_retry as f32)) as u64,
                    ));
                }
            }
        }
    }
}

#[async_trait]
impl<T: DownloadInfoService + Send + Sync + 'static> DownloadInfoService for RetryHttpAdapter<T> {
    #[instrument(name="retry_reqwest_adapter_get_info",skip(self),fields(url=url.as_str()))]
    async fn get_info(&self, url: Url) -> Result<DownloadInfo, DomainError> {
        let max_retries = self.retry_config.max_no_retries();
        let delay = self.retry_config.retry_delay_secs();
        let retry_backoff = self.retry_config.retry_backoff();
        let mut current_retry = 0;
        loop {
            match self.inner.get_info(url.clone()).await {
                Ok(download_info) => {
                    return Ok(download_info);
                }
                Err(err) => {
                    if !Self::can_retry(&err) {
                        return Err(err);
                    } else if current_retry >= max_retries {
                        return Err(DomainError::Other {
                            message: "Retry operation timeout, can't get file".into(),
                        });
                    }

                    warn!(
                        "Retrying get bytes operation,current retry count {}...",
                        current_retry
                    );

                    current_retry += 1;
                    time::sleep(Duration::from_secs(
                        (delay as f32 * retry_backoff.powf(current_retry as f32)) as u64,
                    ))
                    .await;
                }
            }
        }
    }
}

pub struct HttpAdapter {
    client: Client,
    pool_handles:Vec<AbortHandle>,
    pool: JoinSet<()>,
}

impl HttpAdapter {
    #[instrument(name="new_http_adapter",fields(config=format!("{:?}", config)))]
    pub fn new(config: HttpConfig) -> Result<Self, DomainError> {
        let client = config.try_into();
        match client {
            Err(err) => Err(Self::map_err(err)),
            Ok(client) => {
                let pool = JoinSet::new();
                Ok(HttpAdapter {
                    client,
                    pool,
                    pool_handles: vec![],
                })
            }
        }
    }

    async fn process_chunk(mut resp: Response,tx:Sender<Result<Bytes, DomainError>>) {
        loop {
            match resp.chunk().await {
                Ok(Some(bytes)) => {
                    if let Err(err) = tx.send(Ok(bytes)).await {
                        error!(error = %err, "Error sending bytes to channel");
                        break;
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(err) => {
                    if let Err(err) = tx.send(Err(Self::map_err(err.into()))).await {
                        error!(error = %err, "Error sending error to channel");
                    }
                    break;
                }
            }
        }
    }

    ///This function handles parsing of content disposition header to extract download name.
    #[instrument(name="parse_content_disposition",fields(content_disposition=content_disposition))]
    fn parse_content_disposition(
        content_disposition: &str,
    ) -> Result<Option<Cow<'_, str>>, DomainError> {
        let pattern = r#"filename[^;=\n]*=((['"]).*?\2|[^;\n]*)"#; //regex pattern for extracting file name from Content-Disposition header.Don't use it for now because regex::Regex can't compile it because backreferencing is currently not supported.
        let regex_obj = Regex::new(pattern).map_err(|_| DomainError::Other {
            message: "Can't compile regex expression,incorrect pattern.".into(),
        })?;

        match regex_obj.captures(content_disposition) {
            Ok(Some(captures)) => {
                let filename = captures.get(1).map(|m| m.as_str().to_string());
                if let Some(mut fname) = filename {
                    if fname.starts_with("UTF-8''") {
                        // check if name starts  UTF-8''
                        debug!(name:"download_name_prefix","Download name starts with UTF-8''.");
                        fname = percent_encoding::percent_decode_str(&fname[7..])
                            .decode_utf8_lossy()
                            .trim_matches('"') //trim strings with " matches after decoding string.
                            .to_string();
                    }

                    return Ok(Some(Cow::from(fname)));
                }
            }
            Ok(None) => {
                debug!("No regex capture found for string :{}", content_disposition);
            }

            Err(err) => error!(error = %err, "Error capturing regex"),
        }

        Ok(None)
    }

    fn map_err(err: anyhow::Error) -> DomainError {
        if let Some(error) = err.downcast_ref::<reqwest::Error>() {
            if error.is_timeout() {
                return DomainError::NetworkTimeoutError("Download request timeout, can't fetch file from server.check your connection and try again.".into());
            }
            if error.is_connect() {
                return DomainError::NetworkConnectError(
                    "Can't connect to server,check your URL and your internet connection.".into(),
                );
            }
            if error.is_redirect() {
                return DomainError::NetworkError("Network error, http client exceeded max redirect, or invalid redirect configuration.".into());
            }
            if error.is_decode() {
                return DomainError::NetworkError(
                    "Network error, Can't decode http response body".into(),
                );
            }
            if error.is_body() {
                return DomainError::NetworkError(
                    "Network error, error is related to the request or response body".into(),
                );
            }
            if error.is_request() {
                return DomainError::NetworkError("Network error, invalid http request.".into());
            }
            if error.is_builder() {
                return DomainError::NetworkError("Error, Invalid request Configuration !".into());
            }
            if let Some(error) = error.status() {
                if error.is_client_error() {
                    return DomainError::NetworkError(
                        "Client error check your request parameters and configuration!".into(),
                    );
                }
                if error.is_informational() {
                    return DomainError::NetworkError("Informational Error!".into());
                }
                if error.is_redirection() {
                    return DomainError::NetworkError("Redirect Error!".into());
                }
                if error.is_server_error() {
                    return DomainError::NetworkError(
                        "Server Error !, error originated from http server.".into(),
                    );
                }
                if let Some(reason) = error.canonical_reason() {
                    return DomainError::NetworkError(format!("Network error, reason:{}", reason));
                }
            } else {
                return DomainError::NetworkError("Network error, unable to download file.".into());
            }
        }
        DomainError::Other {
            message: "Unknown error occurred. ".into(),
        }
    }
}

impl ShutdownDownloadService for HttpAdapter {
    ///This method will only wait for the task pool (all tasks) to finish.
    /// This method should be the last method of HttpAdapter that should be called.
    #[instrument(name = "shutdown_http_adapter", skip(self))]
    async fn shutdown(&mut self) {
        while self.pool.join_next().await.is_some() {}
    }
}

impl SimpleDownload for HttpAdapter {
    fn get_bytes(
        &mut self,
        url: Url,
        buffer_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, DomainError>> + Send + 'static>>, DomainError>
    {
        debug!(name:"initialize_channel","Initialize Stream channel");
        let (tx, rx) = mpsc::channel::<Result<Bytes, DomainError>>(buffer_size);
        let client_clone = self.client.clone();
        let url_clone = url.clone();

        let resolve_streaming = async move {
            debug!(name:"initialize_simple_response","Initializing simple Http response.");
            let response = client_clone.get(url_clone).send().await;

            match response {
                Ok(mut resp) => {
                    debug!(name:"successful_simple_response","Simple response successful, getting response chunks.");
                    Self::process_chunk(resp,tx).await;
                }
                Err(err) => {
                    if let Err(err) = tx.send(Err(Self::map_err(err.into()))).await {
                        error!(error = %err, "Error sending error to channel");
                    }
                }
            }
        };

        let abort_handle = self.pool.spawn(resolve_streaming);

        self.pool_handles.push(abort_handle);

        Ok(ReceiverStream::new(rx).boxed())
    }
}

impl MultiPartDownload for HttpAdapter {
    ///Synchronous method to get a download bytes as continuous thread safe bytes streams.
    /// This method uses a task pool of type tokio::task::JoinSet<()> for spawning async tasks efficiently,
    /// because this method is expected to be called multiple times.
    #[instrument(name="reqwest_adapter_get_bytes_range",skip(self,),fields(url=url.as_str(),range=format!("{:?}", range),buffer_size=buffer_size))]
    fn get_bytes_range(
        &mut self,
        url: Url,
        range: &[usize; 2],
        buffer_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, DomainError>> + Send + 'static>>, DomainError>
    {
        debug!(name:"initialize_channel","Initialize Stream channel");
        let (tx, rx) = mpsc::channel::<Result<Bytes, DomainError>>(buffer_size);
        let client_clone = self.client.clone();
        let url_clone = url.clone();
        let range_clone = *range;

        let resolve_streaming = async move {
            let bytes_start = range_clone[0];
            let bytes_end = range_clone[1];
            debug!(name:"initialize_multipart _response","Initializing multipart Http response.");
            let response = client_clone
                .get(url_clone)
                .header(RANGE, format!("bytes={}-{}", bytes_start, bytes_end))
                .send()
                .await;

            match response {
                Ok(mut resp) => {
                    debug!(name:"successful_multipart_response","Multipart response, successful getting response chunks.");
                    Self::process_chunk(resp,tx);
                }
                Err(err) => {
                    if let Err(err) = tx.send(Err(Self::map_err(err.into()))).await {
                        error!(error = %err, "Error sending error to channel");
                    }
                }
            }
        };

        let abort_handle = self.pool.spawn(resolve_streaming);

        self.pool_handles.push(abort_handle);

        Ok(ReceiverStream::new(rx).boxed())
    }
}

#[async_trait]
impl DownloadInfoService for HttpAdapter {
    #[instrument(name="reqwest_adapter_get_info",skip(self),fields(url=url.as_str()))]
    ///Asynchronous method to Build DownloadInfo object from a given url.
    async fn get_info(&self, url: Url) -> Result<DownloadInfo, DomainError> {
        let mut size_info = None;
        let mut name_info: Option<String> = None;
        let mut content_type_info: Option<String> = None;
        debug!(name:"Initialize_Response","Initializing Http Header Response for Url {}",url.clone());

        let resp = self
            .client
            .head(url.clone())
            .send()
            .await
            .map_err(|e| Self::map_err(e.into()))?;
        debug!(name:"nullable_name_result","Checking nullable download name for url {}.",url.clone());

        let name_option: Option<Result<String, DomainError>> = resp
            .headers()
            .get(CONTENT_DISPOSITION)
            .map(|header|->Result<String,DomainError>{
             header
             .to_str()
             .map(|s| s.to_string())
             .map_err(|_|
                DomainError::Other{message:"Error !, Can't convert response header CONTENT-DISPOSITION header to string".into()}
            )});

        if let Some(name_result) = name_option {
            if let Ok(name) = name_result {
                if let Some(parsed_name) = Self::parse_content_disposition(&name)? {
                    debug!(name:"download_name_ready","Got download name {}",parsed_name.as_ref());
                    name_info = Some(parsed_name.into_owned());
                }
            }
        } else {
            debug!(name:"no_download_name","No name for url {} ,in http header Content-Disposition",url.clone());
        }

        debug!(name:"nullable_size_result","Checking nullable download size for url {}.",url.clone());
        let size_result =
            resp.headers()
                .get(CONTENT_LENGTH)
                .map(|header| -> Result<&str, DomainError> {
                    header.to_str().map_err(|_| DomainError::Other {
                        message:
                            "Error !, Can't convert response header CONTENT-LENGTH header to string"
                                .into(),
                    })
                });

        if let Some(size) = size_result {
            debug!(name:"download_size_ready","Got download size.");
            size_info = Some(size?.trim().parse::<usize>().map_err(|_| {
                DomainError::Other {
                    message:
                        "Error !, Can't convert  file size from http header to usize object header"
                            .into(),
                }
            })?);
        } else {
            debug!(name:"no_download_size","No name for url {} ,in http header Content-Length",url.clone());
        }

        debug!(name:"nullable_type_result","Checking nullable download type for url {}.",url.clone());
        let content_type_result: Option<Result<String, DomainError>> = resp
            .headers()
            .get(CONTENT_TYPE)
            .map(|header| -> Result<String, DomainError> {
                header
                    .to_str()
                    .map(|s| s.to_string())
                    .map_err(|_| DomainError::Other {
                        message:
                            "Error !, Can't convert response header CONTENT-TYPE header to string"
                                .into(),
                    })
            });

        
        if let Some(content_type_result) = content_type_result {
            let content_type=content_type_result?;
            debug!(name:"download_type_ready","Got download content type {}",&content_type);
            content_type_info = Some(content_type);
        } else {
            debug!(name:"no_download_type","No type for url {} ,in http header Content-Type",url.clone());
        }

        let download_date = Local::now();
        Ok(DownloadInfo::new(
            url.clone(),
            name_info,
            size_info,
            download_date,
            content_type_info,
        ))
    }
}

async fn timeout<F: Future>(function: F, delay: Duration) -> Result<F::Output, Duration> {
    tokio::select! {
        result=function=>Ok(result),
        _=time::sleep(delay)=>Err(delay)
    }
}

#[tokio::test]
async fn test_timeout() {
    let sleep_more = async || time::sleep(Duration::from_secs(18));
    let timeout_result = timeout(sleep_more(), Duration::from_secs(20)).await;
    assert!(timeout_result.is_ok());
}

#[tokio::test]
async fn test_simple_download() {
    let config = HttpConfig::default();
    match HttpAdapter::new(config) {
        Ok(mut client) => {
            if let Ok(url) = Url::parse("http://127.0.0.1:8080/fake_mp4.mp4") {
                let mut streams: Vec<
                    Pin<Box<dyn Stream<Item = Result<Bytes, DomainError>> + Send + 'static>>,
                > = vec![];
                for _ in 1..=3 {
                    let mut bytes_stream = client
                        .get_bytes(url.clone(), 1024)
                        .expect("can't get bytes");
                    streams.push(bytes_stream);
                }

                for (idx, mut stream) in streams.into_iter().enumerate() {
                    while let Some(Ok(part)) = stream.next().await {
                        info!("stream number {}\n: {:?}", idx, part);
                    }
                }

                info!("Shutting down!");
                client.shutdown().await;
                info!("Shutdown successful!");
            }
        }

        Err(err) => {
            error!(error = %err, "Error creating http client");
        }
    }

    assert!(true);
}

#[tokio::test]
async fn test_get_bytes() {
    let config = HttpConfig::default();
    match HttpAdapter::new(config) {
        Ok(mut client) => {
            if let Ok(url) = Url::parse("http://127.0.0.1:8080/fake_mp4.mp4") {
                let mut streams: Vec<
                    Pin<Box<dyn Stream<Item = Result<Bytes, DomainError>> + Send + 'static>>,
                > = vec![];
                for _ in 1..=3 {
                    let mut bytes_stream = client
                        .get_bytes_range(url.clone(), &[0, 10000], 1024)
                        .expect("can't get bytes");
                    streams.push(bytes_stream);
                }

                for (idx, mut stream) in streams.into_iter().enumerate() {
                    while let Some(Ok(part)) = stream.next().await {
                        info!("stream number {}\n: {:?}", idx, part);
                    }
                }

                info!("Shutting down!");
                client.shutdown().await;
                info!("Shutdown successful!");
            }
        }

        Err(err) => {
            error!(error = %err, "Error creating http client");
        }
    }

    assert!(true);
}

#[tokio::test]
async fn test_retry_check_name() {
    let config = HttpConfig::default();
    let retry_config = RetryConfig::new(10, 10, 0.2);
    match HttpAdapter::new(config) {
        Ok(client) => {
            if let Ok(url) = Url::parse("http://127.0.0.1:8080/fake_mp4.mp4") {
                let new_adapter = RetryHttpAdapter::new(client, retry_config);
                let info = new_adapter.get_info(url).await.expect("Download info");
                let name = info.name().clone().unwrap_or("No name !".into());
                let date = info.download_date();
                let download_type = info.download_type().clone().unwrap_or("No type !".into());
                let size = info.size().unwrap_or(0);
                info!(
                    "name:{},date:{},download type:{},size:{}",
                    name, date, download_type, size
                );
            }
        }

        Err(err) => {
            error!(error = %err, "Error creating http client");
        }
    }

    assert!(true);
}

// I haven't run this test, uncomment when ready.

// #[tokio::test]
// async fn test_retry_bytes() {
//     let config = HttpConfig::default();
//     let retry_config = RetryConfig::default();
//     match HttpAdapter::new(config) {
//         Ok(client) => {
//             if let Ok(url) = Url::parse("http://127.0.0.1:8080/fake_mp4.mp4") {
//                 let mut new_adapter = RetryHttpAdapter::new(client, retry_config);
//                 let bytes_result = tokio::task::spawn_blocking(move || {
//                     new_adapter.get_bytes_range(url, &[0, 40000], 4044)
//                 })
//                 .await;

//                 assert!(bytes_result.is_ok()); //Assert successful handle joining.
//                 let inner_result = bytes_result.unwrap();
//                 assert!(inner_result.is_err()); // check if function can be retried.
//                 if let Err(err) = inner_result {
//                     assert!(
//                         matches!(err,DomainError::Other { message } if message.contains("timeout"))
//                     );
//                 }
//             }
//         }
//         Err(err) => {
//             error!(error = %err, "Can't create http client");
//         }
//     }
// }
