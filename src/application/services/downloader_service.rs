use std::{num::NonZeroUsize, path::PathBuf, sync::Arc};
use tokio::fs::OpenOptions;
use anyhow::{Context, Result};
use lru::LruCache;
use tokio::{fs::File, sync::Mutex};
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, instrument, warn};
use url::Url;

use crate::domain::services::download::{DownloadFile, ProgressFile};
use crate::application::services::progress_service::DefaultProgressTracker;
use crate::domain::ports::progress_tracker::ProgressTracker;
use crate::domain::services::infer_name::DownloadName;
use crate::infra::config::HttpConfig;
use crate::infra::config::RetryConfig;
use crate::infra::network::http_adapter::HttpAdapter;
use dirs::home_dir;
use sanitize_filename::sanitize;

struct CliService{
    urls: Vec<Url>,
    cache_dir: Option<PathBuf>,
    download_dir: Option<PathBuf>,
    http_config: HttpConfig,
    retry_config: RetryConfig,
    handles_cache: Arc<Mutex<LruCache<PathBuf, File>>>,
}

impl CliService{
    #[instrument(skip_all, fields(urls_count = urls.len()))]
    fn new(urls: Vec<Url>, download_dir: Option<PathBuf>, cache_dir: Option<PathBuf>, http_config: HttpConfig, retry_config: RetryConfig) -> Self {
        let cap = NonZeroUsize::new(50).expect("LRU capacity must be non-zero");
        let lru_cache = LruCache::new(cap);
        debug!("Initialized CliService with {} URLs", urls.len());
        Self { 
            urls, 
            cache_dir, 
            download_dir, 
            http_config, 
            retry_config,
            handles_cache: Arc::new(Mutex::new(lru_cache)) 
        }
    }
    #[instrument(skip_all)]
    async fn init(&mut self) -> Result<()> {
        let cache_dir = self.checked_cliant_dir().await?;
        let http_config = self.http_config.clone();
        let retry_config = self.retry_config.clone();
        
        info!("Starting concurrent downloads for {} URLs", self.urls.len());

        // spawn one task per url to perform the download concurrently
        for url in self.urls.clone() {
            let cache_dir_clone = cache_dir.clone();
            let http_cfg = http_config.clone();
            let retry_cfg = retry_config.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::download_single(url, cache_dir_clone, http_cfg, retry_cfg).await {
                    error!(error=%e, "download task failed");
                }
            });
        }
        
        debug!("All download tasks spawned");
        Ok(())
    }
    
    /// Handles a single URL download with proper filename resolution using DownloadName module.
    #[instrument(skip_all, fields(url=%url))]
    async fn download_single(url: Url, cache_dir: PathBuf, http_config: HttpConfig, retry_config: RetryConfig) -> Result<()> {
        // Create HTTP adapter and use DownloadName to resolve filename (handles percent-encoding, fallback inference)
        let mut adapter = HttpAdapter::new(http_config.clone(), &retry_config)
            .context("Failed to create HTTP adapter")?;
        
        let mut download_name = DownloadName::new(&mut adapter);
        let filename = download_name.get(url.clone()).await
            .context("Failed to infer download name")?
            .map(|cow| cow.into_owned())
            .unwrap_or_else(|| "download".to_string());
        
        debug!("Resolved filename: {}", filename);
        
        // Sanitize filename to prevent path traversal and invalid chars
        let sanitized_filename = sanitize(&filename);
        let file_path = cache_dir.join(&sanitized_filename);
        
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
        let download_file = DownloadFile::new(file, url.clone());
        let download_arc = Arc::new(Mutex::new(download_file));

        let multipart = StartMultiPart::new(url, download_arc, Arc::new(http_config), sanitized_filename.clone(), retry_config);

        if let Err(e) = multipart.start().await {
            error!(error=%e, "multipart download failed");
            return Err(e);
        }
        
        info!("Download completed: {}", sanitized_filename);
        Ok(())
    }

    async fn check_download_dir(&mut self) -> Result<PathBuf> {
        let option_download_dir = self.download_dir.clone().or_else(|| {
            if let Some(home_dir) = home_dir() {
                let download_dir = home_dir.join("Downloads");
                debug!("No user defined download directory, using directory:{:?}", download_dir);
                return Some(download_dir);
            }
            None
        });
        let download_dir = option_download_dir.context("Can't get user download directory")?;
        debug!("Making sure download directory {:?} exists ...", &download_dir);
        tokio::fs::create_dir_all(&download_dir).await?;
        Ok(download_dir)
    }

    async fn checked_cliant_dir(&mut self) -> Result<PathBuf> {
        let option_home_dir = self.cache_dir.clone().or_else(|| {
            if let Some(new_dir) = home_dir() {
                debug!("No user defined cache directory, using directory:{:?}", new_dir);
                return Some(new_dir);
            }
            None
        });
        let home_dir = option_home_dir.context("Can't get user home directory")?;
        let cache_dir = home_dir.join(".cliant_cache");
        debug!("Making sure cache directory {:?} exists ...", &cache_dir);
        tokio::fs::create_dir_all(&cache_dir).await?;
        Ok(cache_dir)
    }

}

