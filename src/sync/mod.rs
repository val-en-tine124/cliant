pub mod download_manager;
mod download_task;
mod download_part;
mod base_file_part;
mod check_name;

use std::path::{Path,PathBuf};
use std::fs::{self,File};
use anyhow::{Result,Context};
use colored::Colorize;
use log::{error, info};
use reqwest::blocking::Response;
use std::time::Duration;



#[derive(Clone)]
enum PartStatus {
    Starting,
    Started,
    Completed(FileMetaData),
    Broken(BrokenFilePart),
}
#[derive(Clone)]
struct FileMetaData {
    path: PathBuf,
    completed_bytes: usize,
}

#[derive(Clone)]
struct BrokenFilePart(u64);

#[derive(Clone)]
pub struct DiskFileSystem;

impl FileSystemIO for DiskFileSystem {
    fn create_dir_all(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path).context("Failed to create directory")
    }

    fn create_file(&self, path: &Path) -> Result<File> {
        File::create(path).context("Failed to create file")
    }

    fn open_file(&self, path: &Path) -> Result<File> {
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .context(format!("Can't open path:{:?}", path).yellow())
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        fs::remove_file(path).context("Failed to remove file")
    }

    fn metadata(&self, path: &Path) -> Result<fs::Metadata> {
        fs::metadata(path).context("Failed to get metadata")
    }

    fn remove_dir_all(&self, path: &Path) -> Result<()> {
        fs::remove_dir_all(path).context("Failed to remove directory")
    }
}


pub trait FileSystemIO {
    fn create_dir_all(&self, path: &Path) -> Result<()>;
    fn create_file(&self, path: &Path) -> Result<File>;
    fn open_file(&self, path: &Path) -> Result<File>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<fs::Metadata>;
    fn remove_dir_all(&self, path: &Path) -> Result<()>;
}

pub fn retry_request<F>(max_retry_no: u8, function: F) -> Result<Response, anyhow::Error>
where
    F: Fn() -> Result<Response, reqwest::Error>,
{
    for current_retry in 1..=max_retry_no {
        match function() {
            Ok(response) => {
                return Ok(response);
            }
            Err(error) if error.is_connect() || error.is_timeout() || error.is_request() => {
                if let Some(err_url) = error.url() {
                    let url = err_url.clone();
                    info!("Can't get http response body for url {url}");
                }
                error!("Network error, retrying HTTP request {current_retry}...");
                std::thread::sleep(Duration::from_millis(10000));
                continue;
            }
            Err(error) => {
                if let Some(err_url) = error.url() {
                    let url = err_url.clone();
                    info!("Can't get http response body for url {url}");
                }

                return Err(error.into());
            }
        }
    }
    anyhow::bail!(format!("Spurious network operation timeout.").yellow());
}
