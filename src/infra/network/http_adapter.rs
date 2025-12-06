#![allow(unused)]
use std::borrow::Cow;
use std::pin::Pin;
use std::{future::Future, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Local;
use fancy_regex::Regex;
use futures::StreamExt;

use anyhow::{anyhow, Context, Result};
use reqwest::{
    header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, RANGE},
    Client, Response,
};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use reqwest_tracing::TracingMiddleware;
use tokio::sync::mpsc::{self, Sender};
use tokio::task::{AbortHandle, JoinHandle, JoinSet};
use tokio::time;
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tracing::{debug, error, info, instrument, warn};
use url::Url;

use crate::utils::create_byte_stream;
use super::super::config::{HttpConfig,RetryConfig};
use crate::domain::{
    models::DownloadInfo,
    ports::download_service::{DownloadInfoService, MultiPartDownload, SimpleDownload},
};
/// http client wrapper for reqwest library.
type BoxedStream=Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>;


pub struct HttpAdapter {
    client: ClientWithMiddleware,
}

impl HttpAdapter {
    #[instrument(name="new_http_adapter",fields(config=format!("{:?}\n{:?}", http_config,retry_config)))]
    pub fn new(http_config: HttpConfig, retry_config: &RetryConfig) -> Result<Self> {
        let delay_secs = *retry_config.retry_delay_secs();
        let max_retry_bound=delay_secs.max(2);
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(
                Duration::from_secs(1),
                Duration::from_secs(max_retry_bound as u64),
            )
            .build_with_max_retries(*retry_config.max_no_retries() as u32);
        let retry_middleware = RetryTransientMiddleware::new_with_policy(retry_policy);
        let try_client = Client::try_from(http_config)
            .context("Can't create http client due to misconfiguration.")?;
        let client: ClientWithMiddleware = ClientBuilder::new(try_client)
            .with(TracingMiddleware::default())
            .with(retry_middleware)
            .build();
        Ok(Self { client })
    }
    //This function get http response body as chunks,log event especially error and send the chunk to a reciever
    async fn process_chunk(mut resp: Response, tx: Sender<Result<Bytes>>) {
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
                    //Propagate error to sender to handle it.
                    if let Err(err) = tx.send(Err(err.into())).await {
                        error!(error = %err, "Error sending error to channel");
                    }
                    break;
                }
            }
        }
    }

    ///This function handles parsing of content disposition header to extract download name.
    #[instrument(name="parse_content_disposition",fields(content_disposition=content_disposition))]
    fn parse_content_disposition(content_disposition: &str) -> Result<Option<Cow<'_, str>>> {
        let pattern = r#"filename[^;=\n]*=((['"]).*?\2|[^;\n]*)"#; //regex pattern for extracting file name from Content-Disposition header.Don't use it for now because regex::Regex can't compile it because backreferencing is currently not supported.
        let regex_obj =
            Regex::new(pattern).context("Can't compile regex expression,incorrect pattern.")?;

        match regex_obj.captures(content_disposition) {
            Ok(Some(captures)) => {
                let mut filename = captures
                    .get(1)
                    .map(|m| m.as_str().to_string())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Can't capture name pattern in content-disposition header.")
                    })?
                    .trim_matches('\"')
                    .to_string();
                if filename.starts_with("UTF-8''") {
                    // check if name starts  UTF-8''
                    debug!(
                        name = "download_name_prefix",
                        "Download name starts with UTF-8''."
                    );
                    filename = percent_encoding::percent_decode_str(&filename[7..])
                        .decode_utf8_lossy()
                        .to_string();
                }

                return Ok(Some(Cow::from(filename)));
            }
            Ok(None) => {
                debug!("No regex capture found for string :{}", content_disposition);
            }

            Err(err) => error!(error = %err, "Error capturing regex"),
        }

        Ok(None)
    }
}



