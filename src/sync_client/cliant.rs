use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use colored::Colorize;
use log::{error, info};
use rayon::prelude::*;
use reqwest::blocking::Client;
use reqwest::header::{HeaderValue, CONTENT_LENGTH, RANGE};
use reqwest::Url;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{self, BufWriter};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::sync_client::check_name::check_name;
use crate::sync_client::split_parts::split_parts;
use super::utils::retry_request;

pub trait FileSystemIO {
    fn create_dir_all(&self, path: &Path) -> Result<()>;
    fn create_file(&self, path: &Path) -> Result<File>;
    fn open_file(&self, path: &Path) -> Result<File>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<fs::Metadata>;
    fn remove_dir_all(&self, path: &Path) -> Result<()>;
}

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
/// BaseFilePart contains method for adding File bytes to a base path.
struct BaseFilePart<F: FileSystemIO> {
    path_handle: BufWriter<File>,
    file_data: FileMetaData,
    fs: F,
}

impl<F: FileSystemIO> BaseFilePart<F> {
    fn new(path: PathBuf, completed_bytes: usize, fs: F) -> Result<BaseFilePart<F>> {
        let file_handle = fs.open_file(&path)?;
        let self_buf_writer = std::io::BufWriter::new(file_handle);

        Ok(BaseFilePart {
            path_handle: self_buf_writer,
            file_data: FileMetaData {
                completed_bytes: completed_bytes,
                path: path,
            },
            fs,
        })
    }

    fn add(&mut self, rhs: &FileMetaData) -> Result<()> {
        let mut rhs_handle = self.fs.open_file(&rhs.path)?;
        io::copy(&mut rhs_handle, &mut self.path_handle)?;
        self.file_data.completed_bytes += rhs.completed_bytes;
        self.fs.remove_file(&rhs.path)?;
        Ok(())
    }
}

struct DownloadPart<F: FileSystemIO> {
    url: Url,
    client: Client,
    bytes_start: u64,
    bytes_end: u64,
    part_name: String,
    status: PartStatus,
    part_path: PathBuf,
    fs: F,
}

impl<F: FileSystemIO> DownloadPart<F> {
    fn new(
        url: Url,
        bytes_range: &[u64],
        part_num: u32,
        part_folder: PathBuf,
        client: Client,
        fs: F,
    ) -> DownloadPart<F> {
        let bytes_start: u64 = bytes_range[0] as u64;
        let bytes_end: u64 = bytes_range[1] as u64;
        let part_name = format!("part_{}.part", part_num);
        let part_path = Path::new(&part_folder).join(&part_name);

        return DownloadPart {
            url: url,
            client: client,
            bytes_start: bytes_start,
            bytes_end: bytes_end,
            part_name: part_name,
            status: PartStatus::Starting,
            part_path: part_path,
            fs,
        };
    }

    pub fn check_part(&mut self) -> &mut Self {
        if let Ok(part_metadata) = self.fs.metadata(&self.part_path) {
            let part_size_on_fs = part_metadata.file_size();
            if self.bytes_end == part_size_on_fs {
                self.status = PartStatus::Completed(FileMetaData {
                    completed_bytes: part_size_on_fs as usize,
                    path: self.part_path.clone(),
                });
            } else if part_size_on_fs > 0 && part_size_on_fs < self.bytes_end {
                self.status = PartStatus::Broken(BrokenFilePart(part_size_on_fs));
            } else {
                self.status = PartStatus::Starting;
            }
        } else {
            self.status = PartStatus::Starting;
        }
        self
    }

    pub fn get_part(&mut self) -> Result<FileMetaData> {
        let status = self.status.clone();

        match status {
            PartStatus::Starting => {
                self.status = PartStatus::Started;
                self.write_to_path(self.bytes_start)?;
                let file_info = FileMetaData {
                    completed_bytes: self.bytes_end as usize,
                    path: self.part_path.clone(),
                };

                self.status = PartStatus::Completed(file_info.clone());
                Ok(file_info)
            }

            PartStatus::Completed(completed_file_part) => Ok(completed_file_part),

            PartStatus::Broken(BrokenFilePart(broken_bytes)) => {
                self.status = PartStatus::Started;
                self.write_to_path(broken_bytes)?;
                let file_info = FileMetaData {
                    completed_bytes: self.bytes_end as usize,
                    path: self.part_path.clone(),
                };
                self.status = PartStatus::Completed(file_info.clone());
                Ok(file_info)
            }

            _ => todo!(),
        }
    }

    fn write_to_path(&self, bytes_offset: u64) -> Result<()> {
        let resp_result_fn=||{
            let response = self
            .client
            .get(self.url.as_ref())
            .header(RANGE, format!("bytes={}-{}", bytes_offset, self.bytes_end))
            .send()?;
        let response_result = response.error_for_status();
        response_result
        };

        let resp_retry_result=retry_request(3, resp_result_fn);

        match resp_retry_result {
            Ok(mut response) => {
                let mut part_file_handle = self.fs.create_file(&self.part_path)?;
                io::copy(&mut response, &mut part_file_handle)?;
                let part_name = self.part_name.as_str();
                info!(" Part {part_name} Download completed.");
                return Ok(());
            }

            Err(error) => {
                return Err(error)
            }
        }
    }
}

