use std::io;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};


use log::info;
use reqwest::blocking::Client;
use reqwest::header::RANGE;
use reqwest::Url;
use anyhow::Result;

use super::{FileSystemIO,PartStatus,FileMetaData,BrokenFilePart,retry_request,};



pub struct DownloadPart<F: FileSystemIO> {
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
    pub fn new(
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
                io::copy(&mut response, &mut part_file_handle)?;
                let part_name = self.part_name.as_str();
                info!(" Part {part_name} Download completed.");
                return Ok(());
            }

            Err(error) => return Err(error),
        }
    }
}

