use crate::domain::ports::progress_tracker::{ProgressInfo, ProgressTracker};
use async_trait::async_trait;
use colored::Colorize;
use indicatif::{self, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Instant;
use tracing::info;

/// Default progress tracker implementation using thread-safe in-memory storage
/// Tracks progress for each part independently and aggregates total progress
pub struct DefaultProgressTracker {
    part_progress: Arc<RwLock<HashMap<usize, usize>>>,
    completed_parts: Arc<RwLock<usize>>,
    total_bytes: usize,
    total_parts: usize,
}

impl DefaultProgressTracker {
    /// Create a new progress tracker
    /// # Parameters
    /// * `total_bytes` - Total size of the download in bytes
    /// * `total_parts` - Number of parts/chunks to download
    pub fn new(total_bytes: usize, total_parts: usize) -> Self {
        Self {
            part_progress: Arc::new(RwLock::new(HashMap::new())),
            completed_parts: Arc::new(RwLock::new(0)),
            total_bytes,
            total_parts,
        }
    }
}

#[async_trait]
impl ProgressTracker for DefaultProgressTracker {
    async fn update(&self, part_id: usize, bytes_written: usize) {
        let mut progress = self.part_progress.write().await;
        progress.insert(part_id, bytes_written);
    }

    async fn complete_part(&self, part_id: usize, total_bytes: usize) {
        let mut progress = self.part_progress.write().await;
        progress.insert(part_id, total_bytes);

        let mut completed = self.completed_parts.write().await;
        *completed += 1;

        info!(
            part_id = part_id,
            bytes = total_bytes,
            "Part completed successfully"
        );
    }

    async fn fail_part(&self, part_id: usize, error: String) {
        tracing::error!(part = part_id, "Part failed: {}", error);
    }

    async fn total_progress(&self) -> ProgressInfo {
        let progress = self.part_progress.read().await;
        let downloaded = progress.values().sum();
        let completed = *self.completed_parts.read().await;

        ProgressInfo {
            total_bytes: self.total_bytes,
            downloaded_bytes: downloaded,
            completed_parts: completed,
            total_parts: self.total_parts,
        }
    }

    async fn finish(&self) {
        let progress = self.total_progress().await;
        info!(
            total_bytes = progress.total_bytes,
            completed_parts = progress.completed_parts,
            total_parts = progress.total_parts,
            "Download completed successfully"
        );
    }
}

pub struct CliProgressTracker {
    elapsed: Instant,
    part_progress: Arc<RwLock<ProgressBar>>,
    completed_parts: Arc<RwLock<usize>>,
    total_bytes: usize,
    total_parts: usize,
}
impl CliProgressTracker {
    // Create a new progress tracker
    /// # Parameters
    /// * `total_bytes` - Total size of the download in bytes
    /// * `total_parts` - Number of parts/chunks to download
    pub fn new(total_bytes: usize, total_parts: usize) -> Self {
        let elapsed = Instant::now();
        let progress = ProgressBar::new(total_bytes as u64);
        progress.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
    .unwrap()
    .progress_chars("##-"));

        Self {
            elapsed,
            part_progress: Arc::new(RwLock::new(progress)),
            completed_parts: Arc::new(RwLock::new(0)),
            total_bytes,
            total_parts,
        }
    }
}

#[async_trait]
impl ProgressTracker for CliProgressTracker {
    async fn complete_part(&self, part_id: usize, total_bytes: usize) {
        let mut completed = self.completed_parts.write().await;
        *completed += 1;

        info!(
            part_id = part_id,
            bytes = total_bytes,
            "Part completed successfully"
        );
    }
    async fn update(&self, part_id: usize, bytes_written: usize) {
        let progress = self.part_progress.write().await;
        progress.inc(bytes_written as u64);
    }
    async fn fail_part(&self, part_id: usize, error: String) {
        tracing::error!(part = part_id, "Part failed: {}", error);
    }
    async fn total_progress(&self) -> ProgressInfo {
        let progress = self.part_progress.read().await;
        let downloaded = progress.position();
        let completed = *self.completed_parts.read().await;

        ProgressInfo {
            total_bytes: self.total_bytes,
            downloaded_bytes: downloaded as usize,
            completed_parts: completed,
            total_parts: self.total_parts,
        }
    }
    async fn finish(&self) {
        let progress = self.total_progress().await;
        let progress_bar = self.part_progress.read().await;
        let colored_string = format!(
            "total bytes: 
        {}\ntotal bytes: {}\ntotal parts: {}\n
        Download completed successfully",
            progress.total_bytes,
            progress.completed_parts,
            progress.total_parts
        )
        .purple();
        progress_bar.finish_with_message(colored_string.to_string());

        info!(
            total_bytes = progress.total_bytes,
            completed_parts = progress.completed_parts,
            total_parts = progress.total_parts,
            "Download completed successfully"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_tracker() {
        let tracker = DefaultProgressTracker::new(10000, 4);

        // Update part 0 with 1000 bytes
        tracker.update(0, 1000).await;
        let progress = tracker.total_progress().await;
        assert_eq!(progress.downloaded_bytes, 1000);
        assert_eq!(progress.completed_parts, 0);

        // Complete part 0
        tracker.complete_part(0, 2500).await;
        let progress = tracker.total_progress().await;
        assert_eq!(progress.downloaded_bytes, 2500);
        assert_eq!(progress.completed_parts, 1);

        // Complete remaining parts
        tracker.complete_part(1, 2500).await;
        tracker.complete_part(2, 2500).await;
        tracker.complete_part(3, 2500).await;

        let progress = tracker.total_progress().await;
        assert_eq!(progress.downloaded_bytes, 10000);
        assert_eq!(progress.completed_parts, 4);
        assert_eq!(progress.percentage(), 100.0);
    }

    #[tokio::test]
    async fn test_progress_percentage() {
        let tracker = DefaultProgressTracker::new(1000, 2);
        tracker.update(0, 500).await;
        let progress = tracker.total_progress().await;
        assert_eq!(progress.percentage(), 50.0);
    }
}
