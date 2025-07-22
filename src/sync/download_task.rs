use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use colored::Colorize;
use log::error;
use rayon::prelude::*;
use reqwest::blocking::Client;
use reqwest::header::{HeaderValue, CONTENT_LENGTH};
use reqwest::Url;

use super::{FileSystemIO,FileMetaData};
use super::retry_request;
use super::download_part::DownloadPart;
use super::base_file_part::BaseFilePart;
use crate::split_parts::split_parts;

pub struct DownloadTask<F: FileSystemIO> {
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
    pub fn new(
        url: Url,
        client: Client,
        max_concurrent_part: u32,
        min_split_part_mb: u32,
        download_path: PathBuf,
        fs: F,
    ) -> Result<DownloadTask<F>> {
        let timestamp = Local::now();

        //might replace content_length implementations.

        let response_result_fn = || {
            let response = client.head(url.as_ref()).send();
            match response {
                Ok(resp) => {
                    let response_result = resp.error_for_status();
                    response_result
                }
                Err(error) => {
                    error!("Could'nt get file size from server.");
                    Err(error)
                }
            }
        };

        let resp_retry_result = retry_request(3, response_result_fn);
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