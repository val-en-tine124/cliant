use chrono::{DateTime, Local};
use derive_getters::Getters;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Getters)]
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
        Self { src, dst, file_size, file_name, timestamp }
    }
}

#[derive(Serialize, Getters)]
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
        Self { file_path, file_size, file_name, timestamp }
    }
}