impl SimpleDownload for HttpAdapter {
    fn get_bytes(
        &mut self,
        url: Url,
        buffer_size: usize,
    ) -> Result<(
        Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>,
        JoinHandle<()>,
    )> {
        debug!(name = "initialize_channel", "Initialize Stream channel");
        let client_clone = self.client.clone();
        let url_clone = url.clone();

        let (stream, handle) = create_byte_stream(buffer_size, move |tx| async move {
            debug!(
                name = "initialize_simple_response",
                "Initializing simple Http response."
            );
            let response = client_clone.get(url_clone).send().await;

            match response {
                Ok(mut resp) => {
                    info!(
                        name = "successful_simple_response",
                        "Simple response successful, getting response chunks."
                    );
                    Self::process_chunk(resp, tx).await;
                }
                Err(err) => {
                    if let Err(err) = tx.send(Err(err.into())).await {
                        error!(error = %err, "Error sending error to channel");
                    }
                }
            }
        });

        Ok((stream, handle))
    }
}

impl MultiPartDownload for HttpAdapter {
    ///Synchronous method to get a download bytes as continuous thread safe bytes streams.
    /// This method uses a task pool of type ``tokio::task::JoinSet<()>`` for spawning async tasks efficiently,
    /// because this method is expected to be called multiple times.
    #[instrument(name="reqwest_get_bytes_range",skip(self,),fields(url=url.as_str(),range=format!("{:?}", range),buffer_size=buffer_size))]
    fn get_bytes_range(
        &mut self,
        url: Url,
        range: &[usize; 2],
        buffer_size: usize,
    ) -> Result<(
        BoxedStream,
        JoinHandle<()>
    )> {
        debug!(
            name = "initialize_channel",
            "Initializing Stream channel..."
        );

        let client_clone = self.client.clone();
        let url_clone = url.clone();
        let range_clone = *range;

        let (stream, handle) = create_byte_stream(buffer_size, move |tx| async move {
            let bytes_start = range_clone[0];
            let bytes_end = range_clone[1];
            debug!(
                name = "initialize_multipart_response",
                "Initializing multipart Http response..."
            );
            let response = client_clone
                .get(url_clone)
                .header(RANGE, format!("bytes={bytes_start}-{bytes_end}"))
                .send()
                .await;

            match response {
                Ok(mut resp) => {
                    info!(
                        name = "successful_multipart_response",
                        "Multipart response, successful getting response chunks."
                    );
                    Self::process_chunk(resp, tx).await;
                }
                Err(err) => {
                    if let Err(err) = tx.send(Err(err.into())).await {
                        error!(error = %err, "Error sending error to channel");
                    }
                }
            }
        });

        Ok((stream, handle))
    }
}

