//! # Download Task
//!
//! This module defines the `DownloadTask` struct, which represents a single
//! file download operation. It handles the overall download process, including
//! splitting the file into parts, managing concurrent downloads of these parts,
//! and reassembling the final file.


use std::path::PathBuf;
use indicatif::{ProgressBar};

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use colored::Colorize;
use log::{error, info};
use rayon::prelude::*;
use reqwest::blocking::Client;
use reqwest::header::{HeaderValue, CONTENT_LENGTH};
use reqwest::Url;

use super::{FileMetaData, FileSystemIO};
use super::base_file_part::BaseFilePart;
use super::download_part::DownloadPart;
use super::retry_request;
use crate::split_parts::split_parts;

/// Represents a single file download task.
///
/// A `DownloadTask` manages the entire lifecycle of downloading a file,
/// from determining its size and splitting it into manageable parts, to
/// coordinating the parallel download of these parts and finally combining
/// them into the complete file.
pub struct DownloadTask<F: FileSystemIO> {
    download_url: Url,
    timestamp: DateTime<Local>,
    download_path: PathBuf,
    client: Client,
    content_length: u64,
    max_concurrent_part: u32,
    min_split_part_mb: u32,
    fs: F,
    progress_bar: Option<ProgressBar>,
    task_name:String,
}

impl<F: FileSystemIO + Clone + Send + Sync + 'static> DownloadTask<F> {
    /// Creates a new `DownloadTask` instance.
    ///
    /// This constructor performs a HEAD request to the download URL to determine
    /// the content length, which is crucial for splitting the file into parts.
    /// It initializes the task with the given URL, client, download configuration,
    /// and file system implementation.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the file to download.
    /// * `client` - The HTTP client to use for requests.
    /// * `max_concurrent_part` - The maximum number of parts to download concurrently.
    /// * `min_split_part_mb` - The minimum size of each part in megabytes.
    /// * `download_path` - The desired path for the completed download.
    /// * `fs` - An implementation of the `FileSystemIO` trait.
    /// * `progress_bar` - An optional `indicatif::ProgressBar` to track overall progress.
    /// * `task_name` - Name of the file to download.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `DownloadTask` instance on success.
    pub fn new(
        url: Url,
        client: Client,
        max_concurrent_part: u32,
        min_split_part_mb: u32,
        download_path: PathBuf,
        fs: F,
        progress_bar: Option<ProgressBar>,
        task_name:String,
    ) -> Result<DownloadTask<F>> {
        let timestamp = Local::now();

        let response_result_fn = || {
            let response = client.head(url.as_ref()).send();
            match response {
                Ok(resp) => resp.error_for_status(),
                Err(error) => {
                    error!("Couldn't get file size from server.");
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
                    progress_bar,
                    task_name,
                };

                Ok(download_task)
            }

            Err(error) => Err(error),
        }
    }

    /// Sets the progress bar for this download task.
    ///
    /// This method allows associating an `indicatif::ProgressBar` with the task
    /// for visual progress tracking.
    ///
    /// # Arguments
    ///
    /// * `progress_bar` - The `ProgressBar` instance to use.
    pub fn set_progress_bar(&mut self, progress_bar: ProgressBar) {
        self.progress_bar = Some(progress_bar);
    }

    /// Starts the download process for the task.
    ///
    /// This is the main method to initiate the file download. It calculates
    /// file parts, creates a temporary directory for them, downloads each part
    /// concurrently, concatenates them, and cleans up the temporary directory.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure of the download.
    pub fn start(&self) -> Result<()> {
        if let Some(pb) = &self.progress_bar {
            pb.set_length(self.content_length);
        }

        let parts = split_parts(
            self.content_length,
            self.max_concurrent_part,
            self.min_split_part_mb,
        );
        
        info!("parts split are {:?}",&parts);

        
        let part_folder = self.download_path.join("cliant_parts").join(&self.task_name);
        self.fs.create_dir_all(&part_folder)?;

        let tasks: Vec<Result<FileMetaData>> = parts
            .par_iter()
            .enumerate()
            .map(|(idx, parts)| {
                let mut download_part = DownloadPart::new(
                    self.download_url.clone(),
                    parts,
                    idx as u32,
                    part_folder.clone(),
                    self.client.clone(),
                    self.fs.clone(),
                    self.progress_bar.as_ref(),
                );
                let part_meta_data = download_part.check_part().get_part();

                part_meta_data
            })
            .collect();
        let mut completed_parts = Vec::new();
        info!("gathering parts metadata.");
        for task in tasks {
            completed_parts.push(task?);
        }
        info!("Joining path...");
        self.concat_file_part(completed_parts)?;
        info!("Removing path dir...");
        self.fs.remove_dir_all(&part_folder)?; // Remove the parts directory and it content here.
        info!("cliant_path dir removed.");

        if let Some(pb) = &self.progress_bar {
            pb.finish_with_message("downloaded");
        }

        Ok(())
    }

    /// Concatenates all downloaded file parts into the final base file.
    ///
    /// This private helper method takes a list of completed file parts and uses
    /// `BaseFilePart` to combine them sequentially into the target download file.
    ///
    /// # Arguments
    ///
    /// * `file_parts` - A vector of `FileMetaData` representing the completed parts.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `BaseFilePart` instance representing the final file.
    fn concat_file_part(&self, file_parts: Vec<FileMetaData>) -> Result<BaseFilePart<F>> {
        let mut base_file = BaseFilePart::new(self.download_path.clone(),self.task_name.clone(), 0, self.fs.clone(),)?;
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
        Some(self.cmp(other))
    }
}
