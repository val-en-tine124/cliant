use anyhow::{Context, Result};
use std::{ path::PathBuf, sync::Arc};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeek, AsyncWrite, BufWriter};
use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument, warn};
use url::Url;
use crate::application::dto::{DownloadResponse, DownloadStatus};
use crate::application::services::progress_service::CliProgressTracker;
use crate::domain::models::DownloadInfo;
use crate::domain::ports::download_service::{
    DownloadInfoService, MultiPartDownload, SimpleDownload,
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
        
        // Unwrap user output path or default to current working directory.
        let mut file_path = output_file.clone().unwrap_or_else(|| {
            let cwd = std::env::current_dir().unwrap_or_else(|_| {
                error!("Can't get current working directory");
                std::process::exit(1);
            });
            cwd.join(&filename)
        });

        // If the inferred filename is "download" and the user provided an output directory,
        // we should append the inferred filename to that directory.
        if filename == "download" && output_file.is_some() && file_path.is_dir() {
            file_path = file_path.join("download");
        }
        
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

        // Determine the progress file path
        let progress_file_path = file_path.with_extension("progress");

        // Load existing progress or create a new one
        let progress_file = if progress_file_path.exists() {
            info!("Resuming download. Loading progress file from: {:?}", progress_file_path);
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true) // Should exist, but create if not (e.g., first run after deletion)
                .open(&progress_file_path)
                .await?;
            ProgressFile::load_progress(&ProgressFile::new(0, "".to_string()), &mut file).await?
        } else {
            info!("Starting new download. Creating new progress file at: {:?}", progress_file_path);
            ProgressFile::new(
                download_info.size().unwrap_or(0), // Use actual size if available
                filename.clone(),
            )
        };

        info!("Creating/opening file at: {:?}", file_path);
        let file =OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .append(true) // Do not truncate, append to existing file for resumption
            .open(&file_path)
            .await?;
            // .context("Failed to create/open file for download")?;
        

        // Create DownloadFile that owns the writer and wrap it for concurrent access
        let buf_writer = BufWriter::with_capacity(128 * 1024, file);
        let download_arc = Arc::new(Mutex::new(buf_writer));
        if let (Some(_), Some(_)) =
            (download_info.size(), download_info.name())
        {
            let progress_file_arc = Arc::new(Mutex::new(progress_file));
            let multipart = StartMultiPart::new(
                url.clone(),
                download_arc,
                Arc::new(http_config),
                retry_config,
                progress_file_arc.clone(),
                progress_file_path,
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

struct StartMultiPart {
    url: Url,
    download_handle: Arc<Mutex<BufWriter<tokio::fs::File>>>,
    http_config: Arc<HttpConfig>,
    retry_config: RetryConfig,
    progress_file: Arc<Mutex<ProgressFile>>,
    progress_file_path: PathBuf,
}

impl StartMultiPart {
    fn new(
        url: Url,
        download_handle: Arc<Mutex<BufWriter<tokio::fs::File>>>,
        http_config: Arc<HttpConfig>,
        retry_config: RetryConfig,
        progress_file: Arc<Mutex<ProgressFile>>,
        progress_file_path: PathBuf,
    ) -> Self {
        Self {
            url,
            download_handle,
            http_config,
            retry_config,
            progress_file,
            progress_file_path,
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
                self.progress_file.clone(),
                self.progress_file_path.clone(),
            )
            .await?;
        } else {
            // write to writer using the write_stream helper
            //let tracker: Arc<dyn ProgressTracker> = Arc::new(DefaultProgressTracker::new(0, 1));
            do_single(self.url.clone(), self.download_handle.clone(), adapter, file_info)
                .await?;
        }

        Ok(())
    }
}
async fn do_multi_part(
    url: Url,
    filename: String,
    file_size: usize,
    part_size: usize,
    writer: Arc<Mutex<BufWriter<tokio::fs::File>>>,
    http_adapter: HttpAdapter,
    progress_file_arc: Arc<Mutex<ProgressFile>>,
    progress_file_path: PathBuf,
) -> Result<()>
{
    // compute ranges
    let mut ranges: Vec<[usize; 2]> = Vec::new();
    let mut start = 0usize;
    while start < file_size {
        let end = (start + part_size - 1).min(file_size - 1);
        ranges.push([start, end]);
        start = end + 1;
    }

    let progress_file_locked = progress_file_arc.lock().await;
    let completed_chunks = progress_file_locked.load_chunk().clone();
    drop(progress_file_locked); // Release the lock immediately after cloning

    let mut ranges_to_download: Vec<[usize; 2]> = Vec::new();
    for range in ranges {
        let mut is_completed = false;
        for completed_range in &completed_chunks {
            // Check if the current range is fully contained within a completed chunk
            if range[0] >= completed_range.0 && range[1] <= completed_range.1 {
                is_completed = true;
                break;
            }
        }
        if !is_completed {
            ranges_to_download.push(range);
        }
    }

    let total_parts = ranges_to_download.len();

    let progress_file_for_tracker = ProgressFile::new(file_size, filename.clone());
    let _progress_arc_for_tracker = Arc::new(Mutex::new(progress_file_for_tracker));

    let tracker: Arc<dyn ProgressTracker> =
        Arc::new(CliProgressTracker::new(file_size, total_parts));

    let adapter_arc = Arc::new(http_adapter);

    // spawn all part tasks
    let mut handles = Vec::with_capacity(total_parts);
    for (part_id, range) in ranges_to_download.into_iter().enumerate() {
        let download_clone = writer.clone();
        let adapter_clone = adapter_arc.clone();
        let progress_clone = progress_file_arc.clone(); // Pass the main progress_file_arc
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
                progress_clone, // Use the main progress_file_arc here
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

    // Periodically save the progress file
    let save_progress_handle = tokio::spawn({
        let progress_file_arc = progress_file_arc.clone();
        let progress_file_path = progress_file_path.clone();
        async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await; // Save every 5 seconds
                let progress_file_locked = progress_file_arc.lock().await;
                let mut file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&progress_file_path)
                    .await?;
                progress_file_locked.save_progress(&mut file).await?;
                info!("Progress file saved to {:?}", progress_file_path);
            }
            #[allow(unreachable_code)]
            Ok::<(), anyhow::Error>(())
        }
    });

    // finalize
    tracker.finish().await;
    save_progress_handle.abort(); // Stop the saving task

    // Remove the progress file on successful completion
    tokio::fs::remove_file(&progress_file_path).await?;
    info!("Progress file {:?} removed.", progress_file_path);

    info!("Multipart download completed for {}", url);
    Ok(())
}
async fn do_single<H>(
    url: Url,
    writer: Arc<Mutex<BufWriter<tokio::fs::File>>>,
    http_adapter: H,
    file_info: DownloadInfo,
) -> Result<()>
where
    H: SimpleDownload + MultiPartDownload + 'static,
{
    let tracker: Arc<dyn ProgressTracker> =
        Arc::new(CliProgressTracker::new(0, 0));

    let current_file_size = writer.lock().await.get_ref().metadata().await?.len() as usize;
    let mut start_offset = 0;

    if let Some(total_size) = file_info.size() {
        if current_file_size > 0 && current_file_size < *total_size {
            info!("Resuming single-part download from offset: {}", current_file_size);
            start_offset = current_file_size;
        }
    }

    let (stream, handle) = if start_offset > 0 {
        let end_offset = file_info.size().map(|s| s - 1).unwrap_or(0);
        let range = [start_offset, end_offset];
        info!("Requesting bytes range: {:?}", range);
        http_adapter.get_bytes_range(url.clone(), &range, 16 * 1024)?
    } else {
        http_adapter.get_bytes(url.clone(), 16 * 1024)?
    };

    write_stream(writer, stream, 0, tracker,).await?;
    // wait for background handle
    handle.await?;
    Ok(())
}