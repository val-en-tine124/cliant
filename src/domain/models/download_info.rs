use chrono::{DateTime, Local};
use serde::Serialize;
use url::Url;
use derive_getters::Getters;

/// # DownloadInfo
/// A struct to represent a Download file information.
/// ### Parameters:
/// * url : This is the download url.
/// #### Optional parameters:
/// * name : Name of the download.
/// * size : Size of the download in bytes.
/// download_date : Date of the download as a Datetime<Local> (from chrono crate) representation.
/// download_type : MIME representation of the download e.g video/mp4, audio/mp3.

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
