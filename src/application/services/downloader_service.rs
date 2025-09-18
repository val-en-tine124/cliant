use chrono::Local;
use tracing::{instrument,error,debug};

use crate::application::dto::download_response::{DownloadResponse, DownloadStatus};
use crate::domain::commands::start_download::MultiPartCommand;
use crate::domain::ports::download_service::{DownloadInfoService, MultiPartDownload};
use crate::domain::ports::storage_service::FileIO;
use crate::domain::services::multipart_download::MultiParts;

struct MultipartDownloader<'a,T,F>{
    connector:&'a mut T,
    fs:&'a F,
}
impl<'a,T:MultiPartDownload+DownloadInfoService,F:FileIO> MultipartDownloader<'a,T,F>{
    pub fn new(connector:&'a mut T,fs:&'a F)->Self{
        Self{
            connector:connector,
            fs:fs,
        }
    }
    #[instrument(name="multipart_downloader_execute",skip(self,command),)]
    pub async fn execute(& mut self,command:MultiPartCommand<'a>)->DownloadResponse{
        
        let mut multi_parts=MultiParts::new(self.fs,&mut *(self.connector));
        let url=command.url();
        let path=command.path();
        debug!(name:"initialize_multi_paart_download","Initialize multipart download for url {}.",url);
        let exec_result=multi_parts.execute(url.clone(),path,2048,command.frames_no(),command.frame_size()).await;

        match exec_result{
            Ok(download_info)=>{
                let time=download_info.download_date().clone();
                return DownloadResponse::new(url.clone(), command.path().to_owned(), download_info.name().map(|s| s.to_string()),time, download_info.download_type().map(|s| s.to_string()), download_info.size(), DownloadStatus::Success);
            },
            
            Err(err)=>{
                error!(error=%err,"Can't execute multipart download.");
                DownloadResponse::new(url.clone(),path.to_owned(),None,Local::now(),None,None,DownloadStatus::Error(err.to_string()))
            }
        }

  
        
    }

}

