use chrono::{DateTime, Local};
use derive_getters::Getters;
use std::path::Path;
use url::Url;

#[derive(Debug, Clone, Getters)]
pub struct MultiPartCommand<'a> {
    url: Url,
    path: &'a Path,
    max_no_frames: usize,
    min_frame_size_mb: usize,
    timestamp: DateTime<Local>,
}

impl<'a> MultiPartCommand<'a> {
    pub fn new(
        url: Url,
        path: &'a Path,
        max_no_frames: usize,
        min_frame_size_mb: usize,
    ) -> Self {
        Self {
            url,
            path,
            max_no_frames,
            min_frame_size_mb,
            timestamp: Local::now(),
        }
    }
}