struct StartMultiPart {
    url: Url,
    download_handle: Arc<Mutex<DownloadFile<File>>>,
    http_config: Arc<HttpConfig>,
    filename: String,
    retry_config: RetryConfig,
}

impl StartMultiPart {
    fn new(url: Url, download_handle: Arc<Mutex<DownloadFile<File>>>, http_config: Arc<HttpConfig>, filename: String, retry_config: RetryConfig) -> Self {
        Self {
            url,
            download_handle,
            http_config,
            filename,
            retry_config,
        }
    }

    #[instrument(skip(self), fields(url=%self.url))]
    async fn start(&self) -> Result<()> {
        debug!("Starting multipart download");

        // bring download traits into scope for trait methods
        use crate::domain::ports::download_service::{DownloadInfoService, SimpleDownload};

        // create adapter
        let mut adapter = HttpAdapter::new(self.http_config.as_ref().clone(), &self.retry_config)
            .context("Failed to create HTTP adapter for multipart")?;

        // try to get info (size, name etc)
        let file_info = adapter.get_info(self.url.clone()).await.context("Failed getting download info")?;

        // Determine chunking
        let part_size: usize = 128 * 1024; // 128 KiB per part
        let size = match file_info.size() {
            Some(s) => *s,
            None => {
                // fallback: run a single-stream download using adapter.get_bytes
                let (mut stream, handle) = adapter.get_bytes(self.url.clone(), 16 * 1024)?;
                // open a separate file handle for appending
                let mut f = tokio::fs::OpenOptions::new().append(true).open(self.filename_path()).await?;
                while let Some(chunk_res) = stream.next().await {
                    let chunk = chunk_res?;
                    f.write_all(chunk.as_ref()).await?;
                }
                // wait for background handle
                handle.await?;
                return Ok(());
            }
        };

        // compute ranges
        let mut ranges: Vec<[usize; 2]> = Vec::new();
        let mut start = 0usize;
        while start < size {
            let end = (start + part_size - 1).min(size - 1);
            ranges.push([start, end]);
            start = end + 1;
        }

        let total_parts = ranges.len();

        let progress_file = ProgressFile::new(size, self.filename.clone());
        let progress_arc = Arc::new(Mutex::new(progress_file));

        let tracker: Arc<dyn ProgressTracker> = Arc::new(DefaultProgressTracker::new(size, total_parts));

        let adapter_arc = Arc::new(Mutex::new(adapter));

        // spawn all part tasks
        let mut handles = Vec::with_capacity(total_parts);
        for (part_id, range) in ranges.into_iter().enumerate() {
            let download_clone = self.download_handle.clone();
            let adapter_clone = adapter_arc.clone();
            let progress_clone = progress_arc.clone();
            let tracker_clone = tracker.clone();

            let h = tokio::spawn(async move {
                let mut df_guard = download_clone.lock().await;
                df_guard.fetch_part(16 * 1024, &range, part_id, adapter_clone, progress_clone, tracker_clone).await
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

        info!("Multipart download completed for {}", self.url);
        Ok(())
    }
}

impl StartMultiPart {
    // helper to expose a Path for append writing fallback
    fn filename_path(&self) -> PathBuf {
        // if filename is just a name, use current dir
        PathBuf::from(&self.filename)
    }
}

