use std::path::Path;
use url::Url;
use tokio_stream::{Stream,StreamExt};

use crate::domain::errors::DomainError;
use crate::domain::ports::storage_service::FileIO;
use crate::domain::ports::download_service::{ShutdownDownloadService, DownloadService};

pub struct MergeParts<'a,F,S>{
    fs:&'a F,
    path:&'a Path,
    download_service:&'a mut S,
}

impl<'a,F:FileIO,S:DownloadService> MergeParts<'a,F,S>{
    pub async fn new(path:&'a Path,fs:&'a F,download_service:&'a mut S)-> Result<Self,DomainError>{
        if !fs.file_exists(path).await?{
        fs.create_file(path).await?;
        return Ok(Self {path,fs,download_service});
        }
        Ok(Self {path,fs,download_service})
    }

    async fn work_on_stream(&self,stream:&mut std::pin::Pin<Box<dyn Stream<Item = Result<bytes::Bytes, DomainError>> + Send + 'static>>)->Result<(),DomainError>{
        while let Some(chunk_result) = stream.next().await{
            
            match chunk_result{
                Ok(chunk)=>{
                    if let Err(err)=self.fs.append_to_file(&chunk, self.path).await{
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
    
    pub async fn merge(&mut self,url:Url,buffer_size:usize,bytes_range:Vec<&[u64;2]>)->Result<(),DomainError>{
        let stream_results = bytes_range.iter().map(|range|{
            self.download_service.get_bytes(url.clone(), range, buffer_size)
        }).collect::<Vec<_>>();

        let transformed_stream=stream_results.into_iter().map(async |result|->Result<(),DomainError>{
            match result{
                Ok( mut stream)=>{
                    let _ = self.work_on_stream(&mut stream).await?;
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
        
        Ok(())


        
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::infrastructure::storage::fs_adapter::DiskFileSystem;
    use crate::infrastructure::network::http_adapter::HttpAdapter;
    use crate::infrastructure::config::http_config::HttpConfig;

    #[tokio::test]
    async fn test_merge(){
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("my_mp4.mp4");
        let fs = DiskFileSystem::new();
        let config = HttpConfig::default();
        let mut download_service = HttpAdapter::new(config).expect("Can't get adapter.");
        let mut merge_part = MergeParts::new(&path, &fs, &mut download_service).await.expect("Can't create MergeParts struct.");
        if let Ok(url) = Url::parse("http://127.0.0.1:8080/fake_mp4.mp4"){
            let result = merge_part.merge(url, 1024, vec![&[0,1024],&[1025,2048]]).await;
            fs.ge
            assert!(result.is_ok());
        }
    }
}
