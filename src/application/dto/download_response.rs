use chrono::{DateTime, Local};
use std::{borrow::Cow, fmt::Debug, path::{Path, PathBuf}};
use url::Url;
use serde::Serialize;

#[derive(Serialize)]
pub enum DownloadStatus {
    Success,
    Error(String),
}

#[derive(Serialize)]
pub struct DownloadResponse {
    url: Url,
    path: PathBuf,
    name: Option<String>,
    size: Option<usize>,
    download_date: DateTime<Local>,
    download_type: Option<String>,
    status: DownloadStatus,
}

impl DownloadResponse {
    pub fn new(
        url: Url,
        path: PathBuf,
        name: Option<String>,
        download_date: DateTime<Local>,
        download_type: Option<String>,
        size: Option<usize>,
        status: DownloadStatus,
    ) -> Self {
        Self {
            url: url,
            path: path,
            name: name,
            download_date: download_date,
            download_type: download_type,
            size:size,
            status:status,

        }

    }

    pub fn size(&self)->String{
        if let Some(size)=self.size{
            return format!("{}",size).into();
        }
        "".into()
    }

    pub fn url(&self) -> Cow<'_,Url> {
        Cow::Borrowed(&self.url)
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn download_date(&self) -> DateTime<Local> {
        let date = self.download_date;
        date.clone()
    }

    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("")
    }

    pub fn download_type(&self) -> &str{
        self.download_type.as_deref().unwrap_or("")
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
        \nDownload path:{:?}
        \nDownload name:{}
        \nDownload date:{}
        \nDownload type:{}
        \nDownload size(bytes):{}
        \nDownload status:{}."
        ,self.url().as_str(),
        self.path(),
        self.name(),
        self.download_date.to_string(),
        self.download_type(),
        self.size(),
        self.status(),
        ).as_str())   
    }
}
