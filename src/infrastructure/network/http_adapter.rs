#![allow(unused)]
use std::pin::Pin;
use std::{future::Future, time::Duration};

use async_trait::async_trait;
use chrono::Local;
use reqwest::{
    Client,
    header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, RANGE},
};
use bytes::Bytes;
use futures::StreamExt;
use regex::{Regex, RegexBuilder};
use tokio::sync::mpsc;
use tokio::time;
use tokio_stream::{Stream, wrappers::ReceiverStream};
use url::Url;

use super::super::config::http_config::HttpConfig;
use crate::domain::{
    errors::DomainError,
    models::download_info::DownloadInfo,
    ports::download_service::{DownloadInfoService, DownloadService},
};
/// http client wrapper for reqwest library.

pub struct RetryConfig {
    max_no_retries: usize,
    retry_delay_secs: usize,
    retry_backoff: usize,
}


impl RetryConfig {
    pub fn new(max_no_retries: usize, retry_delay_secs: usize, retry_backoff: usize) -> Self {
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
    pub fn retry_backoff(&self) -> usize {
        self.retry_backoff
    }
}

// a decorator to give  the DownloadService type retry capability.
struct RetryHttpAdapter<T> {
    inner: T,
    retry_config: RetryConfig,
}
impl<T> RetryHttpAdapter<T> {
    pub fn new(inner: T, retry_config: RetryConfig) -> Self
    where
        T: DownloadService + DownloadInfoService + Send + Sync + 'static,
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

impl<T: DownloadService + Send + Sync + 'static> DownloadService for RetryHttpAdapter<T> {
    ///Synchronous method to get a download bytes as continuous bytes streams.
    /// This method should be called in a seperate thread or tokio::ytask::spawn_blocking,
    /// or else it will block the current async runtime thread.
    fn get_bytes(
        &self,
        url: Url,
        range: &[u64; 2],
        buffer_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, DomainError>> + Send + 'static>>, DomainError>
    {
        let max_retries = self.retry_config.max_no_retries();
        let delay = self.retry_config.retry_delay_secs();
        let retry_backoff = self.retry_config.retry_backoff();
        let mut current_retry = 0;

        loop {
            match self.inner.get_bytes(url.clone(), range, buffer_size) {
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

                    println!(
                        "Retrying get bytes operation,current retry count {}...",
                        current_retry
                    );
                    current_retry += 1;
                    std::thread::sleep(Duration::from_secs(
                        (delay * retry_backoff.pow(current_retry as u32)) as u64,
                    ));
                }
            }
        }
    }
}

#[async_trait]
impl<T: DownloadInfoService + Send + Sync + 'static> DownloadInfoService for RetryHttpAdapter<T> {
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

                    println!(
                        "Retrying get bytes operation,current retry count {}...",
                        current_retry
                    );

                    current_retry += 1;
                    time::sleep(Duration::from_secs(
                        (delay * retry_backoff.pow(current_retry as u32)) as u64,
                    ))
                    .await;
                }
            }
        }
    }
}

pub struct HttpAdapter {
    client: Client,
}

impl HttpAdapter {
    pub fn new(config: HttpConfig) -> Result<Self, DomainError> {
        let client = config.try_into();
        match client {
            Err(err) => Err(Self::map_err(err)),
            Ok(client) => Ok(HttpAdapter { client }),
        }
    }

