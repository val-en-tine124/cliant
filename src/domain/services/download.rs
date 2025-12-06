//! This module contains Object for running download tasks.

use crate::domain::ports::download_service::MultiPartDownload;
use crate::domain::ports::progress_tracker::ProgressTracker;
use anyhow::Result;
use chrono::{DateTime, Local};
use derive_getters::Getters;
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, SeekFrom};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{
    AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufWriter,
};
use tokio::sync::Mutex;
use tokio_stream::{Stream, StreamExt};
use tracing::{debug, info, instrument};
use url::Url;
#[allow(unused)]
type BytesStream =
    Pin<Box<dyn Stream<Item = Result<bytes::Bytes, anyhow::Error>> + std::marker::Send + 'static>>;
#[allow(unused)]
const CHUNKSIZE: usize = 1024;

///This is the progress file that can get serialized
/// and deserialized on-demand for the purpose of tracking download progress
#[derive(Deserialize, Serialize, Getters, Setters, Debug)]
struct Progress {
    /// Path of the download.
    #[setters(skip)]
    path: PathBuf,
    /// Name of the download file.
    #[setters(skip)]
    download_name: String,
    ///Total download file size.
    #[setters(skip)]
    total_size: usize,
    ///Completed download segments or chunks.
    #[setters(rename = "completed_fragments")]
    completed_chunks: Vec<(usize, usize)>,
    ///Date the download started.
    #[setters(skip)]
    started_on: DateTime<Local>,
}

impl Progress {
    fn new(path: PathBuf, total_size: usize, download_name: String) -> Self {
        Self {
            path,
            download_name,
            total_size,
            completed_chunks: vec![],
            started_on: Local::now(),
        }
    }
    ///This method will take a reader i.e a type implementing
    /// ``tokio::io::Reader`` and load json string
    /// representation of Progress type.
    #[instrument(name = "load_progress", skip(self, reader))]
    async fn load_progress<'a, R>(&self, reader: &'a mut R) -> Result<Progress>
    where
        R: AsyncRead + AsyncSeek + Unpin,
    {
        let mut buf = String::new();
        // Seek back to start because write advances cursor to end
        debug!("Seeking to position 0 on Writer...");
        reader.seek(SeekFrom::Start(0)).await?;
        let bytes_count = reader.read_to_string(&mut buf).await?;
        debug!("Read {bytes_count} of progress report to String buffer.");
        debug!(
            "Loading download Progress Object information from string buffer {:?}",
            &buf
        );

        let progress: Progress = serde_json::from_str(buf.trim())?;

        Ok(progress)
    }
    ///This method will take a reader i.e a type implementing
    /// ``tokio::io::Writer`` and write to the writer a json string
    /// representation of Progress type.
    #[instrument(name = "load_progress", skip(self, writer))]
    async fn save_progress<W>(&self, writer: &mut W) -> Result<()>
    where
        W: AsyncWrite + AsyncSeek + Unpin,
    {
        let progress_json = serde_json::to_string(self)?;
        let mut progress_cursor = Cursor::new(progress_json.trim());
        // Seek back to start because read advances cursor to end
        debug!("Seeking to position 0 on Writer...");
        writer.seek(SeekFrom::Start(0)).await?;
        debug!("Writing download Progress Object in String buffer data to Writer...");
        writer.write_all_buf(&mut progress_cursor).await?;
        debug!("Flushing data in writer buffer... ");
        writer.flush().await?;

        Ok(())
    }
}
///This function will take a ``size``
/// And generate a vector chunks size ``[start, end]``.
fn generate_chunk(size: usize) -> Vec<[usize; 2]> {
    let mut handle: Vec<[usize; 2]> = Vec::new();
    for start in (0..size).step_by(CHUNKSIZE) {
        let end = (start + CHUNKSIZE - 1).min(size - 1);
        handle.push([start, end]);
    }
    handle
}

