use path::Path;
use chrono::{Local,DateTime};

pub struct DeleteFile<'a>{
    file_path:&'a Path,
    timestamp:DateTime<Local>,
}

impl<'a> DeleteFile<'a>{
    pub fn new(path:&Path)->Self{
        Self{
            file_path:path,
            timestamp:Local::now(),
        }
    }
    pub fn file_path(&self)->&Path{
        &self.file_path
    }
    pub fn timestamp(&self)->&DateTime<Local>{
        &self.timestamp
    }
}

pub struct MoveFile<'a>{
    src:&'a Path,
    dst:&'a Path,
    timestamp:DateTime<Local>,
}
impl<'a> MoveFile<'a>{
    pub fn new(src:&'a Path,dst:&'a Path,)->Self{
        Self{
            src:src,
            dst:dst,
            timestamp:Local::now(),
        }
    }
    pub fn src_path(&self)->&'a Path{
        &self.src
    }

    pub fn dst_path(&self)->&'a Path{
        &self.dst
    }

    pub fn timestamp(&self)->&DateTime<Local>{
        &self.timestamp
    }

}