struct DownloadTask<F: FileSystemIO> {
    download_url: Url,
    timestamp: DateTime<Local>,
    download_path: PathBuf,
    client: Client,
    content_length: u64,
    max_concurrent_part: u32,
    min_split_part_mb: u32,
    fs: F,
}

impl<F: FileSystemIO + Clone + Send + Sync + 'static> DownloadTask<F> {
    fn new(
        url: Url,
        client: Client,
        max_concurrent_part: u32,
        min_split_part_mb: u32,
        download_path: PathBuf,
        fs: F,
    ) -> Result<DownloadTask<F>> {
        let timestamp = Local::now();

        //might replace content_length implementations.
        

        let response_result_fn =|| { 
            let response=client
            .head(url.as_ref())
            .send();
        match response{
            Ok(resp)=>{
                let response_result = resp.error_for_status();
                response_result
            }
            Err(error)=>{
                error!("Could'nt get file size from server.");
                Err(error)
            }
        }
        
        };

        let resp_retry_result=retry_request(3,response_result_fn);
        match resp_retry_result {
            Ok(response) => {
                let default_content_length = HeaderValue::from_str("0")?;
                let content_length_str = response
                    .headers()
                    .get(CONTENT_LENGTH)
                    .unwrap_or(&default_content_length)
                    .to_str()?;

                let content_length_int: u64 = content_length_str
                    .parse::<u64>()
                    .context(format!("Can't get content length.").yellow())?;
                let download_task = DownloadTask {
                    download_url: url,
                    download_path: download_path,
                    client: client,
                    timestamp: timestamp,
                    content_length: content_length_int,
                    max_concurrent_part: max_concurrent_part,
                    min_split_part_mb: min_split_part_mb,
                    fs,
                };

                Ok(download_task)
            }
            
            Err(error) => {
                return Err(error);
            }
        }
    }

    pub fn start(&self) -> Result<()> {
        let parts = split_parts(
            self.content_length,
            self.max_concurrent_part,
            self.min_split_part_mb,
        );

        let part_abs_folder = self.download_path.clone();
        self.fs.create_dir_all(&part_abs_folder)?;

        let tasks: Vec<Result<FileMetaData>> = parts
            .par_iter()
            .enumerate()
            .map(|(idx, parts)| {
                let mut download_part = DownloadPart::new(
                    self.download_url.clone(),
                    parts,
                    idx as u32,
                    part_abs_folder.clone(),
                    self.client.clone(),
                    self.fs.clone(),
                );
                download_part.check_part().get_part()
            })
            .collect();
        let mut completed_parts = Vec::new();
        for task in tasks {
            completed_parts.push(task?);
        }
        self.concat_file_part(completed_parts)?;

        self.fs.remove_dir_all(&part_abs_folder)?; // Remove the parts directory and it content here.

        Ok(())
    }

    ///Add the CompleteFilePart types together and return a base_file(the summation of the CompleteFilePart types).
    fn concat_file_part(&self, file_parts: Vec<FileMetaData>) -> Result<BaseFilePart<F>> {
        let mut base_file = BaseFilePart::new(self.download_path.clone(), 0, self.fs.clone())?;
        for file_part in &file_parts {
            base_file.add(file_part)?;
        }
        Ok(base_file)
    }
}

impl<F: FileSystemIO> Ord for DownloadTask<F> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl<F: FileSystemIO> PartialEq for DownloadTask<F> {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
    }
}
impl<F: FileSystemIO> Eq for DownloadTask<F> {}

impl<F: FileSystemIO> PartialOrd for DownloadTask<F> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.timestamp.partial_cmp(&(other.timestamp))
    }
    fn ge(&self, other: &Self) -> bool {
        self.timestamp >= other.timestamp
    }

    fn le(&self, other: &Self) -> bool {
        self.timestamp <= other.timestamp
    }
}

pub struct DownloadManager<F: FileSystemIO> {
    tasks: BTreeSet<DownloadTask<F>>,
}

impl<F: FileSystemIO + Clone + Send + Sync + 'static> DownloadManager<F> {
    pub fn new(
        urls: Vec<Url>,
        client: Client,
        max_concurrent_part: u32,
        split_part_min_mb: u32,
        fs: F,
    ) -> Result<DownloadManager<F>> {
        let mut tasks = BTreeSet::new();
        for url in urls {
            let filename = check_name(url.clone(), &client)?;
            let cliant_root = env::var("CLIANT_ROOT").unwrap_or(".".to_string());
            let download_path = PathBuf::from(cliant_root).join(&filename);
            let _ = tasks.insert(DownloadTask::new(
                url,
                client.clone(),
                max_concurrent_part,
                split_part_min_mb,
                download_path,
                fs.clone(),
            )?);
        }
        Ok(DownloadManager { tasks: tasks })
    }

    pub fn start_tasks(&self) -> Result<()> {
        for task in self.tasks.iter().rev() {
            task.start()?;
        }
        Ok(())
    }
}
