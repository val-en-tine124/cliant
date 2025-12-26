use anyhow::{Context, Result};
use std::{ path::PathBuf, sync::Arc};
use tokio::fs::OpenOptions;
use tokio::io::BufWriter;
use tokio::io::{AsyncSeek, AsyncWrite};
use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument, warn};
use url::Url;
use crate::application::dto::{DownloadResponse, DownloadStatus};
use crate::application::services::progress_service::CliProgressTracker;
use crate::domain::ports::download_service::{
    DownloadInfoService, SimpleDownload,
};
use crate::domain::ports::progress_tracker::ProgressTracker;
use crate::domain::services::download::{
    ProgressFile, fetch_part_parallel, write_stream,
};
use crate::domain::services::infer_name::DownloadName;
use crate::infra::config::HttpConfig;
use crate::infra::config::RetryConfig;
use crate::infra::network::http_adapter::HttpAdapter;

pub struct HttpCliService {
    url: Url,
    output_file: Option<PathBuf>,
    http_config: HttpConfig,
    retry_config: RetryConfig,
}

impl HttpCliService {
    #[instrument(skip_all,)]
    pub fn new(
        url: Url,
        output_file: Option<PathBuf>,
        http_config: HttpConfig,
        retry_config: RetryConfig,
    ) -> Self {
        debug!("Initialized HttpCliService with {} URL", url   );
        Self {
            url,
            output_file,
            http_config,
            retry_config,
        }
    }
    #[instrument(skip_all)]
    pub async fn start_download(&mut self) -> Result<DownloadResponse> {
        let http_config = self.http_config.clone();
        let retry_config = self.retry_config;

        // spawn one task per url to perform the download concurrently
        
        let cache_dir_clone = self.output_file.clone();
        let http_cfg = http_config.clone();
        let retry_cfg = retry_config;
        info!("Start {} Download task...", self.url.clone());
        let url_clone=self.url.clone();
        let handle = tokio::spawn(async move {
            match Self::download_single(
                url_clone,
                cache_dir_clone,
                http_cfg,
                retry_cfg,
            )
            .await
            {
                Ok(download_resp) => download_resp,
                Err(e) => {
                    error!(error=%e, "download task failed");
                    std::process::exit(1);
                }
            }
        });
        
        debug!("All download task has been spawned");
        info!("Waiting for spawned task ...");
        
        let download_resp=handle.await?;

        Ok(download_resp)
    }

    /// Handles a single URL download with proper filename resolution using `DownloadName` module.
    #[instrument(skip_all, fields(url=%url,))]
    async fn download_single(
        url: Url,
        output_file: Option<PathBuf>,
        http_config: HttpConfig,
        retry_config: RetryConfig,
    ) -> Result<DownloadResponse> {
        // Create HTTP adapter and use DownloadName to resolve filename (handles percent-encoding, fallback inference)
        debug!("Initializing http adapter ...");
        let mut adapter = HttpAdapter::new(http_config.clone(), &retry_config)
            .context("Failed to create HTTP adapter")?;
        debug!("Getting download information ...");
        let download_info = adapter.get_info(url.clone()).await?;
        let mut download_name = DownloadName::new(&mut adapter);
        let filename = download_name
            .get_or_parse(url.clone())
            .await
            .context("Failed to infer download name")?
            .map_or_else(
                || "download".to_string(),
                std::borrow::Cow::into_owned,
            );

        // Unwrap user output path or default to current working directory.
        
        let file_path = output_file.unwrap_or_else(|| {
            let cwd = std::env::current_dir();

            if let Ok(cwd) = cwd {
                let new_path=cwd.join(&filename);
                debug!("No user defined path using path :{new_path:?}");
                new_path

            } else {
                error!("Can't get current working directory");
                std::process::exit(1);
            }
        });
        //remove the file component of this path.
        let mut file_path_ancestors=file_path.clone();
        file_path_ancestors.pop();
        debug!("Creating directories in path {file_path_ancestors:?} recursively ...");
        // Making sure file_path exists and is valid.
        if !file_path_ancestors.try_exists()? {
            warn!(
                "The specified path {:?} does not exist. Creating directories...",
                file_path_ancestors
            );
            tokio::fs::create_dir_all(file_path_ancestors).await?;
        }
        

        info!("Creating/opening file at: {:?}", file_path);
        let file =OpenOptions::new()
            .truncate(true)
            .create(true)
            .read(true)
            .write(true)
            .open(&file_path)
            .await?;
            // .context("Failed to create/open file for download")?;
        

        // Create DownloadFile that owns the writer and wrap it for concurrent access
        let buf_writer = BufWriter::with_capacity(128 * 1024, file);
        let download_arc = Arc::new(Mutex::new(buf_writer));
        if let (Some(_), Some(_)) =
            (download_info.size(), download_info.name())
        {
            let multipart = StartMultiPart::new(
                url.clone(),
                download_arc,
                Arc::new(http_config),
                retry_config,
            );

            if let Err(e) = multipart.start().await {
                error!(error=%e, "multipart download failed");
                return Err(e);
            }

            info!("Download completed: {}", filename.clone());
        }
        let download_resp = DownloadResponse::new(
            Some(download_info),
            file_path,
            DownloadStatus::Success,
        );

        Ok(download_resp)
    }
}

