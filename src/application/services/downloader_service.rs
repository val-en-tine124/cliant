use anyhow::{Context, Result};
use lru::LruCache;
use std::{num::NonZeroUsize, path::PathBuf, sync::Arc};
use tokio::fs::OpenOptions;
use tokio::io::BufWriter;
use tokio::io::{AsyncSeek, AsyncWrite};
use tokio::{fs::File, sync::Mutex};
use tracing::{debug, error, info, instrument, warn};
use url::Url;

use crate::application::dto::{DownloadResponse, DownloadStatus};
use crate::application::services::progress_service::CliProgressTracker;
use crate::domain::models::DownloadInfo;
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
struct CliService {
    urls: Vec<Url>,
    output_file: Option<PathBuf>,
    download_dir: Option<PathBuf>,
    http_config: HttpConfig,
    retry_config: RetryConfig,
    handles_cache: Arc<Mutex<LruCache<PathBuf, File>>>,
    progress_tracker_single: Arc<dyn ProgressTracker>,
    progress_tracker_multi: Arc<dyn ProgressTracker>,
}

impl CliService {
    #[instrument(skip_all, fields(urls_count = urls.len()))]
    fn new(
        urls: Vec<Url>,
        download_dir: Option<PathBuf>,
        output_file: Option<PathBuf>,
        http_config: HttpConfig,
        retry_config: RetryConfig,
        progress_tracker_single: Arc<dyn ProgressTracker>,
        progress_tracker_multi: Arc<dyn ProgressTracker>,
    ) -> Self {
        let cap =
            NonZeroUsize::new(50).expect("LRU capacity must be non-zero");
        let lru_cache = LruCache::new(cap);
        debug!("Initialized CliService with {} URLs", urls.len());
        Self {
            urls,
            output_file,
            download_dir,
            http_config,
            retry_config,
            handles_cache: Arc::new(Mutex::new(lru_cache)),
            progress_tracker_single,
            progress_tracker_multi,
        }
    }
    #[instrument(skip_all)]
    async fn start_download(&mut self) -> Result<Vec<DownloadResponse>> {
        let http_config = self.http_config.clone();
        let retry_config = self.retry_config;
        let mut handles: Vec<tokio::task::JoinHandle<DownloadResponse>> =
            vec![];
        info!("Starting concurrent downloads for {} URLs", self.urls.len());

        // spawn one task per url to perform the download concurrently
        for url in self.urls.clone() {
            let cache_dir_clone = self.output_file.clone();
            let http_cfg = http_config.clone();
            let retry_cfg = retry_config;
            info!("Start {url} Download task...");
            let handle = tokio::spawn(async move {
                match Self::download_single(
                    url,
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
            handles.push(handle);
        }
        debug!("All download tasks spawned");
        info!("Waiting for spawned tasks ...");
        let mut download_resps = vec![];
        for handle in handles {
            download_resps.push(handle.await?);
        }

        Ok(download_resps)
    }

    /// Handles a single URL download with proper filename resolution using `DownloadName` module.
    #[instrument(skip_all, fields(url=%url))]
    async fn download_single(
        url: Url,
        output_file: Option<PathBuf>,
        http_config: HttpConfig,
        retry_config: RetryConfig,
    ) -> Result<DownloadResponse> {
        // Create HTTP adapter and use DownloadName to resolve filename (handles percent-encoding, fallback inference)
        let mut adapter = HttpAdapter::new(http_config.clone(), &retry_config)
            .context("Failed to create HTTP adapter")?;
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
                cwd.join(&filename)
            } else {
                error!("Can't get current working directory");
                std::process::exit(1);
            }
        });

        // Make sure file_path exists and is valid.
        tokio::fs::create_dir_all(&file_path).await?;

        info!("Creating/opening file at: {:?}", file_path);

        // Create or open file for writing
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&file_path)
            .await
            .context("Failed to create/open file for download")?;

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
                filename.clone(),
                download_info.clone(),
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
    download_info: DownloadInfo,
    sanitized_filename: String,
    retry_config: RetryConfig,
}

impl<W: AsyncWrite + AsyncSeek + Unpin + 'static + Send> StartMultiPart<W> {
    fn new(
        url: Url,
        download_handle: Arc<Mutex<BufWriter<W>>>,
        http_config: Arc<HttpConfig>,
        sanitized_filename: String,
        download_info: DownloadInfo,
        retry_config: RetryConfig,
    ) -> Self {
        Self {
            url,
            download_handle,
            http_config,
            download_info,
            sanitized_filename,
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
