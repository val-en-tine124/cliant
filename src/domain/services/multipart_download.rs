use std::path::Path;
use tokio::sync::OnceCell;
use url::Url;
use tokio_stream::{Stream,StreamExt};

use crate::domain::errors::DomainError;
use crate::domain::ports::storage_service::FileIO;
use crate::domain::ports::download_service::{DownloadInfoService,MultiPartDownload};
use crate::domain::models::download_info::DownloadInfo;
use crate::domain::models::file_info::FileInfo;
 

 enum DownloadState{
    Initializing,
    Started,
    BrokenFile,
 }

 ///A struct to download a file and save it to a file system.
struct MultiParts<'a,F,S>{
    fs:&'a F,
    download_service:&'a mut S,
    file_info:OnceCell<FileInfo<'a>>,
    download_info:OnceCell<DownloadInfo>,
    state:DownloadState,
}


impl<'a,F:FileIO,S:MultiPartDownload+DownloadInfoService> MultiParts<'a,F,S>{
    pub fn new(fs:&'a F,download_service:&'a mut S,)->Self{
        
        Self{
            fs:fs,
            download_service:download_service,
            file_info:OnceCell::new(),
            download_info:OnceCell::new(),
            state:DownloadState::Initializing,

        }
    }

    pub async fn execute(&mut self,url:Url,path:&'a Path,buffer_size:usize,bytes_range:Vec<&[u64;2]>)->Result<(),DomainError>{
        let _ = self.init(path,url)
        .await?.handle_broken_file(path)
        .await?.start_download(buffer_size, bytes_range).await?;
        Ok(())
    }

    async fn init(& mut self,path:&'a Path,url:Url)->Result<&mut Self,DomainError>{
        if let DownloadState::Initializing=self.state{

            let download_info=self.download_service.get_info(url).await?;
            self.download_info.set(download_info).map_err(
                |_|DomainError::Other{message
                    :"Can't intialize file information for file on file system .".into()
                }
            )?;

            if !self.fs.file_exists(path).await?{ // handle non-existence file.
            self.fs.create_file(path).await?;
            }

            self.file_info.set(self.fs.file_info(path).await?).map_err(|_|DomainError::Other{message
                :"Can't intialize file information for file on file system .".into()})?;
        
        }


        

        
        

        

        Ok(self)
    }

    async fn start_download(& mut self,buffer_size:usize,bytes_range:Vec<&[u64;2]>)->Result<(),DomainError>{
        if self.download_info.initialized() && self.file_info.initialized(){
            self.state=DownloadState::Started;
        }

        if let DownloadState::Started=self.state{
            
            let stream_results = bytes_range.iter().map(|range|{
                self.download_service.get_bytes_range(self.download_info.get().unwrap().url().clone(), range, buffer_size)
                }).collect::<Vec<_>>();

                let transformed_stream=stream_results.into_iter().map(async |result|->Result<(),DomainError>{
                    match result{
                        Ok( mut stream)=>{
                            let _ = self.write_stream(& mut stream).await?;
                            Ok(())
                        },
                        Err(err)=>{
                            eprintln!("Error! : {}",err);
                            Err(err)    
                        }
                    }
                }).collect::<Vec<_>>();

                for async_func in transformed_stream{
                    async_func.await?;
                }

        }


        
        
        Ok(())

    }

    async fn write_stream(& self,stream:&mut std::pin::Pin<Box<dyn Stream<Item = Result<bytes::Bytes, DomainError>> + Send + 'static>>)->Result<(),DomainError>{
        while let Some(chunk_result) = stream.next().await{
            
            match chunk_result{
                Ok(chunk)=>{
                    
                    if let Err(err)=self.fs.append_to_file(&chunk, self.file_info.get().unwrap().path()).await{
                        eprintln!("Error:{}",err);
                        return Err(err);
                    }
                },
                Err(err)=>{
                    eprintln!("Error! : {}",err);
                    return Err(err);
                }
                
            }
        }
        Ok(())

    }



    async fn handle_broken_file(& mut self,path:& Path)->Result<& mut Self,DomainError>{
        let download_info=self.download_info.get();
        let file_info=self.file_info.get();
        if let (Some(download),Some(file))= (download_info,file_info){
            if download.size().unwrap_or(0)  != file.size(){
                self.state=DownloadState::BrokenFile;
            }
        }

        if let DownloadState::BrokenFile=self.state{ // handle broken file.
            self.fs.remove_file(path).await?;
            self.fs.create_file(path).await?;
        }
        Ok(self)
    }


}





#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::domain::ports::download_service::DownloadInfoService;
    use crate::infrastructure::storage::fs_adapter::DiskFileSystem;
    use crate::infrastructure::network::http_adapter::HttpAdapter;
    use crate::infrastructure::config::http_config::HttpConfig;

    #[tokio::test]
    async fn test_merge(){
        let temp_dir = tempdir().unwrap();
        //let path = temp_dir.path().join("my_mp4.mp4");
        let path=Path::new("C:\\").join("my_mp4.mp4");
        let fs = DiskFileSystem::new();
        let config = HttpConfig::default();
        let mut download_service = HttpAdapter::new(config).expect("Can't get adapter.");
        let mut multi_part = MultiParts::new(&fs,&mut download_service,);
        if let Ok(url) = Url::parse("http://127.0.0.1:8080/fake_mp4.mp4"){
            
            let  result= multi_part.execute(url,&path, 1024, vec![&[0,1024],&[1025,2048]]).await;
            
            assert!(result.is_ok());
            
            
        }
    }
}
