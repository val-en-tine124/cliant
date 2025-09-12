use std::path::Path;
///# FileInfo
///A struct for representing a file information on a file system.
/// * Path : Absolute path of the file on the file system.
/// * file_name : Name of the file on the file system.
/// * file_size : Size of the file(bytes) in the file system.
pub struct FileInfo<'a>{
    path:&'a Path,
    file_size:usize,
    file_name:String,
}

impl<'a> FileInfo<'a>{
    pub fn new(path:&'a Path,size:usize,name:String,)->Self{
        Self{path:path,file_size:size,file_name:name}
    }
    
    pub fn name(&self)->&String{
        &self.file_name
    }
    pub fn size(&self)->usize{
        self.file_size
    }

    pub fn path(&self)->&Path{
        self.path
    }
}