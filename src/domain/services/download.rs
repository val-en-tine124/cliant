//! This module contains Object for running download tasks.

use crate::domain::ports::download_service::MultiPartDownload;
use crate::domain::ports::progress_tracker::ProgressTracker;
use anyhow::Result;
use chrono::{DateTime, Local};
use derive_getters::Getters;
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, SeekFrom};
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


///This is the progress file that can get serialized
/// and deserialized on-demand for the purpose of tracking download progress
#[derive(Deserialize, Serialize, Getters, Setters, Debug)]
pub struct ProgressFile {
    /// Name of the download file.
    #[setters(skip)]
    download_name: String,
    ///Total download file size.
    #[setters(skip)]
    total_size: usize,
    ///Completed download segments or chunks.
    
    #[getter(rename = "load_chunk")]
    completed_chunks: Vec<(usize, usize)>,
    ///Date the download started.
    #[setters(skip)]
    started_on: DateTime<Local>,
}

impl ProgressFile {
    pub fn new(total_size: usize, download_name: String) -> Self {
        Self {
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
    pub async fn load_progress<R>(&self, reader: &mut R) -> Result<ProgressFile>
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

        let progress: ProgressFile = serde_json::from_str(buf.trim())?;

        Ok(progress)
    }
    ///This method will take a reader i.e a type implementing
    /// ``tokio::io::Writer`` and write to the writer a json string
    /// representation of Progress type.
    #[instrument(name = "load_progress", skip(self, writer))]
    pub async fn save_progress<W>(&self, writer: &mut W) -> Result<()>
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
    /// Add a set of completed download chunk to the total progress
    fn add_chunk(&mut self,chunk_range:(usize,usize)){
        self.completed_chunks.push(chunk_range);
    }

}
///This function will take a ``size``
/// And generate a vector chunks size ``[start, end]``.
pub fn generate_chunk(download_size: usize,chunk_size:Option<usize>) -> Vec<[usize; 2]> {
    if let Some(chunk)= chunk_size{
        let mut handle: Vec<[usize; 2]> = Vec::new();
    for start in (0..download_size).step_by(chunk) {
        let end = (start + chunk - 1).min(download_size - 1);
        handle.push([start, end]);
    }
    return handle;
    }
    vec![[0usize,download_size-1]]
}

/// Standalone helper to fetch and write a part with minimal writer locking.
/// This function acquires the network stream WITHOUT holding the download-file lock,
/// then only locks the writer for seek+write operations on each received chunk.
/// This enables true parallelism across multiple spawned tasks.
#[instrument(name="fetch_part_parallel",skip(download_file_arc,downloader,tracker),fields(range=format!("{:?}", range),part_id=part_id))]
pub async fn fetch_part_parallel<W, D>(
    download_file_arc: Arc<Mutex<DownloadFile<W>>>,
    buffer_size: usize,
    range: [usize; 2],
    part_id: usize,
    url: Url,
    downloader: Arc<Mutex<D>>,
    progress_file: Arc<Mutex<ProgressFile>>,
    tracker: Arc<dyn ProgressTracker>,
) -> Result<()>
where
    W: AsyncWrite + AsyncSeek + Unpin + 'static,
    D: MultiPartDownload + 'static,
{
    let [first, last] = range;
    let part_size = last - first + 1;

    // Acquire network stream WITHOUT holding the download-file lock
    let (mut stream, handle) = {
        let mut dl = downloader.lock().await;
        dl.get_bytes_range(url, &range, buffer_size)?
    };

    let mut write_pos = first as u64;
    let mut bytes_written = 0;

    // Process stream; only lock file when writing each chunk
    while let Some(chunk_res) = stream.next().await {
        let chunk_var = chunk_res?;
        let chunk_len = chunk_var.len();
        bytes_written += chunk_len;

        // Lock only to seek and write this chunk (minimal critical section)
        {
            let mut df = download_file_arc.lock().await;
            df.writer.seek(SeekFrom::Start(write_pos)).await?;
            df.writer.write_all(&chunk_var).await?;
            df.writer.flush().await?
        } // Lock released here; other parts can write now

        write_pos += chunk_len as u64;
        tracker.update(part_id, bytes_written).await;
        info!("Writing chunk of len {} to writer at offset {}", chunk_len, write_pos);
    }

    // Wait for background task
    info!("Waiting for async chunk retrieval task...");
    handle.await?;

    debug!("Adding completed chunk to progress file");
    progress_file.lock().await.add_chunk((first, last));
    tracker.complete_part(part_id, part_size).await;

    Ok(())
}


