//! This module contains Object for running download tasks.

use std::path::PathBuf;
use std::pin::Pin;
use bytes::BytesMut;
use chrono::{DateTime, Local};
use derive_getters::Getters;
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
#[derive(Deserialize,Serialize,Getters)]
struct Progress{
    /// Path of the download.
    path:PathBuf,
    /// Name of the download file.
    download_name:String,
    ///Total download file size.
    total_size:usize,
    ///Completed download segments or chunks.
    completed_chunks:Vec<(usize,usize)>,
    ///Date the download started.
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
    async fn load_progress<R>(&self,reader:R)->Result<Progress>
    where R:AsyncRead +
    {
        let mut buf=String::new();
        futures::pin_mut!(reader);
        let bytes_count=reader.read_to_string(&mut buf).await?;
        debug!("Red {bytes_count} of progress report to String buffer.");
        debug!("Loading download Progress Object information from string buffer {:?}",&buf);
        let progress:Progress=serde_json::from_str(&buf)?;
        
        Ok(progress)
    }
    ///This method will take a reader i.e a type implementing 
    /// ``tokio::io::Writer`` and write to the writer a json string 
    /// representation of Progress type.
    #[instrument(name="load_progress",skip(self,writer),)]
    async fn save_progress<W>(&self,mut writer:W)->Result<()>
    where W:AsyncWrite + Unpin
    {
        let progress_json=serde_json::to_string(self)?;
        let mut  progress_cursor=Cursor::new(progress_json);
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
    let mut my_vec: Vec<[usize; 2]>=Vec::new();
    for start in (0..size).step_by(CHUNKSIZE){
    let end=(start+CHUNKSIZE - 1).min(size-1);
    my_vec.push([start,end]);

    } 
    my_vec
    
}


#[derive(Getters)]
///DownloadFile object abstracts operations e.g multipart operation on downloads
struct DownloadFile<W>{
    #[getter(skip)]
    writer:Pin<Box<BufWriter<W>>>,
    ///This is the download url.
    url:Url,
}
impl<W> DownloadFile<W> where W:AsyncWrite+AsyncSeek{
    fn new(writer:W,url:Url)->Self{
        let buf_writer=BufWriter::with_capacity(128*1024, writer);
        let pinned_writer=Box::pin(buf_writer);
        Self {writer:pinned_writer,url}
    }

    /// This method changes i.e seek The ``Writer`` cursors in other to 
    /// write a download chunk fetched from an http server.
    /// ## Parameters:
    /// * downloader : Protocols that support multipart downloading.
    /// * range : Slice of integers for protocols that support multipart downloading.
   ///  * buffer_size : Size of the in-memory buffer
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