use chrono::{DateTime, Local};
use std::{fmt::Debug, path::PathBuf};
use url::Url;
use serde::Serialize;
use derive_getters::Getters;
use anyhow::Result;

#[derive(Serialize)]
pub enum DownloadStatus {
    Success,
    Error(String),
}

use crate::domain::models::download_info::DownloadInfo;

#[derive(Serialize, Getters)]
pub struct DownloadResponse {
    #[getter(skip)]
    download_info: Option<DownloadInfo>,
    path: PathBuf,
    #[getter(skip)]
    status: DownloadStatus,
}

impl DownloadResponse {
    pub fn new(
        download_info: Option<DownloadInfo>,
        path: PathBuf,
        status: DownloadStatus,
    ) -> Self {
        Self {
            download_info,
            path,
            status,
        }
    }

    pub fn size(&self)->String{
        match self.download_info{
            Some(ref info)=>{
                if let Some(size) = info.size(){
                    return format!("{}",size).into();
                }
                return "".into();
                
            },
            None=>{
                "".into()
            }
            
        }
        
    }

    pub fn name(&self)->Option<String>{
        match self.download_info{
            Some(ref info)=>{
                info.name().clone()
            },
            None=>None
        }
    }

    pub fn url(&self) -> Option<&Url> {
        match self.download_info{
            Some(ref info)=>{
                Some(info.url())
            },
            None=>{
                None
            }
        }
    }

    pub fn download_date(&self) -> Option<DateTime<Local>> {
        match self.download_info{
            Some(ref info)=>{
                let date=info.download_date().clone();
                Some(date)
            },
            None=>None
        }
        
    }



    pub fn status(&self) -> String {
        const SUCCESS: &str = "SUCCESS";
        const ERROR: &str = "ERROR";
        match self.status {
            DownloadStatus::Success => SUCCESS.into(),
            DownloadStatus::Error(ref msg) => format!("{}:{}", ERROR, msg).into(),
        }
    }
}

impl Debug for DownloadResponse{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("Url:{}
        
Download path:{:?}
        
Download name:{}
        
Download date:{}
        
Download type:{}
        
Download size(bytes):{}
        
Download status:{}."
        ,self.url().map(|url|url.to_string()).unwrap_or_default(),
        self.path(),
        self.name().unwrap_or("".into()),
        self.download_date().unwrap_or(Local::now()),
        self.size(),
        self.size(),
        self.status(),
        ).as_str())   
    }
}

#[test]
fn test_download_response()->Result<()>{
    let info=DownloadInfo::new(Url::parse("https://")?, Some("download_file.mp4".into()), Some(40000), Local::now(), Some("video/mp4".into()));
    let resp=DownloadResponse::new(Some(info),"".into(),DownloadStatus::Success);
    println!("{:?}",resp);
    assert!(true);
    Ok(())

}