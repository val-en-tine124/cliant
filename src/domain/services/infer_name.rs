use bytes::Bytes;
use chrono::Local;
use infer;
use rand::Rng;
use tokio::io;
use tokio_stream::StreamExt;
use crate::domain::models::download_info::DownloadInfo;
use crate::domain::ports::download_service::{ShutdownDownloadService, DownloadService};

pub fn get_extension(buf:&Bytes)->Option<String> {
        let inferred_type = infer::get(buf);
        if let Some(inferred_type) = inferred_type {
        return Some(inferred_type.extension().to_string());
    }
    None
}

struct DownloadName<T>{
    info:DownloadInfo,
    download_service:T,
}

impl<T:DownloadService+ShutdownDownloadService> DownloadName<T>{
    pub fn new(info:DownloadInfo,download_service:T)->Self{
        Self{info,download_service}
    }

    pub async fn get(&mut self)->Option<String>{
        if let Some(name)=self.info.name(){
            return Some(name.clone());
        }
        let mut buffer=Vec::with_capacity(2048);
        match self.download_service.get_bytes(self.info.url().clone(),&[0,2048],2048){
            Ok(mut stream)=>{
                while let Some(chunk_result)=stream.next().await{
                    if let Err(error)=&chunk_result{
                        eprintln!("Error:{}",error.to_string());
                    }
                    if let Ok(chunk)=chunk_result{
                        
                        
                        let _ =io::copy(&mut chunk.as_ref(),&mut  buffer).await;
                        buffer.truncate(2048);            
                    }
                    
                }

                

                if let Some(ext)=get_extension(&Bytes::copy_from_slice(&buffer)){
                        let random_no: u32 = rand::thread_rng().gen();
                        self.download_service.shutdown().await;
                        return Some(format!("{}.{}",random_no,ext));
                        

                }
                return None;



            },
            Err(error)=>{
                eprintln!("Error:{}",error);
                return None;
            }
        }
        

    }
}

#[tokio::test]
async fn test_download_name(){
    if let Ok(url)=url::Url::parse("http://127.0.0.1:8080"){

        let info=DownloadInfo::new(url,None,None,Local::now(),None);
        use crate::infrastructure::network::http_adapter::HttpAdapter;
        use crate::infrastructure::config::http_config::HttpConfig;
        let adapter=HttpAdapter::new(HttpConfig::default()).expect("No adapter");
        let mut d_name=DownloadName::new(info,adapter);
        if let Some(name) = d_name.get().await{
            println!("Got! name {}",name);
        }else{
            println!("Can't get name.");
        }
    }
    



}
