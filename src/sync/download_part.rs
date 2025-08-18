//! # Download Part
//!
//! This module defines the `DownloadPart` struct, which represents a single
//! part of a larger file being downloaded. It handles the download of a specific
//! byte range and manages the state of that part.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use anyhow::Result;
use indicatif::ProgressBar;
use log::error;
use log::info;
use reqwest::blocking::Client;
use reqwest::header::RANGE;
use reqwest::Url;

use super::{retry_request, BrokenFilePart, FileMetaData, FileSystemIO, PartStatus};

/// Represents a single part of a file to be downloaded.
///
/// `DownloadPart` manages the download of a specific byte range of a file.
/// It keeps track of the download status, the path to the temporary part file,
/// and updates a progress bar if provided.
pub struct DownloadPart<'a, F: FileSystemIO> {
    url: Url,
    client: Client,
    bytes_start: u64,
    bytes_end: u64,
    part_name: String,
    status: PartStatus,
    part_path: PathBuf,
    fs: F,
    progress_bar: Option<&'a ProgressBar>,
}

impl<'a, F: FileSystemIO> DownloadPart<'a, F> {
    /// Creates a new `DownloadPart` instance.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the file being downloaded.
    /// * `bytes_range` - A slice containing the start and end byte offsets for this part.
    /// * `part_num` - The sequential number of this part.
    /// * `part_folder` - The directory where the temporary part file will be stored.
    /// * `client` - The HTTP client to use for the download.
    /// * `fs` - An implementation of the `FileSystemIO` trait.
    /// * `progress_bar` - An optional reference to an `indicatif::ProgressBar` to update progress.
    pub fn new(
        url: Url,
        bytes_range: &[u64],
        part_num: u32,
        part_folder: PathBuf,
        client: Client,
        fs: F,
        progress_bar: Option<&'a ProgressBar>,
    ) -> DownloadPart<'a, F> {
        let bytes_start: u64 = bytes_range[0];
        let bytes_end: u64 = bytes_range[1];
        let part_name = format!("part_{}.part", part_num);
        let part_path = Path::new(&part_folder).join(&part_name);

        DownloadPart {
            url,
            client,
            bytes_start,
            bytes_end,
            part_name,
            status: PartStatus::Starting,
            part_path,
            fs,
            progress_bar,
        }
    }
    fn get_metadata(&mut self) {
        match self.fs.metadata(&self.part_path) {
            Ok(part_metadata) => {
                let part_size_on_fs = part_metadata.len();
                if self.bytes_end == part_size_on_fs {
                    self.status = PartStatus::Completed(FileMetaData {
                        completed_bytes: part_size_on_fs as usize,
                        path: self.part_path.clone(),
                    });
                } else if part_size_on_fs > 0 && part_size_on_fs < self.bytes_end {
                    self.status = PartStatus::Broken(BrokenFilePart(part_size_on_fs));
                } else {
                    self.status = PartStatus::Starting; //Run if path does not exists on directory.
                }
            }
            Err(err) => {
                let part_name = &self.part_name;
                error!("Can't get part {part_name} metadata,Exception:{err}");
                process::exit(1);
            }
        }
    }

    /// Checks the status (If it exists, is broken or completed) of the download part on the file system.
    ///
    /// This method inspects the temporary part file to determine if it already
    /// exists, is complete, or is broken (partially downloaded).
    ///
    /// # Returns
    ///
    /// A mutable reference to `self` with the updated `status`.
    pub fn check_part(&mut self) -> &mut Self {
        info!("part path,{:?}", &self.part_path);
        let path_exists = std::fs::exists(&self.part_path);
        match path_exists {
            Ok(val) => {
                if val {
                    self.get_metadata();
                } else {
                    self.status = PartStatus::Starting;
                }
            }
            Err(err) => {
                let part_path = &self.part_path;
                error!("Can't check path {part_path:?} existence,{err}");
                process::exit(1);
            }
        }

        self
    }

    /// Initiates or resumes the download of the file part.
    ///
    /// Based on the current `PartStatus`, this method either starts a new download
    /// or resumes a broken one. It writes the downloaded bytes to the temporary
    /// part file.
    ///
    /// # Returns
    ///
    /// A `Result` containing `FileMetaData` for the completed part on success.
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

    /// Writes the downloaded bytes to the specified path.
    ///
    /// This private helper function handles the actual HTTP request for the byte
    /// range and writes the response body to the temporary part file. It also
    /// updates the progress bar.
    ///
    /// # Arguments
    ///
    /// * `bytes_offset` - The starting byte offset for the download.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    fn write_to_path(&mut self, bytes_offset: u64) -> Result<()> {
        let resp_result_fn = || {
            let response = self
                .client
                .get(self.url.as_ref())
                .header(RANGE, format!("bytes={}-{}", bytes_offset, self.bytes_end))
                .send()?;
            let response_result = response.error_for_status();
            response_result
        };

        let resp_retry_result = retry_request(3, resp_result_fn);

        match resp_retry_result {
            Ok(mut response) => {
                let mut part_file_handle = self.fs.create_file(&self.part_path)?;
                let mut buffer = [0; 1024];

                loop {
                    let bytes_read = response.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    part_file_handle.write_all(&buffer[..bytes_read])?;
                    if let Some(pb) = self.progress_bar {
                        pb.inc(bytes_read as u64);
                    }
                }

                let part_name = self.part_name.as_str();
                info!(" Part {part_name} Download completed.");
                Ok(())
            }

            Err(error) => Err(error),
        }
    }
}
