use std::path::Path;

use crate::domain::errors::DomainError;
use crate::domain::ports::storage_service::FileIO;
pub struct MergeParts<'a,F>{
    fs:F,
    path:&'a Path,
}

impl<'a,F:FileIO> MergeParts<'a,F>{
    pub async fn new(path:&'a Path,fs:F)-> Result<Self,DomainError>{
        if !fs.file_exists(path).await?{
        fs.create_file(path).await?;
        return Ok(Self{path:path,fs:fs});
        }
        Ok(Self {path:path,fs:fs})
    }
    pub async fn merge(&self,buf:&[u8])->Result<&Self,DomainError>{
        self.fs.append_to_file(buf, self.path).await?;
        Ok(&self)
    }
}