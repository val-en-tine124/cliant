use chrono::{DateTime, Local};
use url::Url;

/// # DownloadInfo
/// A struct to represent a Download file information.
/// ### Parameters:
/// * url : This is the download url.
/// #### Optional parameters:
/// * name : Name of the download.
/// * size : Size of the download in bytes.
/// download_date : Date of the download as a Datetime<Local> (from chrono crate) representation.
/// download_type : MIME representation of the download e.g video/mp4, audio/mp3.

#[derive(Clone)]
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
            url: url,
            name: name,
            size: size,
            download_type: download_type,
            download_date: download_date,
        }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    pub fn size(&self) -> Option<usize> {
        self.size
    }
    pub fn download_date(&self) -> &DateTime<Local> {
        &self.download_date
    }
    pub fn download_type(&self) -> Option<&str> {
        self.download_type.as_deref()
    }
}
