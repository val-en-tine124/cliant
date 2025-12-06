
use std::borrow::Cow;
use anyhow::Result;
use tracing::{debug, error, instrument};
use bytes::Bytes;
use infer;
use rand::Rng;
use tokio::io;
use tokio_stream::StreamExt;
use url::Url;
use crate::domain::ports::download_service::{MultiPartDownload,DownloadInfoService};

pub fn get_extension(buf:&Bytes)->Option<&'static str> {
        let inferred_type = infer::get(buf);
        if let Some(inferred_type) = inferred_type {
        return Some(inferred_type.extension());
    }
    None
}

///Struct representing a download name.
struct DownloadName<'a,T>{
    
    download_service:&'a mut  T,
}

impl<'a,T:MultiPartDownload+DownloadInfoService> DownloadName<'a,T>{
    pub fn new(download_service:&'a mut  T)->Self{
        Self{download_service}
    }

    ///This method only works for protocols that implements ``MultiPartDownload`` trait. 
    #[instrument(name="infer_name",skip(self,))]
    pub async fn get(&mut self,url:Url,)->Result<Option<Cow<'_,str>>>{
        let info = self.download_service.get_info(url).await?;
        if let Some(name)=info.name(){
            let name_string=name.clone();
            return Ok(Some(Cow::from(name_string)));
        }
        let mut buffer=Vec::with_capacity(2048);
        match self.download_service.get_bytes_range(info.url().clone(),&[0,2048],2048){
            Ok((mut stream,handle))=>{
                while let Some(chunk_result)=stream.next().await{ //Iterate over stream generator.
                     let chunk=chunk_result?;   
                    debug!("Copying bytes from Reader to writer...");
                    let _ =io::copy(&mut chunk.as_ref(),&mut  buffer).await;
                    debug!("Buffer after copy : {:?}",&buffer);
                    buffer.truncate(2048);            
                    debug!("Buffer after copy : {:?}",&buffer);
                    
                    
                }

                let _=handle.await; //Make sure the stream fetch has been completed.

                

                if let Some(ext)=get_extension(&Bytes::copy_from_slice(&buffer)){
                        let random_no: u32 = rand::thread_rng().gen();
                        let download_name=format!("{random_no}.{ext}");
                        let cow_dname=Cow::from(download_name);
                        
                        return Ok(Some(cow_dname));
                        

                }
                Ok(None)



            },
            Err(error)=>{
                error!("Error:{}",error);
                Ok(None)
            }
        }
        

    }
}

#[tokio::test]
async fn test_download_name()->Result<()>{
    use crate::infra::network::http_adapter::HttpAdapter;
    use crate::infra::config::{HttpConfig,RetryConfig};
    use tracing::{info, Level};
    use crate::utils::test_logger_init;   

    test_logger_init(Level::DEBUG);
    if let Ok(url)=url::Url::parse("http://ipv4.download.thinkbroadband.com/5MB.zip"){
        
        let mut adapter=HttpAdapter::new(HttpConfig::default(),&RetryConfig::default()).expect("No adapter");
        let mut d_name=DownloadName::new(&mut adapter);
        if let Some(name) = d_name.get(url).await?{
            info!("Got! name {name}",);
        }else{
            info!("Can't get name.");
        }
    }
    Ok(())



}
