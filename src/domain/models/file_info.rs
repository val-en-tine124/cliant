use std::path::PathBuf;
use derive_getters::Getters;
///# ``FileInfo``
///A struct for representing a file information on a file system.
/// * Path : Absolute path of the file on the file system.
/// * ``file_name`` : Name of the file on the file system.
/// * ``file_size`` : Size of the file(bytes) in the file system.
#[derive(Getters,Debug,Clone)]
pub struct FileInfo {
    path: PathBuf,
    file_size: usize,
    file_name: String,
}

impl FileInfo {
    pub fn new(path: PathBuf, size: usize, name: String,) -> Self {
        Self {
            path,
            file_size: size,
            file_name: name,
        }
    }
}