    /// Write an arbitrary bytes stream into the writer. This is used for the
    /// fallback path when the server doesn't provide content length. The
    /// `tracker` should be a progress tracker that understands this single
    /// streaming part and will be updated accordingly.
    #[instrument(name = "write_stream", skip(download_file_arc, stream, tracker))]
    pub async fn write_stream<W>(
        download_file_arc: Arc<Mutex<DownloadFile<W>>>,
        mut stream: BytesStream,
        part_id: usize,
        tracker: Arc<dyn ProgressTracker>,
    ) -> Result<()> 
    where
    W: AsyncWrite + AsyncSeek + Unpin + 'static
    {
        let mut bytes_written: usize = 0;
        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res?;
            bytes_written += chunk.len();
            {
                let mut df=download_file_arc.lock().await;
                df.writer.write_all(chunk.as_ref()).await?;
                df.writer.flush().await?;
            }
            
            tracker.update(part_id, bytes_written).await;
            info!("Writing fallback chunk len {}", chunk.len());
        }
        
        Ok(())
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
    pub fn new(writer: W, url: Url) -> Self {
        let buf_writer = BufWriter::with_capacity(128 * 1024, writer);

        Self {
            writer: buf_writer,
            url,
        }
    }

}
#[cfg(test)]
mod tests {
    use super::{DownloadFile, ProgressFile,fetch_part_parallel};
    use crate::application::services::progress_service::DefaultProgressTracker;
    use crate::domain::models::DownloadInfo;
    use crate::domain::ports::download_service::DownloadInfoService;
    use crate::domain::ports::progress_tracker::ProgressTracker;
    use crate::domain::services::download::generate_chunk;
    use crate::infra::config::HttpConfig;
    use crate::infra::{config::RetryConfig, network::http_adapter::HttpAdapter};
    use anyhow::Result;
    use anyhow::Context;
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
        let mut progress = ProgressFile::new(total_size, name);
        progress.add_chunk((34, 567));
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
        let config=HttpConfig::default();
        let part_size=config.multipart_part_size;
        let adapter = HttpAdapter::new(config, &RetryConfig::default())?;
        let file_info: DownloadInfo = adapter.get_info(url.clone()).await?;
        let arc_adapter = Arc::new(Mutex::new(adapter));
        if let Some(size) = file_info.size() {
            info!("download file size from server is {size}");
            let writer = async_tempfile::TempFile::new().await?;
            debug!("Writer path is :{}",writer.file_path().display());
            let range_vec = generate_chunk(*size,part_size);
            let download_file = DownloadFile::new(writer, url.clone());
            let download_file_arc = Arc::new(Mutex::new(download_file));
            let progress_file=ProgressFile::new(file_info.size().context("Can't get size.")?,file_info.name().clone().context("Can't get name.")?);
            let progress_file_arc=Arc::new(Mutex::new(progress_file));
            
            // Create progress tracker for monitoring download progress
            let tracker: Arc<dyn ProgressTracker> = Arc::new(DefaultProgressTracker::new(*size, range_vec.len()));
            let mut future_vec = vec![];

            // Use optimized buffer size (256 KiB) instead of small 1KiB to reduce lock contention
            let buffer_size = 256 * 1024;

            for (part_id, range) in range_vec.into_iter().enumerate() {
                let download_file_clone = download_file_arc.clone();
                let arc_adapter_clone = arc_adapter.clone();
                let tracker_clone = tracker.clone();
                let progress_file_clone=progress_file_arc.clone();
                let url_clone = url.clone();
                
                // Use fetch_part_parallel to minimize lock contention
                let handle = tokio::spawn(async move {
                    fetch_part_parallel(
                        download_file_clone,
                        buffer_size,
                        range,
                        part_id,
                        url_clone,
                        arc_adapter_clone,
                        progress_file_clone,
                        tracker_clone,
                    )
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
                *size as u64,
                download_file_arc
                    .lock()
                    .await
                    .writer
                    .get_ref()
                    .metadata()
                    .await?
                    .len() 
            );
        }

        Ok(())
    }
}
