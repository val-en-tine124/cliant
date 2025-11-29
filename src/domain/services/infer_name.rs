use std::borrow::Cow;

use bytes::Bytes;
use infer;
use rand::Rng;
use tokio::io;
use tokio_stream::StreamExt;
use crate::domain::models::download_info::DownloadInfo;
use crate::domain::ports::download_service::MultiPartDownload;
use crate::infra::network::http_adapter::RetryConfig;

pub fn get_extension(buf:&Bytes)->Option<&'static str> {
        let inferred_type = infer::get(buf);
        if let Some(inferred_type) = inferred_type {
        return Some(inferred_type.extension());
    }
    None
}

///Struct representing a download name.
struct DownloadName<'a,T>{
    info:DownloadInfo,
    download_service:&'a mut  T,
}

impl<'a,T:MultiPartDownload> DownloadName<'a,T>{
    pub fn new(info:DownloadInfo,download_service:&'a mut  T)->Self{
        Self{info,download_service}
    }

    ///This method only works for protocols that implements MultiPartDownload trait. 
    pub async fn get(&mut self)->Option<Cow<'_,str>>{
        if let Some(name)=self.info.name(){
            let name_string=name.to_string();
            return Some(Cow::from(name_string));
        }
        let mut buffer=Vec::with_capacity(2048);
        match self.download_service.get_bytes_range(self.info.url().clone(),&[0,2048],2048){
            Ok((mut stream,handle))=>{
                while let Some(chunk_result)=stream.next().await{ //Iterate over stream generator.
                    if let Err(error)=&chunk_result{
                        eprintln!("Error:{}",error.to_string());
                    }
                    if let Ok(chunk)=chunk_result{
                        
                        
                        let _ =io::copy(&mut chunk.as_ref(),&mut  buffer).await;
                        buffer.truncate(2048);            
                    }  
                    
                }

                let _=handle.await; //Make sure the stream fetch has been completed.

                

                if let Some(ext)=get_extension(&Bytes::copy_from_slice(&buffer)){
                        let random_no: u32 = rand::thread_rng().gen();
                        let download_name=format!("{}.{}",random_no,ext);
                        let cow_dname=Cow::from(download_name);
                        
                        return Some(cow_dname);
                        

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
        use crate::infra::network::http_adapter::HttpAdapter;
        use chrono::Local;
        use crate::infra::config::http_config::HttpConfig;
        let mut adapter=HttpAdapter::new(HttpConfig::default(),RetryConfig::default()).expect("No adapter");
        let mut d_name=DownloadName::new(info,&mut adapter);
        if let Some(name) = d_name.get().await{
            println!("Got! name {}",name);
        }else{
            println!("Can't get name.");
        }
    }
    



}
