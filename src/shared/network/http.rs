use std::time::Duration;
use tracing::{info,warn};
use anyhow::{Context, Result};
use reqwest_tracing::TracingMiddleware;
use tokio::sync::mpsc::channel;
use tracing::{error, instrument};
use url::Url;

use super::http_args::HttpArgs;
use crate::shared::{errors::CliantError, network::DataTransport};
use bytes::Bytes;
use reqwest::{Client, header::CONTENT_LENGTH};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream};

pub struct HttpAdapter {
    client: ClientWithMiddleware,
}

impl HttpAdapter {
    #[instrument(name="new_http_adapter",fields(config=format!("{:?}\n", http_args,)))]
    pub fn new(http_args: HttpArgs) -> Result<Self> {
        let retry_args = http_args.retry_args.unwrap_or_default();
        let delay_secs = *retry_args.retry_delay_secs();
        let max_retry_bound = delay_secs.max(2);
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(
                Duration::from_secs(1),
                Duration::from_secs(max_retry_bound as u64),
            )
            .build_with_max_retries(*retry_args.max_no_retries() as u32);
        let retry_middleware =
            RetryTransientMiddleware::new_with_policy(retry_policy); // Enable retry with exponential backoff.
        let try_client = Client::try_from(http_args)
            .context("Can't create http client due to misconfiguration.")?;
        let client: ClientWithMiddleware = ClientBuilder::new(try_client)
            .with(TracingMiddleware::default()) // Enable built-in http client tracing and logging.
            .with(retry_middleware)
            .build();

        Ok(Self { client })
    }
}

impl DataTransport for HttpAdapter {
    async fn receive_data(
        &self,
        source: url::Url,
    ) -> Result<impl Stream<Item = Result<Bytes, CliantError>>, CliantError>
    {
        let (tx, rx) = channel(256);
        match self.client.get(source.clone()).send().await {
            Ok(mut resp) => {
                loop {
                    match resp.chunk().await {
                        Ok(Some(bytes)) => {
                            if let Err(err) = tx.send(Ok(bytes)).await {
                                error!(error = %err, "Error sending bytes to channel");
                                return Err(CliantError::Fatal(
                                    "Error sending bytes to channel".into(),
                                ));
                            }
                        }
                        Ok(None) => {
                            break;
                        }
                        Err(err) => {
                            //Propagate error to sender to handle it.
                            error!(error = %err, "Error sending error to channel");
                            return Err(CliantError::ReqwestClient(err));
                        }
                    }
                }
            }
            Err(err) => {
                error!("could'nt download {source} due to :{err}");
                return Err(CliantError::ReqwestMiddleware(err));
            }
        }
        Ok(ReceiverStream::new(rx))
    }
    async fn total_bytes(&self,source:url::Url)->Result<Option<usize>,CliantError> {
        let resp=self.client.head(source.clone()).send().await?;
        let size_result = resp
            .headers()
            .get(CONTENT_LENGTH)
            .map(|header| -> Result<&str> {
                let header_str = header.to_str().context(
                    "Error !, Can't convert response header CONTENT-LENGTH header to string",
                )?;
                Ok(header_str)
            });
        

        let size_info=if let Some(size) = size_result {
            info!(name = "download_size_ready", "Got download size.");
            
            Some(size?.trim().parse::<usize>().map_err(
                |err| CliantError::ParseError(format!("Error !, Can't convert  file size from http header to usize object header,caused by:{err}")),
            )?)
            
        } else {
            warn!(
                name = "no_download_size",
                "Can't get download size for url {} ,in http header Content-Length", &source
            );
            None
        };
        Ok(size_info)

    }
}

#[tokio::test]
async fn test_download() -> Result<()> {
    use url::Url;
    let adapter = HttpAdapter::new(HttpArgs::default())?;
    let source = Url::parse("http://speedtest.tele2.net/1MB.zip")?;
    let stream_result = adapter.receive_data(source).await;
    match stream_result {
        Ok(mut stream) => {
            let next_stream: std::result::Result<Option<Bytes>, CliantError>=stream.try_next().await;
            assert!(next_stream.is_ok());
            assert!(next_stream.unwrap().is_some()); // safe to call unwrap here it won't panic.
        }

        Err(err) => {
            println!("something happened :{err}");
        }
    }
    Ok(())
}
