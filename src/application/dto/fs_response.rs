use std::path::Path;
use chrono::{DateTime, Local};
use serde::Serialize;

#[derive(Serialize)]
pub struct FileMoveResponse<'a> {
    src: &'a Path,
    dst: &'a Path,
    file_size: usize,
    file_name: String,
    timestamp: &'a DateTime<Local>,
}

impl<'a> FileMoveResponse<'a> {
    pub fn new(
        src: &'a Path,
        dst: &'a Path,
        file_size: usize,
        file_name: String,
        timestamp: &'a DateTime<Local>,
    ) -> Self {
        Self {
            src:src,
            dst: dst,
            file_size: file_size,
            file_name: file_name,
            timestamp: timestamp,
        }
    }
    pub fn src_path(&self) -> &'a Path {
        &self.src
    }

    pub fn dst_path(&self) -> &'a Path {
        &self.dst
    }

    pub fn file_size(&self) -> usize {
        self.file_size
    }

    pub fn file_name(&self) -> &String {
        &self.file_name
    }

    pub fn timestamp(&self) -> &DateTime<Local> {
        &self.timestamp
    }
}

#[derive(Serialize)]
pub struct FileDeleteResponse<'a> {
    file_path: &'a Path,
    file_size: usize,
    file_name: String,
    timestamp: &'a DateTime<Local>,
}


impl<'a> FileDeleteResponse<'a> {
    pub fn new(
        file_path: &'a Path,
        file_size: usize,
        file_name: String,
        timestamp: &'a DateTime<Local>,
    ) -> Self {
        Self {
            file_path: file_path,
            file_size: file_size,
            file_name: file_name,
            timestamp: timestamp,
        }
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
    pub fn timestamp(&self) -> &DateTime<Local> {
        &self.timestamp
    }

    pub fn file_size(&self) -> usize {
        self.file_size
    }

    pub fn file_name(&self) -> &String {
        &self.file_name
    }
}


