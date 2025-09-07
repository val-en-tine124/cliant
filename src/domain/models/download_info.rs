use chrono::{DateTime,Local};

#[derive(Clone)]
pub struct DownloadInfo{
    name:Option<String>,
    size:Option<usize>,
    download_date:DateTime<Local>,
    download_type:Option<String>,
}

impl DownloadInfo{
    pub fn new(
        name:Option<String>,
        size:Option<usize>,
        download_date:DateTime<Local>,
        download_type:Option<String>,)->Self{
            Self{name:name,size:size,download_type:download_type,download_date:download_date}
    }
    pub fn name(&self)->&Option<String>{
        &self.name
    }
    pub fn size(&self)->Option<usize>{
        self.size
    }
    pub fn download_date(&self)->&DateTime<Local>{
        &self.download_date
    }
    pub fn download_type(&self)->&Option<String>{
        &self.download_type
    }
}   