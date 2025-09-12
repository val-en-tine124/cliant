use url::Url;
use chrono::{Local,DateTime};

pub struct StartDownload{
    url:Url,
    multi_part:bool,
    timestamp:DateTime<Local>,
}

impl StartDownload{
    pub fn new(url:Url,multi_part:bool)->Self{
        Self{
            url,
            multi_part:multi_part,
            timestamp:Local::now(),
        }
    }

    pub fn url(&self)->&Url{
        &self.url
    }
    pub fn multi_part(&self)->bool{
        self.multi_part
    }
    pub fn timestamp(&self)->&DateTime<Local>{
        &self.timestamp
    }

}