    ///This function handles parsing of contetn disposition header to extract download name.
    fn parse_content_disposition(content_disposition:&str)->Result<Option<String>,DomainError>{

        let pattern = r#"filename[^;=\n]*=((['"]).*?\2|[^;\n]*)"#; //regex pattern for extracting file name from Content-Disposition header.
        let regex_obj = Regex::new(pattern).map_err(|_| DomainError::Other {
            message: "Can't compile regex expression,incorrect pattern".into(),
        })?;

        if let Some(captures) = regex_obj.captures(content_disposition) {
            let filename = captures.get(1).map(|m| m.as_str().to_string());
            if let Some(fname) = filename {
                // if fname.starts_with("UTF-8''"){ // check if name starts  UTF-8''
                //     fname=percent_encoding::percent_decode_str(&fname[7..]).decode_utf8_lossy().to_string();
                // }
                return Ok(Some(fname));
            }
            return Ok(None);
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
                    "Can't connect from server,check your URL and your internet connection.".into(),
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

impl DownloadService for HttpAdapter {
    ///synchronous method to get a download bytes as continuous bytes streams.
    fn get_bytes(
        &self,
        url: Url,
        range: &[u64; 2],
        buffer_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, DomainError>> + Send + 'static>>, DomainError>
    {
        let (tx, rx) = mpsc::channel::<Result<Bytes, DomainError>>(buffer_size);
        let client_clone = self.client.clone();
        let url_clone = url.clone();
        let range_clone = *range;

        // Consider implementing logic to join this handle later.
        tokio::spawn(async move {
            let bytes_start = range_clone[0];
            let bytes_end = range_clone[1];
            let response = client_clone
                .get(url_clone)
                .header(RANGE, format!("bytes={}-{}", bytes_start, bytes_end))
                .send()
                .await;

            match response {
                Ok(mut resp) => loop {
                    match resp.chunk().await {
                        Ok(Some(bytes)) => {
                            if let Err(err) = tx.send(Ok(bytes)).await {
                                eprintln!("Error:{}", err.to_string());
                                break;
                            }
                        }
                        Ok(None) => {
                            break;
                        }
                        Err(err) => {
                            if let Err(err) = tx.send(Err(Self::map_err(err.into()))).await {
                                eprintln!("Error:{}", err.to_string());
                            }
                            break;
                        }
                    }
                },
                Err(err) => {
                    if let Err(err) = tx.send(Err(Self::map_err(err.into()))).await {
                        eprintln!("Error:{}", err.to_string());
                    }
                }
            }
        });
        Ok(ReceiverStream::new(rx).boxed())
    }
}

#[async_trait]
impl DownloadInfoService for HttpAdapter {
    ///Asynchronous method to Build DownloadInfo object from a given url.
    async fn get_info(&self, url: Url) -> Result<DownloadInfo, DomainError> {
        let mut size_info = None;
        let mut name_info = None;
        let mut content_type_info = None;
        let resp = self
            .client
            .head(url.clone())
            .send()
            .await
            .map_err(|e| Self::map_err(e.into()))?;

        let name_option: Option<Result<&str, DomainError>> = resp
            .headers()
            .get(CONTENT_DISPOSITION)
            .map(|header|->Result<&str,DomainError>{
             header
             .to_str()
             .map_err(|_|
                DomainError::Other{message:"Error !, Can't convert response header CONTENT-DISPOSITION header to string".into()}
            )});

        match name_option {
            Some(name_result) => {
                if let Ok(Some(name)) = Self::parse_content_disposition(name_result?){
                    name_info=Some(name);
                }
                
            }
            None => {}
        }

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
            size_info = Some(size?.trim().parse::<usize>().map_err(|_| {
                DomainError::Other {
                    message:
                        "Error !, Can't convert  file size from http header to usize object header"
                            .into(),
                }
            })?);
        }

        let content_type_result =
            resp.headers()
                .get(CONTENT_TYPE)
                .map(|header| -> Result<&str, DomainError> {
                    header.to_str().map_err(|_| DomainError::Other {
                        message:
                            "Error !, Can't convert response header CONTENT-TYPE header to string"
                                .into(),
                    })
                });

        if let Some(content_type) = content_type_result {
            content_type_info = Some(content_type?.to_string());
        }

        let download_date = Local::now();
        Ok(DownloadInfo::new(
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
async fn test_get_bytes() {
    let config = HttpConfig {
        username: None,
        password: None,
        max_redirects: None,
        timeout: None,
        proxy_url: None,
        request_headers: None,
        http_cookies: None,
        http_version: None,
    };
    match HttpAdapter::new(config) {
        Ok(client) => {
            if let Ok(url) = Url::parse("http://127.0.0.1:8080/fake_mp4.mp4") {
                let mut bytes_stream = client
                    .get_bytes(url.clone(), &[0, 10000], 1024)
                    .expect("can't get bytes");
                while let Some(part) = bytes_stream.next().await {
                    println!("{:?}", part);
                }
            }
        }

        Err(err) => {
            println!("{}", err);
        }
    }

    assert!(true);
}

#[tokio::test]
async fn test_check_name() {
    let config = HttpConfig {
        username: None,
        password: None,
        max_redirects: None,
        timeout: None,
        proxy_url: None,
        request_headers: None,
        http_cookies: None,
        http_version: None,
    };
    match HttpAdapter::new(config) {
        Ok(client) => {
            if let Ok(url) = Url::parse("http://127.0.0.1:8080/fake_mp4.mp4") {
                let info = client.get_info(url).await.expect("Download info");
                let name = info.name().clone().unwrap_or("No name !".into());
                let date = info.download_date();
                let download_type = info.download_type().clone().unwrap_or("No type !".into());
                let size = info.size().unwrap_or(0);
                println!(
                    "name:{},date:{},download type:{},size:{}",
                    name, date, download_type, size
                );
            }
        }

        Err(err) => {
            println!("{}", err);
        }
    }

    assert!(true);
}

#[tokio::test]
async fn test_retry_adapter(){
    let config = HttpConfig {
        username: None,
        password: None,
        max_redirects: None,
        timeout: None,
        proxy_url: None,
        request_headers: None,
        http_cookies: None,
        http_version: None,
    };
    let http_adapter=HttpAdapter::new(config);
    let retry_config=RetryConfig::new(10, 10, 2);

    match http_adapter{
        Ok(adapter)=>{

            let new_adapter=RetryHttpAdapter::new(adapter,retry_config);
            if let Ok(url) = Url::parse("http://127.0.0.1:8080/fake_mp4.mp4") {
                let info=new_adapter.get_info(url.clone()).await.expect("Can't get info.");
                let name = info.name().clone().unwrap_or("No name !".into());
                let date = info.download_date();
                let download_type = info.download_type().clone().unwrap_or("No type !".into());
                let size = info.size().unwrap_or(0);
                println!(
                    "name:{},date:{},download type:{},size:{}",
                    name, date, download_type, size
                );

                let task_handle=tokio::task::spawn_blocking(move || {
                    let stream=new_adapter.get_bytes(url, &[10,10000],2048);
                    stream
                    }    
                );

                let  mut stream=task_handle.await.expect("Can't join task handle.").expect("Can't get bytes.");
                while let Some(Ok(bytes))=stream.next().await{
                        println!("bytes :{:?}",bytes);
                }
                
            }
        },
            
        Err(err)=>{
            println!("{}",err);
        }
    

    }
    
    assert!(true);
}

#[test]
fn test_regex(){
    let pattern = r#"filename[^;=\n]*=((['"]).*?\2|[^;\n]*)"#; //regex pattern for extracting file name from Content-Disposition header.
    
    let regex_obj = RegexBuilder::new(pattern).size_limit(1_000_000).build().map_err(|e| DomainError::Other {
        message: format!("Can't compile regex expression,incorrect pattern:{}",e),
    });
    match regex_obj{
        Ok(regex)=>{
          if let Some(captures) = regex.captures("filename=my_file.mp4") {
            let filename = captures.get(1).map(|m| m.as_str().to_string());
            if let Some(fname) = filename {
                // if fname.starts_with("UTF-8''"){ // check if name starts  UTF-8''
                //     fname=percent_encoding::percent_decode_str(&fname[7..]).decode_utf8_lossy().to_string();
                // }
                println!("extracted filename: {}",fname); 
            }
        }  
},
        Err(err)=>{
            eprintln!("Error: {}",err);
        }
    }
}
    



