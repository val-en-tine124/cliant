use crate::domain::{
    models::DownloadInfo,
    ports::download_service::{DownloadInfoService, MultiPartDownload},
};
use anyhow::Result;
use bytes::Bytes;
use infer;
use percent_encoding::percent_decode_str;
use sanitize_filename::sanitize;
use std::borrow::Cow;
use std::path::Path;
use tokio_stream::StreamExt;
use tracing::{debug, error, instrument};
use url::Url;

pub fn get_extension(buf: &Bytes) -> Option<&'static str> {
    let inferred_type = infer::get(buf);
    if let Some(inferred_type) = inferred_type {
        return Some(inferred_type.extension());
    }
    None
}

///Struct representing a download name.
pub struct DownloadName<'a, T> {
    download_service: &'a mut T,
}

impl<'a, T: MultiPartDownload + DownloadInfoService> DownloadName<'a, T> {
    pub fn new(download_service: &'a mut T) -> Self {
        Self { download_service }
    }

    ///This method only works for protocols that implements ``MultiPartDownload`` trait.
    #[instrument(name = "infer_name", skip(self,))]
    pub async fn get_or_parse(
        &mut self,
        url: Url,
    ) -> Result<Option<Cow<'_, str>>> {
        let url_clone = url.clone();
        let info = self.download_service.get_info(url_clone.clone()).await?;
        if let Some(name) = info.name() {
            let name_string = name.clone();
            return Ok(Some(Cow::from(name_string)));
        }
        // Try to infer from the URL path segment (decoding percent-encoding)
        if let Some(seg) =
            url_clone.path_segments().and_then(std::iter::Iterator::last)
        {
            // Try to decode percent-encoded UTF-8 strictly first, then fallback to lossy
            let decoded = percent_decode_str(seg).decode_utf8().map_or_else(
                |_| percent_decode_str(seg).decode_utf8_lossy().into_owned(),
                std::borrow::Cow::into_owned,
            );
            if !decoded.is_empty() {
                // If there's an extension, return it immediately
                if Path::new(&decoded).extension().is_some() {
                    return Ok(Some(Cow::from(decoded)));
                }
                // Otherwise, fall through to try to infer from bytes
            }
        }
        let mut buffer = Vec::with_capacity(2048);
        let mut total_read = 0usize;
        match self.download_service.get_bytes_range(
            info.url().clone(),
            &[0, 2048],
            2048,
        ) {
            Ok((mut stream, handle)) => {
                while let Some(chunk_result) = stream.next().await {
                    //Iterate over stream generator.
                    let chunk = chunk_result?;
                    debug!("Copying bytes from Reader to writer...");
                    let to_copy = (2048 - total_read).min(chunk.len());
                    buffer.extend_from_slice(&chunk.as_ref()[..to_copy]);
                    total_read += to_copy;
                    if total_read >= 2048 {
                        break;
                    }
                }

                let _ = handle.await; //Make sure the stream fetch has been completed.

                if let Some(ext) =
                    get_extension(&Bytes::copy_from_slice(&buffer))
                {
                    let random_no: u32 = rand::random::<u32>();
                    // If we had a path segment that lacked an extension, use that name + inferred ext
                    if let Some(seg) =
                        url.path_segments().and_then(std::iter::Iterator::last)
                    {
                        let decoded = percent_decode_str(seg)
                            .decode_utf8_lossy()
                            .to_string();
                        if !decoded.is_empty()
                            && Path::new(&decoded).extension().is_none()
                        {
                            let download_name = format!("{decoded}.{ext}");
                            return Ok(Some(Cow::from(download_name)));
                        }
                    }
                    let download_name = format!("{random_no}.{ext}");
                    // Sanitize filename to prevent path traversal and invalid chars
                    let cow_dname = Cow::from(sanitize(download_name));

                    return Ok(Some(cow_dname));
                }
                Ok(None)
            }
            Err(error) => {
                error!("Error:{}", error);
                Ok(None)
            }
        }
    }
}

#[tokio::test]
async fn test_download_name() -> Result<()> {
    use crate::infra::config::{HttpConfig, RetryConfig};
    use crate::infra::network::http_adapter::HttpAdapter;
    use crate::utils::test_logger_init;
    use tracing::{Level, info};

    test_logger_init(Level::DEBUG);
    if let Ok(url) =
        url::Url::parse("http://ipv4.download.thinkbroadband.com/5MB.zip")
    {
        let mut adapter =
            HttpAdapter::new(HttpConfig::default(), &RetryConfig::default())
                .expect("No adapter");
        let mut d_name = DownloadName::new(&mut adapter);
        if let Some(name) = d_name.get_or_parse(url).await? {
            info!("Got! name {name}",);
        } else {
            info!("Can't get name.");
        }
    }
    Ok(())
}