struct StartMultiPart<W> {
    url: Url,
    download_handle: Arc<Mutex<BufWriter<W>>>,
    http_config: Arc<HttpConfig>,
    retry_config: RetryConfig,
}

impl<W: AsyncWrite + AsyncSeek + Unpin + 'static + Send> StartMultiPart<W> {
    fn new(
        url: Url,
        download_handle: Arc<Mutex<BufWriter<W>>>,
        http_config: Arc<HttpConfig>,
        retry_config: RetryConfig,
    ) -> Self {
        Self {
            url,
            download_handle,
            http_config,
            retry_config,
        }
    }

    #[instrument(skip(self), fields(url=%self.url))]
    async fn start(&self) -> Result<()> {
        debug!("Starting multipart download");

        // bring download traits into scope for trait methods
        use crate::domain::ports::download_service::DownloadInfoService;

        // create adapter
        let adapter = HttpAdapter::new(
            self.http_config.as_ref().clone(),
            &self.retry_config,
        )
        .context("Failed to create HTTP adapter for multipart")?;

        // try to get info (size, name etc)
        let file_info = adapter
            .get_info(self.url.clone())
            .await
            .context("Failed getting download info")?;

        // Determine chunking from http config or default
        let part_size: usize =
            self.http_config.multipart_part_size.unwrap_or(128 * 1024); // default 128 KiB per part
        if let (Some(s), Some(filename)) = (file_info.size(), file_info.name())
        {
            let file_size = *s;
            do_multi_part(
                self.url.clone(),
                filename.clone(),
                file_size,
                part_size,
                self.download_handle.clone(),
                adapter,
            )
            .await?;
        } else {
            // write to writer using the write_stream helper
            //let tracker: Arc<dyn ProgressTracker> = Arc::new(DefaultProgressTracker::new(0, 1));
            do_single(self.url.clone(), self.download_handle.clone(), adapter)
                .await?;
        }

        Ok(())
    }
}
async fn do_multi_part<W>(
    url: Url,
    filename: String,
    file_size: usize,
    part_size: usize,
    writer: Arc<Mutex<BufWriter<W>>>,
    http_adapter: HttpAdapter,
) -> Result<()>
where
    W: AsyncWrite + AsyncSeek + Unpin + 'static + Send,
{
    // compute ranges
    let mut ranges: Vec<[usize; 2]> = Vec::new();
    let mut start = 0usize;
    while start < file_size {
        let end = (start + part_size - 1).min(file_size - 1);
        ranges.push([start, end]);
        start = end + 1;
    }

    let total_parts = ranges.len();

    let progress_file = ProgressFile::new(file_size, filename.clone());
    let progress_arc = Arc::new(Mutex::new(progress_file));

    let tracker: Arc<dyn ProgressTracker> =
        Arc::new(CliProgressTracker::new(file_size, total_parts));

    let adapter_arc = Arc::new(http_adapter);

    // spawn all part tasks
    let mut handles = Vec::with_capacity(total_parts);
    for (part_id, range) in ranges.into_iter().enumerate() {
        let download_clone = writer.clone();
        let adapter_clone = adapter_arc.clone();
        let progress_clone = progress_arc.clone();
        let tracker_clone = tracker.clone();
        let url_clone = url.clone();
        let h = tokio::spawn(async move {
            fetch_part_parallel(
                download_clone,
                256 * 1024,
                range,
                part_id,
                url_clone,
                adapter_clone,
                progress_clone,
                tracker_clone,
            )
            .await
        });
        handles.push(h);
    }

    // await all
    for h in handles {
        let res = h.await?;
        if let Err(e) = res {
            error!(error=%e, "part download failed");
        }
    }

    // finalize
    tracker.finish().await;

    info!("Multipart download completed for {}", url);
    Ok(())
}
async fn do_single<H, W>(
    url: Url,
    writer: Arc<Mutex<BufWriter<W>>>,
    http_adapter: H,
) -> Result<()>
where
    H: SimpleDownload,
    W: AsyncWrite + AsyncSeek + Unpin + 'static,
{
    let tracker: Arc<dyn ProgressTracker> =
        Arc::new(CliProgressTracker::new(0, 0));
    // fallback: run a single-stream download using adapter.get_bytes
    let (stream, handle) = http_adapter.get_bytes(url.clone(), 16 * 1024)?;

    write_stream(writer, stream, 0, tracker).await?;
    // wait for background handle
    handle.await?;
    Ok(())
}