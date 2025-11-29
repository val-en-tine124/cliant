use url::Url;
use std::path::Path;
use chrono::{Local,DateTime};

pub struct MultiPartCommand<'a>{
    url:Url,
    path:&'a Path,
    max_no_frames: usize,
    min_frame_size_mb: usize,
    timestamp:DateTime<Local>,
}

impl<'a> MultiPartCommand<'a>{
    pub fn new(url:Url,path:&'a Path,max_no_frames: usize,min_frame_size_mb: usize,)->Self{
        Self{
            url,
            path,
            max_no_frames,
            min_frame_size_mb,
            timestamp:Local::now(),
        }
    }

    pub fn url(&self) -> &Url{
        &self.url
    }

    pub fn frames_no(&self)->usize{
        self.max_no_frames
    }

    pub fn frame_size(&self)->usize{
        self.min_frame_size_mb
    }

    pub fn path(&self)->&Path{
        &self.path
    }
    
    pub fn command_timestamp(&self)->&DateTime<Local>{
        &self.timestamp
    }

}