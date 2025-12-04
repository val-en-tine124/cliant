//! This module contains Object for running download tasks.

use std::path::PathBuf;
use std::pin::Pin;
use chrono::{DateTime, Local};
use derive_getters::Getters;
use derive_setters::Setters;
use tokio::fs::{File, OpenOptions};
use tracing::instrument::WithSubscriber;
use url::Url;
use std::io::{Cursor, SeekFrom};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek,AsyncSeekExt, AsyncWrite, AsyncWriteExt,BufWriter};
use tracing::{debug, info, instrument};
use tokio_stream::{Stream,StreamExt};


use anyhow::{Context, Result};
use crate::domain::ports::download_service::{DownloadInfoService,MultiPartDownload};
use crate::domain::models::download_info::DownloadInfo;
use crate::domain::models::file_info::FileInfo;
#[allow(unused)]
type BytesStream=Pin<Box<dyn Stream<Item = Result<bytes::Bytes, anyhow::Error>> + std::marker::Send + 'static>>;
#[allow(unused)]
const CHUNKSIZE:usize= 1024;

///This is the progress file that can get serialized 
/// and deserialized on-demand for the purpose of tracking download progress
#[derive(Deserialize,Serialize,Getters,Setters,Debug)]
struct Progress{
    /// Path of the download.
    #[setters(skip)]
    path:PathBuf,
    /// Name of the download file.
    #[setters(skip)]
    download_name:String,
    ///Total download file size.
    #[setters(skip)]
    total_size:usize,
    ///Completed download segments or chunks.
    #[setters(rename = "completed_fragments")]
    completed_chunks:Vec<(usize,usize)>,
    ///Date the download started.
    #[setters(skip)]
    started_on:DateTime<Local>,
}


impl Progress{
    fn new(path:PathBuf,total_size:usize,download_name:String)->Self{
        Self{
            path,
            download_name,
            total_size,
            completed_chunks:vec![],
            started_on:Local::now(),
        }
    }
    ///This method will take a reader i.e a type implementing 
    /// ``tokio::io::Reader`` and load json string 
    /// representation of Progress type.
    #[instrument(name="load_progress",skip(self,reader),)]
    async fn load_progress<'a,R>(&self,reader:&'a mut R)->Result<Progress>
    where R:AsyncRead+AsyncSeek + Unpin
    {
        let mut buf=String::new();
        // Seek back to start because write advances cursor to end
        debug!("Seeking to position 0 on Writer...");
        reader.seek(SeekFrom::Start(0)).await?;
        let bytes_count=reader.read_to_string(&mut buf).await?;
        debug!("Read {bytes_count} of progress report to String buffer.");
        debug!("Loading download Progress Object information from string buffer {:?}",&buf);
        
        let progress:Progress=serde_json::from_str(buf.trim())?;
        
        Ok(progress)
    }
    ///This method will take a reader i.e a type implementing 
    /// ``tokio::io::Writer`` and write to the writer a json string 
    /// representation of Progress type.
    #[instrument(name="load_progress",skip(self,writer),)]
    async fn save_progress<W>(&self,writer:&mut W)->Result<()>
    where W:AsyncWrite+AsyncSeek+Unpin
    {   
        let progress_json=serde_json::to_string(self)?;
        let mut  progress_cursor=Cursor::new(progress_json.trim());
        // Seek back to start because read advances cursor to end
        debug!("Seeking to position 0 on Writer...");
        writer.seek(SeekFrom::Start(0)).await?;
        debug!("Writing download Progress Object in String buffer data to Writer...");
        writer.write_all_buf(&mut progress_cursor).await?;
        info!("Flushing data in writer buffer... ");
        writer.flush().await?;
        
        Ok(())
    }
    
}
///This function will take a ``size``
/// And generate a vector chunks size ``[start, end]``.
fn generate_chunk(size:usize,)->Vec<[usize; 2]>{
    let mut handle: Vec<[usize; 2]>=Vec::new();
    for start in (0..size).step_by(CHUNKSIZE){
    let end=(start+CHUNKSIZE - 1).min(size-1);
    handle.push([start,end]);

    } 
    handle
    
}


#[derive(Getters)]
///DownloadFile object abstracts operations e.g multipart operation on downloads
struct DownloadFile<'a,W>{
    #[getter(skip)]
    writer:BufWriter<&'a mut W>,
    ///This is the download url.
    url:Url,
}
impl<'a ,W> DownloadFile<'a,W> where W:AsyncWrite+AsyncSeek+Unpin{
    fn new(writer:&'a mut W,url:Url)->Self{
        let buf_writer=BufWriter::with_capacity(128*1024, writer);
        
        Self {writer:buf_writer,url}
    }

    /// This method changes i.e seek The ``Writer`` cursors in other to 
    /// write a download chunk fetched from an http server.
    /// ## Parameters:
    /// * ``downloader`` : Protocols that support multipart downloading.
    /// * ``range`` : Slice of integers for protocols that support multipart downloading.
   ///  * ``buffer_size`` : Size of the in-memory buffer
   #[instrument(name="load_progress",skip(self,downloader),fields(buffer_size=buffer_size,range=format!("{:?}", range)))]
    async fn fetch_part<D>(&mut self,buffer_size:usize,range:&[usize;2],mut downloader:D)->Result<()>
 where D:MultiPartDownload
    {
    let [first,_]=*range;
    
    self.writer.seek(SeekFrom::Start(first as u64)).await?; // Let the cursor point to the the current range offset.
    let (mut stream,handle)=downloader.get_bytes_range(self.url.clone(), range, buffer_size)?;
    
    while let Some(chunk) = stream.next().await{
        let chunk_var=chunk?;
        
        self.writer.write_all(&chunk_var).await?;
        info!("Writing chunk of len {} to writer ",chunk_var.len());
    }
    info!("Flushing chunks in writer buffer... ");
    self.writer.flush().await?;
    info!("Waiting for async chunk retrival task...");
    handle.await?;
    
    Ok(())
}
}

#[tokio::test]
async fn progress_file_test()->Result<()>{
    use tracing_subscriber::{fmt,EnvFilter};
    use tracing_subscriber::prelude::*;
    use tracing::Level;
    let filter = EnvFilter::builder()
        .with_default_directive(Level::DEBUG.into())  // default = warn
        .from_env_lossy(); // respects RUST_LOG if user set it
    
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer()
            .with_ansi(true)           // colors in terminal
            .with_target(false)        // cleaner output
            .with_file(false)
            .with_line_number(false)
            .compact())                // one-line format, perfect for CLIs
        .init();

    let home_dir=std::env::home_dir().unwrap_or(std::env::current_dir()?);
    let progress_path=home_dir.join("My_Progress_File.json");
    info!("Progress path is {progress_path:?}");
    let total_size=38560;
    let name="My_video_file.mp4".to_string();
    let mut  progress=Progress::new(home_dir,total_size,name);
    progress=progress.completed_fragments(vec![(34,567)]);
    let mut handle=OpenOptions::new().create(true).truncate(false).read(true).write(true).open(progress_path).await?;
    let save_result=progress.save_progress(&mut handle).await;
    
    let mut buf=String::new();
    handle.read_to_string(&mut buf).await?;
    info!("content of in-memory buffer: {buf}",);
    
    assert!(save_result.is_ok());
    
    handle.seek(SeekFrom::Start(0)).await?;
    let load_result=progress.load_progress(&mut handle).await;
    assert!(load_result.is_ok());
    info!("Progress Object:{:?}",load_result?);
    Ok(())
}