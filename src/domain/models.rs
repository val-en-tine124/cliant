use std::path::PathBuf;
use derive_getters::Getters;

use chrono::{DateTime, Local};
use serde::Serialize;
use url::Url;

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


/// # ``DownloadInfo``
/// A struct to represent a Download file information.
/// ### Parameters:
/// * url : This is the download url.
/// #### Optional parameters:
/// * name : Name of the download.
/// * size : Size of the download in bytes.
/// ``download_date`` : Date of the download as a Datetime<Local> (from chrono crate) representation.
/// ``download_type`` : MIME representation of the download e.g video/mp4, audio/mp3.

#[derive(Clone,Debug, Serialize,Getters)]
pub struct DownloadInfo{
    url: Url,
    name: Option<String>,
    size: Option<usize>,
    download_date: DateTime<Local>,
    download_type: Option<String>,
}

impl DownloadInfo{
    pub fn new(
        url: Url,
        name: Option<String>,
        size: Option<usize>,
        download_date: DateTime<Local>,
        download_type: Option<String>,
    ) -> Self {
        Self {
            url,
            name,
            size,
            download_date,
            download_type,
        }
    }


}
