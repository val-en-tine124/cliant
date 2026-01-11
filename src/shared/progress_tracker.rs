use std::{path::PathBuf, sync::Arc};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::RwLock;
use tracing::info;

use crate::shared::errors::CliantError;



/// Trait for progress tracking that any UI/interface can implement
/// This allows decoupling download logic from specific UI implementations (indicatif, GUI, web, etc.)
pub trait ProgressTracker: Send + Sync {
    ///Implement start functionality,initialization or logic here.
    async fn start(&self);
    /// Update progress with bytes written for a specific part/chunk
    async fn update(&self,bytes_written: usize);

    /// Mark entire download as complete
    async fn finish(&self);
}

pub struct CliProgressTracker {
    part_progress: Arc<RwLock<ProgressBar>>,
    download_path:PathBuf,
    download_name:String,
    total_bytes: Option<usize>,
}
impl CliProgressTracker {
    // Create a new progress tracker
    /// # Parameters
    /// * `total_bytes` - Total size of the download in bytes
    /// * `dowload_path` - Path to the download.
    pub fn new(total_bytes: Option<usize>,download_path:PathBuf) -> Result<Self,CliantError> {
        let progress = ProgressBar::new(total_bytes.unwrap_or(0) as u64);
        progress.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
    .unwrap()
    .progress_chars("##-"));
        let download_name=download_path.file_name().ok_or(CliantError::ParseError(format!("Invalid download path {}, can't get file name",download_path.display())))?.to_string_lossy().to_string();
        Ok(Self {
            part_progress: Arc::new(RwLock::new(progress)),
            download_path,
            download_name,
            total_bytes,
        })
    }
}

impl ProgressTracker for CliProgressTracker {
    
    async fn update(&self,bytes_written: usize){
        let progress = self.part_progress.write().await;
        progress.inc(bytes_written as u64);
    }
    
    async fn finish(&self) {
        
        let progress_bar = self.part_progress.read().await;
        progress_bar.finish_and_clear();
        let colored_string = format!(
            "Download '{}' Completed.\nTotal size: {}\n File path:{}"
        ,self.download_name.clone(),progress_bar.length().unwrap_or(0),self.download_path.clone().display())
        .purple();
        progress_bar.finish_with_message(colored_string.to_string());

        info!(
            total_bytes = self.total_bytes,
            downoad_name=self.download_name,
            download_path=self.download_path.to_str(),
            "Download completed successfully"
        );
    }
    
    async fn start(&self) {
        todo!()
    }
}
