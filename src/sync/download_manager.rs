//! # Download Manager
//!
//! This module defines the `DownloadManager` struct, which orchestrates the
//! download of multiple files concurrently. It manages download tasks and
//! provides progress tracking.
#![allow(unused)]
use std::env;
use std::path::PathBuf;
use std::{thread,fs};

use log::info;
use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar};
use reqwest::blocking::Client;
use reqwest::Url;




use super::download_task::DownloadTask;
use super::{check_name::check_name, FileSystemIO};

/// Manages the download of multiple files.
///
/// The `DownloadManager` is responsible for creating and running multiple
/// `DownloadTask` instances. It uses `indicatif`'s `MultiProgress` to display
/// and manage progress bars for each concurrent download.
pub struct DownloadManager<F: FileSystemIO> {
    tasks: Vec<DownloadTask<F>>,
}

impl<F: FileSystemIO + Clone + Send + Sync + 'static> DownloadManager<F> {
    /// Creates a new `DownloadManager` instance.
    ///
    /// Initializes the manager with a list of URLs to download, an HTTP client,
    /// configuration for concurrent parts, minimum split part size, and a
    /// file system implementation.
    /// If file has been downloaded in previous sessions inform user and exit process.
    ///
    /// # Arguments
    ///
    /// * `urls` - A vector of URLs to download.
    /// * `client` - The HTTP client to use for downloads.
    /// * `max_concurrent_part` - Optional maximum number of concurrent parts per download.
    /// * `split_part_min_mb` - Minimum size of each download part in megabytes.
    /// * `fs` - An implementation of the `FileSystemIO` trait.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `DownloadManager` instance on success.
    pub fn new(
        urls: Vec<Url>,
        client: Client,
        max_concurrent_part: Option<u32>,
        split_part_min_mb: u32,
        download_path: Option<PathBuf>,
        fs: F,
    ) -> Result<DownloadManager<F>> {
        let mut tasks = Vec::new();
        let current_dir = env::current_dir()?;
        let download_path: PathBuf = match env::var("CLIANT_ROOT"){
                Ok(env_var)=>{
                    PathBuf::from(env_var)
                },

                Err(_)=>{
                    if let Some(path) = download_path {
                    path
                } else {
                    current_dir
                
                }
            }
        };
        
        for url in urls {
            let filename = check_name(url.clone(), &client)?;
            let file_path=PathBuf::new().join(&download_path).join(&filename);
            if fs::exists(&file_path)?{
                // Might add dialog for downloading existing file if required.
                info!("File already exist at {:?}.",file_path);
                return Ok(DownloadManager { tasks: vec![] });
            }
            tasks.push(DownloadTask::new(
                url,
                client.clone(),
                max_concurrent_part.unwrap_or_else(|| 10),
                split_part_min_mb,
                download_path.clone(),
                fs.clone(),
                None,
                filename
            )?);
        }
        Ok(DownloadManager { tasks })
    }

    /// Starts all managed download tasks.
    ///
    /// This method iterates through all configured download tasks, sets up their
    /// progress bars, and spawns a new thread for each task to run concurrently.
    /// It then waits for all tasks to complete and clears the progress bars.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure of the download process.
    pub fn start_tasks(mut self) -> Result<()> {
        
        let multi_progress = MultiProgress::new();
        let style = indicatif::ProgressStyle::default_bar()
            .template("{msg} {spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes_per_sec} {bytes_human}/{total_bytes_human} ({eta})")
            .unwrap()
            .progress_chars("##-");

        for task in &mut self.tasks {
            let pb = multi_progress.add(ProgressBar::new(0));
            pb.set_style(style.clone());
            task.set_progress_bar(pb);
        }

        thread::scope(|s| {
            for task in self.tasks {
                //if one task fail other might still contnue mitigate these in the future.
                s.spawn(move ||->Result<()> {
                    task.start()?;
                    Ok(())
                });
            }
        });

        multi_progress.clear()?;
        Ok(())
    }
}
