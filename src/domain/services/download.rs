use std::path::{Path, PathBuf};
use std::pin::Pin;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender; 
use tracing::error;
use url::Url;
use tokio_stream::{Stream,StreamExt};

use anyhow::{Context, Result};
use crate::domain::ports::storage_service::FileIO;
use crate::domain::ports::download_service::{DownloadInfoService,SimpleDownload};
use crate::domain::models::download_info::DownloadInfo;
use crate::domain::models::file_info::FileInfo;

#[async_trait]
trait DownloadMethod{
    ///This async method will check if a file exists else it will create it and prepare it for download.
    async fn fetch_info<'a>(& mut self,path:&'a Path,url:Url)->Result<()>;
    ///This async method will contain 
    async fn start_download(&mut self,buffer_size:usize,progress_update:&mut Sender<usize>)->Result<()>;
}

async fn write_stream<F>(fs:&F,file_info:&FileInfo,stream: &mut Pin<Box<dyn Stream<Item = Result<bytes::Bytes, anyhow::Error>> + std::marker::Send + 'static>>,progress_update:&mut Sender<usize>)->Result<()>
        where F:FileIO{
        let mut downloaded_size=0; // intialize progress counter.
        while let Some(chunk_result) = stream.next().await{
            
            match chunk_result{
                Ok(chunk)=>{
                    let size=*file_info.file_size();
                    let path=file_info.path().to_owned();
                    let write_at=fs.set_len(&path,size).await?;
                    if let Err(err)=fs.append_to_file(&chunk, file_info.path()).await{
                        error!("Can't write network stream to file: {}.",err);
                        return Err(err);
                    }
                    downloaded_size+=chunk.len(); //increment counter after file has been written to storage.
                    progress_update.send(downloaded_size).await?;
                },
                Err(err)=>{
                    error!("Can't write network stream to file: {}.",err);
                    return Err(err);
                }
                
            }
        }
        Ok(())
}

async fn check_broken_file<F>(fs:&F,file_info:&FileInfo,download_info:&DownloadInfo,path:& Path)->Result<()>
    where F:FileIO
    {
        
        if download_info.size().unwrap_or(0)  != *file_info.file_size(){
            // handle broken file.
            fs.remove_file(path).await?;
            fs.create_file(path).await?;
            return Ok(());
        }
        
        Ok(())
    }


pub struct Simple<F,S>{
    fs:F,
    download_service:S,
    file_info:Option<FileInfo>,
    download_info:Option<DownloadInfo>,
}

impl<F:FileIO,S:SimpleDownload + DownloadInfoService> Simple<F,S>{
    pub fn new(fs:F,download_service:S,)->Self{
        
        Self{
            fs:fs,
            download_service:download_service,
            file_info:None,
            download_info:None,
        }
    }

    pub async fn execute(&mut self,url:Url,path:PathBuf,buffer_size:usize,mut progress_update:Sender<usize>)->Result<DownloadInfo>{
        self.fetch_info(&path,url).await?;
        self.check_broken_file(&path).await?;
        let _ = self.start_download(buffer_size,&mut progress_update).await?;
        let download_info=self.download_info.clone().context("Can't get download information.")?;
        Ok(download_info)
    }

    async fn fetch_info<'a>(& mut self,path:&'a Path,url:Url)->Result<()>{
        let download_info=self.download_service.get_info(url).await?;
        self.download_info=Some(download_info);

        if !self.fs.file_exists(path).await?{ // handle non-existence file.
            self.fs.create_file(&path.to_path_buf()).await?;    
        }
        let info=self.fs.file_info(path).await?;
        self.file_info=Some(info);
        Ok(())

    }

    async fn start_download(&mut self,buffer_size:usize,progress_update:&mut Sender<usize>)->Result<()>{
        let file_info=self.file_info.clone().context("Can't get file information.,FileInfo Object not fetch_infoialized properly.")?;
        let download_info=self.download_info.clone().context("Can't get download information.,DownloadInfo Object not fetch_infoialized properly.")?;
        {

            let (mut stream,handle)=self.download_service.get_bytes(download_info.url().clone(), buffer_size)?;
            let _ = write_stream(&self.fs,&file_info,&mut stream,progress_update).await?;
            let _ = handle.await?;
        }
        
        Ok(())
    }

    

    async fn check_broken_file(& mut self,path:& Path)->Result<()>{
        
        let file_info=self.file_info.clone().context("Can't get file information.")?;
        let download_info=self.download_info.clone().context("Can't get download information.")?;

        if download_info.size().unwrap_or(0)  != *file_info.file_size(){
            // handle broken file.
            self.fs.remove_file(path).await?;
            self.fs.create_file(path).await?;
            return Ok(());
        }
        
        Ok(())
        }


        
}

enum DownloadMode{
    MultiPart,
    Simple,
}
fn start_download(){

}


#[tokio::test]
async fn test_simple()->Result<()>{
    use crate::infra::{config::http_config::HttpConfig,network::http_adapter::{HttpAdapter,RetryConfig}};
    use crate::infra::storage::fs_adapter::DiskFileSystem;
    use tokio::sync::mpsc::channel;

    
    let client=HttpAdapter::new(HttpConfig::default(), RetryConfig::default())?;
    let fs=DiskFileSystem::new();
    let url=Url::parse("http://0.0.0.0:8000/output.pdf")?;
    
    let file_path= std::env::temp_dir().join("output.pdf");
    let (tx,mut rx)=channel::<usize>(100);
    let file_path_clone=file_path.clone();
    println!("downloading to path {:?} ...",&file_path);
    let handle=tokio::spawn(
        async move{
            let result = Simple::new(fs, client)
            .execute(url, file_path, 1024, tx).await;
            if let Err(err) = result{
                return Err(err);

            }
            Ok(())
            }  
    );
    
    while let Some(progress)=rx.recv().await{
        println!("{} No of bytes has been written to file...",progress);
    }

    
    assert!(file_path_clone.try_exists()?);
    tokio::fs::remove_file(file_path_clone).await?;
    handle.await?

}




#[cfg(test)]
mod tests {
    
}