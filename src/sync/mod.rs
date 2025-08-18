//! # Sync Module
//!
//! This module contains the core synchronization and download logic for `cliant-rs`.
//! It defines traits for file system operations, data structures for managing download
//! parts, and helper functions for network requests.

pub mod download_manager;
mod download_task;
mod download_part;
mod base_file_part;
mod check_name;

use std::path::{Path,PathBuf};
use std::fs::{self,File};
use anyhow::{Result,Context};
use colored::Colorize;
use log::{error, info};
use reqwest::blocking::Response;
use std::time::Duration;



/// Represents the status of a download part.
#[derive(Clone)]
enum PartStatus {
    /// The download part is starting.
    Starting,
    /// The download part has started.
    Started,
    /// The download part has completed, with associated file metadata.
    Completed(FileMetaData),
    /// The download part is broken, with the number of broken bytes.
    Broken(BrokenFilePart),
}

/// Metadata about a downloaded file part.
#[derive(Clone)]
struct FileMetaData {
    /// The path to the file part.
    path: PathBuf,
    /// The number of bytes completed for this part.
    completed_bytes: usize,
}

/// Represents a broken file part, indicating the number of bytes that were
/// successfully downloaded before the breakage.
#[derive(Clone)]
struct BrokenFilePart(u64);

/// A concrete implementation of `FileSystemIO` for interacting with the disk.
#[derive(Clone)]
pub struct DiskFileSystem;

impl FileSystemIO for DiskFileSystem {
    fn create_dir_all(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path).context("Failed to create directory")
    }

    fn create_file(&self, path: &Path) -> Result<File> {
        File::create(path).context("Failed to create file")
    }

    fn open_file(&self, path: &Path) -> Result<File> {
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .context(format!("Can't open path:{:?}", path).yellow())
    }

    fn open_file_for_read(&self, path: &Path) -> Result<File> {
        File::open(path).context("Failed to open file for reading")
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        fs::remove_file(path).context("Failed to remove file")
    }

    fn metadata(&self, path: &Path) -> Result<fs::Metadata> {
        fs::metadata(path).context("Failed to get metadata")
    }

    fn remove_dir_all(&self, path: &Path) -> Result<()> {
        fs::remove_dir_all(path).context("Failed to remove directory")
    }
}


/// A trait defining common file system operations.
///
/// This trait allows for abstracting file system interactions, making it easier
/// to test and potentially swap out different file system implementations.
pub trait FileSystemIO {
    /// Creates a directory and all its parent directories if they do not exist.
    fn create_dir_all(&self, path: &Path) -> Result<()>;
    /// Creates a new file at the specified path.
    fn create_file(&self, path: &Path) -> Result<File>;
    /// Opens a file at the specified path for writing, creating it if it doesn't exist,
    /// and truncating it if it does.
    fn open_file(&self, path: &Path) -> Result<File>;
    /// Opens a file at the specified path for reading.
    fn open_file_for_read(&self, path: &Path) -> Result<File>;
    /// Removes a file at the specified path.
    fn remove_file(&self, path: &Path) -> Result<()>;
    /// Retrieves metadata for a file or directory at the specified path.
    fn metadata(&self, path: &Path) -> Result<fs::Metadata>;
    /// Recursively removes a directory and all its contents.
    fn remove_dir_all(&self, path: &Path) -> Result<()>;
}

/// Retries a given function that performs a network request a specified number of times.
///
/// This function is useful for handling transient network errors by retrying the request
/// after a short delay. It specifically retries on connection and timeout errors.
///
/// # Arguments
///
/// * `max_retry_no` - The maximum number of times to retry the request.
/// * `function` - A closure that performs the network request and returns a `Result<Response, reqwest::Error>`.
///
/// # Returns
///
/// A `Result` containing the `reqwest::blocking::Response` on success, or an `anyhow::Error`
/// if all retries fail or a non-retryable error occurs.
pub fn retry_request<F>(max_retry_no: u8, function: F) -> Result<Response, anyhow::Error>
where
    F: Fn() -> Result<Response, reqwest::Error>,
{
    for current_retry in 1..=max_retry_no {
        match function() {
            Ok(response) => {
                return Ok(response);
            }
            Err(error) if error.is_connect() || error.is_timeout() => {
                if let Some(err_url) = error.url() {
                    let url = err_url.clone();
                    info!("Can't get http response body for url {url}");
                }
                error!("Network error, retrying HTTP request {current_retry}...");
                std::thread::sleep(Duration::from_millis(10000));
                continue;
            }
            Err(error) => {
                println!("Error: {:?}", error);
                if let Some(err_url) = error.url() {
                    let url = err_url.clone();
                    info!("Can't get http response body for url {url}");
                }

                return Err(error.into());
            }
        }
    }
    anyhow::bail!(format!("Spurious network operation timeout.").yellow());
}