#[async_trait]
impl DownloadInfoService for HttpAdapter {
    #[instrument(name="reqwest_get_info",skip(self),fields(url=url.as_str()))]
    ///Asynchronous method to Build ``DownloadInfo`` object from a given url.
    async fn get_info(&self, url:Url) -> Result<DownloadInfo> {
        let mut size_info = None;
        let mut name_info: Option<String> = None;
        let mut content_type_info: Option<String> = None;
        debug!(
            name = "Initialize_Response",
            "Initializing Http Header Response for Url {}",
            &url
        );

        let resp = self
            .client
            .head(url.clone())
            .send()
            .await?
            .error_for_status()?;
        debug!(
            name = "nullable_name_result",
            "Checking nullable download name for url {}.",
            &url
        );

        let name_option: Option<Result<String>> =
            resp.headers()
                .get(CONTENT_DISPOSITION)
                .map(|header| -> Result<String> {
                    header
             .to_str()
             .map(|s| s.to_string())
             .context("Error !, Can't convert response header CONTENT-DISPOSITION header to string")
                });

        if let Some(name_result) = name_option {
            if let Ok(name) = name_result {
                if let Some(parsed_name) = Self::parse_content_disposition(&name)? {
                    debug!(
                        name = "download_name_ready",
                        "Got download name {}",
                        parsed_name.as_ref()
                    );
                    name_info = Some(parsed_name.into_owned());
                }
            }
        } else {
            debug!(
                name = "no_download_name",
                "No name for url {} ,in http header Content-Disposition",
                &url
            );
        }

        debug!(
            name = "nullable_size_result",
            "Checking nullable download size for url {}.",
            &url
        );
        let size_result = resp
            .headers()
            .get(CONTENT_LENGTH)
            .map(|header| -> Result<&str> {
                let header_str = header.to_str().context(
                    "Error !, Can't convert response header CONTENT-LENGTH header to string",
                )?;
                Ok(header_str)
            });

        if let Some(size) = size_result {
            info!(name = "download_size_ready", "Got download size.");
            size_info = Some(size?.trim().parse::<usize>().context(
                "Error !, Can't convert  file size from http header to usize object header",
            )?);
        } else {
            warn!(
                name = "no_download_size",
                "No name for url {} ,in http header Content-Length",
                &url
            );
        }

        debug!(
            name = "nullable_type_result",
            "Checking nullable download type for url {}.",
            &url
        );
        let content_type_result: Option<Result<String>> =
            resp.headers()
                .get(CONTENT_TYPE)
                .map(|header| -> Result<String> {
                    header.to_str().map(|s| s.to_string()).context(
                        "Error !, Can't convert response header CONTENT-TYPE header to string",
                    )
                });

        if let Some(content_type_result) = content_type_result {
            let content_type = content_type_result?;
            info!(
                name = "download_type_ready",
                "Got download content type {}", &content_type
            );
            content_type_info = Some(content_type);
        } else {
            warn!(
                name = "no_download_type",
                "No type for url {} ,in http header Content-Type",
                &url
            );
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

#[tokio::test]
async fn test_simple_download() -> Result<()> {
    match HttpAdapter::new(HttpConfig::default(), &RetryConfig::default()) {
        Ok(mut client) => {
            if let Ok(url) = Url::parse("https://ipv4.download.thinkbroadband.com/5MB.zip") {
                let mut streams: Vec<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>> =
                    vec![];
                let mut stream_handles = vec![];
                for id in 1..=3 {
                    let mut stream_tuple = client
                        .get_bytes(url.clone(), 1024)
                        .context(format!("Can't get bytes for stream {id}"))?;
                    streams.push(stream_tuple.0);
                    stream_handles.push(stream_tuple.1);
                }

                for (idx, mut stream) in streams.into_iter().enumerate() {
                    while let Some(Ok(part)) = stream.next().await {
                        info!("stream number {}\n: {:?}", idx, part);
                    }
                }

                info!("Waiting for streams!");
                for handle in stream_handles {
                    handle.await?;
                }

                info!("Shutdown successful!");
                return Ok(());
            }
        }

        Err(err) => {
            error!(error = %err, "Error creating http client");
            return Err(err);
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_get_bytes() -> Result<()> {
    match HttpAdapter::new(HttpConfig::default(), &RetryConfig::default()) {
        Ok(mut client) => {
            let url = Url::parse("https://ipv4.download.thinkbroadband.com/5MB.zip")
                .context("Invalid url.")?;
            let mut streams: Vec<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>> =
                vec![];
            let mut stream_handles = vec![];

            for _ in 1..=3 {
                let mut stream_tuple = client
                    .get_bytes_range(url.clone(), &[0, 10000], 1024)
                    .context("can't get bytes")?;
                streams.push(stream_tuple.0);
                stream_handles.push(stream_tuple.1);
            }

            for (idx, mut stream) in streams.into_iter().enumerate() {
                while let Some(Ok(part)) = stream.next().await {
                    info!("stream number {}\n: {:?}", idx, part);
                }
            }

            info!("Waiting for streams!");
            for handle in stream_handles {
                handle.await?;
            }

            info!("Shutdown successful!");
            return Ok(());
        }

        Err(err) => {
            error!(error = %err, "Error creating http client");
            return Err(err);
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_retry_check_name() -> Result<()> {
    use tracing::Level;
    use crate::utils::test_logger_init;   
    // Use a small retry configuration for tests to avoid long blocking on network failures
    test_logger_init(Level::DEBUG);
    let retry_config = RetryConfig::new(1, 1);
    match HttpAdapter::new(HttpConfig::default(), &retry_config) {
        Ok(client) => {
            let url = Url::parse("http://speedtest.tele2.net/1MB.zip")?;
            let info = client.get_info(url).await?;
            let name = info.name().clone().unwrap_or("No name !".into());
            let date = info.download_date();
            let download_type = info.download_type().clone().unwrap_or("No type !".into());
            let size = info.size().unwrap_or(0);
            println!("name:{name},date:{date},download type:{download_type},size:{size}",);
            return Ok(());
        }

        Err(err) => {
            error!(error = %err, "Error creating http client");
            return Err(err);
        }
    }

    return Ok(());
}