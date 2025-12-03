use std::path::PathBuf;
use std::pin::Pin;
use bytes::BytesMut;
use chrono::{DateTime, Local};
use derive_getters::Getters;
use url::Url;
use std::io::{Cursor, SeekFrom};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek,AsyncSeekExt, AsyncWrite, AsyncWriteExt,BufWriter};
use tracing::{debug, info};
use tokio_stream::{Stream,StreamExt};


use anyhow::{Context, Result};
use crate::domain::ports::download_service::{DownloadInfoService,MultiPartDownload};
use crate::domain::models::download_info::DownloadInfo;
use crate::domain::models::file_info::FileInfo;
#[allow(unused)]
type BytesStream=Pin<Box<dyn Stream<Item = Result<bytes::Bytes, anyhow::Error>> + std::marker::Send + 'static>>;
#[allow(unused)]
const CHUNKSIZE:usize= 1024;

#[derive(Deserialize,Serialize,Getters)]
struct Progress{
    path:PathBuf,
    download_name:String,
    total_size:usize,
    completed_chunks:Vec<(usize,usize)>,
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
    async fn load_progress<R>(&self,reader:R)->Result<Progress>
    where R:AsyncRead +
    {
        let mut buf=String::new();
        futures::pin_mut!(reader);
        let bytes_count=reader.read_to_string(&mut buf).await?;
        debug!("Red {bytes_count} of progress report to to String buffer.");
        debug!("Loading download Progress Object information from string buffer ");
        let progress:Progress=serde_json::from_str(&buf)?;
        
        Ok(progress)
    }
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

fn generate_chunk(url:Url,size:usize,)->Vec<[usize; 2]>{
    let mut my_vec: Vec<[usize; 2]>=Vec::new();
    for start in (0..size).step_by(CHUNKSIZE){
    let end=(start+CHUNKSIZE - 1).min(size-1);
    my_vec.push([start,end]);

    } 
    my_vec
    
}