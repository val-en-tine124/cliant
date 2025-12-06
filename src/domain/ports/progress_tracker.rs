use async_trait::async_trait;

/// Information about overall download progress
#[derive(Clone, Debug)]
pub struct ProgressInfo {
    pub total_bytes: usize,
    pub downloaded_bytes: usize,
    pub completed_parts: usize,
    pub total_parts: usize,
}

impl ProgressInfo {
    /// Calculate progress percentage (0.0 to 100.0)
    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.downloaded_bytes as f64 / self.total_bytes as f64) * 100.0
        }
    }
}

/// Trait for progress tracking that any UI/interface can implement
/// This allows decoupling download logic from specific UI implementations (indicatif, GUI, web, etc.)
#[async_trait]
pub trait ProgressTracker: Send + Sync {
    /// Update progress with bytes written for a specific part/chunk
    async fn update(&self, part_id: usize, bytes_written: usize);

    /// Mark a part as completed
    async fn complete_part(&self, part_id: usize, total_bytes: usize);

    /// Mark a part as failed
    async fn fail_part(&self, part_id: usize, error: String);

    /// Get total progress across all parts
    async fn total_progress(&self) -> ProgressInfo;

    /// Mark entire download as complete
    async fn finish(&self);
}

/// No-op progress tracker for when progress tracking is not needed
pub struct NoOpProgressTracker;

#[async_trait]
impl ProgressTracker for NoOpProgressTracker {
    async fn update(&self, _part_id: usize, _bytes_written: usize) {}

    async fn complete_part(&self, _part_id: usize, _total_bytes: usize) {}

    async fn fail_part(&self, _part_id: usize, _error: String) {}

    async fn total_progress(&self) -> ProgressInfo {
        ProgressInfo {
            total_bytes: 0,
            downloaded_bytes: 0,
            completed_parts: 0,
            total_parts: 0,
        }
    }

    async fn finish(&self) {}
}
