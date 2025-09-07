use std::path::PathBuf;

pub struct DownloadPart{
    path:PathBuf,
    size:usize,
}

impl DownloadPart{
    pub fn new(path:PathBuf,size:usize)->Self{
        DownloadPart{
            path:path,
            size:size,
        }
    }

    pub fn size(&self)->usize{
        self.size
    }
    pub fn path(&self)->&PathBuf{
        &self.path
    }
}