#[derive(Getters)]
///DownloadFile object abstracts operations e.g multipart operation on downloads
pub struct DownloadFile<W> {
    #[getter(skip)]
    writer: BufWriter<W>,
    ///This is the download url.
    url: Url,
}
impl<W> DownloadFile<W>
where
    W: AsyncWrite + AsyncSeek + Unpin,
{
    fn new(writer: W, url: Url) -> Self {
        let buf_writer = BufWriter::with_capacity(128 * 1024, writer);

        Self {
            writer: buf_writer,
            url,
        }
    }

    /// This method changes i.e seek The ``Writer`` cursors in other to
    /// write a download chunk fetched from an http server.
    /// ## Parameters:
    /// * ``downloader`` : Protocols that support multipart downloading.
    /// * ``range`` : Slice of integers for protocols that support multipart downloading.
    /// * ``buffer_size`` : Size of the in-memory buffer
    /// * ``part_id`` : Unique identifier for this part (for progress tracking)
    /// * ``tracker`` : Progress tracker for monitoring download progress
    #[instrument(name="fetch_part",skip(self,downloader,tracker),fields(buffer_size=buffer_size,range=format!("{:?}", range),part_id=part_id))]
    pub async fn fetch_part<D>(
        &mut self,
        buffer_size: usize,
        range: &[usize; 2],
        part_id: usize,
        downloader: Arc<Mutex<D>>,
        tracker: Arc<dyn ProgressTracker>,
    ) -> Result<()>
    where
        D: MultiPartDownload,
    {
        let [first, last] = *range;
        let part_size = last - first + 1;

        self.writer.seek(SeekFrom::Start(first as u64)).await?; // Let the cursor point to the the current range offset.
        let (mut stream, handle) =
            downloader
                .lock()
                .await
                .get_bytes_range(self.url.clone(), range, buffer_size)?;

        let mut bytes_written = 0;
        while let Some(chunk) = stream.next().await {
            let chunk_var = chunk?;
            bytes_written += chunk_var.len();

            self.writer.write_all(&chunk_var).await?;
            tracker.update(part_id, bytes_written).await;
            info!("Writing chunk of len {} to writer", chunk_var.len());
        }
        info!("Flushing chunks in writer buffer... ");
        self.writer.flush().await?;
        info!("Waiting for async chunk retrival task...");
        handle.await?;
        
        tracker.complete_part(part_id, part_size).await;

        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::{DownloadFile, Progress};
    use crate::application::services::progress_service::DefaultProgressTracker;
    use crate::domain::models::download_info::DownloadInfo;
    use crate::domain::ports::download_service::DownloadInfoService;
    use crate::domain::ports::progress_tracker::ProgressTracker;
    use crate::domain::services::download::generate_chunk;
    use crate::infra::config::HttpConfig;
    use crate::infra::{config::RetryConfig, network::http_adapter::HttpAdapter};
    use anyhow::Result;
    use std::sync::Arc;
    use tokio::fs::OpenOptions;
    use tokio::io::AsyncReadExt;
    use tokio::sync::Mutex;
    use url::Url;
    use tracing::{info,debug, Level};
    use crate::utils::test_logger_init;   
    
    
    #[tokio::test]
    async fn progress_file_test() -> Result<()> {
        test_logger_init(Level::DEBUG);
        let home_dir = std::env::home_dir().unwrap_or(std::env::current_dir()?);
        let progress_path = home_dir.join("My_Progress_File.json");
        info!("Progress path is {progress_path:?}");
        let total_size = 38560;
        let name = "My_video_file.mp4".to_string();
        let mut progress = Progress::new(home_dir, total_size, name);
        progress = progress.completed_fragments(vec![(34, 567)]);
        let mut handle = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(progress_path)
            .await?;
        let save_result = progress.save_progress(&mut handle).await;

        let mut buf = String::new();
        handle.read_to_string(&mut buf).await?;
        info!("content of in-memory buffer: {buf}",);

        assert!(save_result.is_ok());

        let load_result = progress.load_progress(&mut handle).await;
        assert!(load_result.is_ok());
        info!("Progress Object:{:?}", load_result?);
        Ok(())
    }
    #[tokio::test]
    async fn download_file_test() -> Result<()> {
        test_logger_init(Level::DEBUG);
        let url = Url::parse("http://speedtest.tele2.net/1MB.zip")?;
        let adapter = HttpAdapter::new(HttpConfig::default(), &RetryConfig::default())?;
        let file_info: DownloadInfo = adapter.get_info(url.clone()).await?;
        let arc_adapter = Arc::new(Mutex::new(adapter));
        if let Some(size) = file_info.size() {
            info!("download file size from server is {size}");
            let writer = async_tempfile::TempFile::new().await?;
            debug!("Writer path is :{}",writer.file_path().display());
            let range_vec = generate_chunk(*size);
            let download_file = DownloadFile::new(writer, url);
            let download_file_arc = Arc::new(Mutex::new(download_file));
            
            // Create progress tracker for monitoring download progress
            let tracker: Arc<dyn ProgressTracker> = Arc::new(DefaultProgressTracker::new(*size, range_vec.len()));
            let mut future_vec = vec![];

            for (part_id, range) in range_vec.into_iter().enumerate() {
                let download_file_clone = download_file_arc.clone();
                let arc_adapter_clone = arc_adapter.clone();
                let tracker_clone = tracker.clone();
                let handle = tokio::spawn(async move {
                    download_file_clone
                        .lock()
                        .await
                        .fetch_part(1024, &range, part_id, arc_adapter_clone, tracker_clone)
                        .await
                });
                future_vec.push(handle);
            }
            
            // Spawn a background task to monitor progress
            let tracker_clone = tracker.clone();
            let progress_monitor = tokio::spawn(async move {
                loop {
                    let progress = tracker_clone.total_progress().await;
                    info!(
                        "Progress: {}/{} bytes ({:.1}%) - {}/{} parts completed",
                        progress.downloaded_bytes,
                        progress.total_bytes,
                        progress.percentage(),
                        progress.completed_parts,
                        progress.total_parts
                    );
                    if progress.completed_parts >= progress.total_parts {
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            });
            
            for future in future_vec {
                let result = future.await?;
                assert!(result.is_ok());
            }
            
            // Wait for progress monitor to finish
            let _ = progress_monitor.await;
            
            // Mark download as finished
            tracker.finish().await;
            
            assert_eq!(
                *size,
                download_file_arc
                    .lock()
                    .await
                    .writer
                    .get_ref()
                    .metadata()
                    .await?
                    .len() as usize
            );
        }

        Ok(())
    }
}
