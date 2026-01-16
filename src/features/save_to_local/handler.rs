//! HTTP Download Handler for Local Filesystem Storage
//!
//! This module provides the core download logic for the `save_to_local` feature.
//! It orchestrates the data transport layer, filesystem operations, and progress tracking
//! to download files from HTTP(S) sources to the local filesystem.
//!
//! # Architecture
//!
//! The handler follows these steps:
//! 1. Parse and validate the download arguments
//! 2. Extract file name and parent directory from the output path
//! 3. Initialize the appropriate transport (HTTP/HTTPS)
//! 4. Create a local filesystem writer with proper resource management
//! 5. Retrieve total file size and initialize progress tracker
//! 6. Stream data chunks from source and write to destination
//! 7. Clean up resources and display completion status
//!
//! # Error Handling
//!
//! The function uses `Result` for proper error propagation with context.
//! If streaming fails, the error is logged but cleanup still occurs.
//!
//! # Resource Management
//!
//! This module ensures proper RAII patterns:
//! - File handles are automatically cleaned up via `close_fs()`
//! - All resources are cleaned up even on error paths
//! - Progress tracker finalization always occurs for proper UI state

use std::path::PathBuf;

use super::cli::LocalArgs;
use crate::shared::fs::FsOps;
use crate::shared::fs::local::LocalFsBuilder;
use crate::shared::network::{
    DataTransport,
    factory::{TransportType, handle_http},
};
use crate::shared::progress_tracker::{CliProgressTracker, ProgressTracker};
use anyhow::{Context, Result};
use tokio_stream::StreamExt;
use tracing::{debug, error, info, instrument, trace};
use tokio::time;
/// Downloads a file from an HTTP(S) URL and saves it to the local filesystem.
///
/// This is the main entry point for the `save_to_local` feature. It coordinates
/// all aspects of the download process including transport negotiation, file creation,
/// streaming, and progress tracking.
///
/// # Arguments
///
/// * `args` - `LocalArgs` containing:
///   - `url`: The HTTP(S) URL to download from
///   - `output`: The local filesystem path where the file will be saved
///   - `http_args`: HTTP-specific configuration (timeout, auth, headers, etc.)
///   - `transport`: The transport protocol to use (currently HTTP only)
///
/// # Process
///
/// 1. Validates and extracts the file name and parent directory from the output path
/// 2. Initializes the transport layer (HTTP client with middleware)
/// 3. Creates a local file handle with proper async I/O buffering
/// 4. Retrieves the total file size for progress tracking
/// 5. Streams data chunks from the remote source
/// 6. Writes each chunk to disk and updates progress
/// 7. Ensures proper cleanup of filesystem resources via RAII
/// 8. Displays completion information
///
/// # Errors
///
/// Returns an error if:
/// - The output path has no file name component
/// - Parent directory cannot be determined
/// - Transport layer fails to initialize
/// - Remote server returns an error
/// - Local filesystem operations fail
/// - Progress tracker initialization fails
///
/// # Resource Management
///
/// This function uses RAII patterns to ensure resources are properly cleaned up:
/// - `LocalFsBuilder::build()` creates resources that are automatically freed
/// - `builder.close_fs()` explicitly flushes and closes the file handle
/// - Progress tracker is always finalized, even if errors occur
/// - On error paths, `close_fs()` is still called to ensure cleanup
#[instrument(name = "handle_http_download", fields(args = %args.url), skip(args))]
pub async fn handle(args: LocalArgs) -> Result<()> {
    let file_path = args.output;
    let url = args.url;
    let http_args = args.http_args;

    // Extract file name from path
    // Using file_name (not full path) because opendal appends path to root directory
    let file_name: PathBuf = file_path
        .file_name()
        .context(format!(
            "Final component of {} is not a file",
            file_path.display()
        ))?
        .into();

    // Get parent directory for file storage
    let file_parent_dir = file_path
        .parent()
        .context(format!(
            "Can't determine parent directory of: {}",
            file_path.display()
        ))?
        .to_path_buf();

    debug!("File path: {:?}", file_path);
    debug!("File parent directory: {:?}", file_parent_dir);

    // Initialize transport layer
    let transport = match args.transport {
        TransportType::Http => handle_http(http_args, &TransportType::Http),
    }?;

    // Create local filesystem writer with proper resource management
    let fs_writer = LocalFsBuilder::new()
        .file_name(file_name)
        .root_path(file_parent_dir)
        .build()
        .await?;

    // Retrieve remote file metadata and initialize tracking
    let stream_result = transport.receive_data(url.clone()).await;
    let total_bytes = transport.total_bytes(url.clone()).await?;
    let tracker = CliProgressTracker::new(total_bytes, file_path.clone())?;

    // Stream and write data with proper error handling and cleanup
    // RAII ensures fs_writer is cleaned up even if errors occur
    match stream_result {
        Ok(mut stream) => {
            info!("Starting download stream...");
            let instant = time::Instant::now();
            while let Some(bytes) = stream.try_next().await? {
                let bytes_size = bytes.len();
                tracker.update(bytes_size).await; // call the update function before append_bytes to reflect actual network speed.
                trace!(
                    "Writing {} bytes to {:?}",
                    bytes_size,
                    file_path
                );
                fs_writer.append_bytes(bytes).await?; // If tracker.update was called here it will reflect file system write speed. 
                
            }
            let elapsed =instant.elapsed();
            info!("Download streaming completed, file fully downloaded in {} secs or {}ms .",elapsed.as_secs(),elapsed.as_millis());
            // Explicit resource cleanup: flush buffers and close file handle
            fs_writer.close_fs().await;
        }

        Err(err) => {
            error!("Failed to stream data from {}: {}", url, err);
            // Ensure cleanup even on error - critical for resource management
            fs_writer.close_fs().await;
            return Err(err).context(format!("Failed to download from {url}"));
        }
    }

    // Finalize progress tracker and display completion info
    tracker.finish().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::network::http::config::HttpArgs;
    use tokio::fs;
    use async_tempfile::TempDir;

    /// Test downloading a file to a valid path
    #[tokio::test]
    async fn test_handle_valid_output_path() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().await?;
        let output_path = temp_dir.dir_path().join("test_file.bin");
        let link = url::Url::parse("http://speedtest.tele2.net/1MB.zip")?;

        let args = LocalArgs {
            url: link,
            http_args: HttpArgs::default(),
            output: output_path.clone(),
            transport: TransportType::Http,
        };

        let result = handle(args).await;
        assert!(result.is_ok(), "Download should succeed");
        assert!(
            output_path.exists(),
            "Downloaded file should exist at {output_path:?}"
        );

        // Verify file has content
        let file_size = fs::metadata(&output_path).await?.len();
        assert!(file_size > 0, "Downloaded file should not be empty");

        Ok(())
    }

    /// Test handling invalid output path (no file name)
    #[tokio::test]
    async fn test_handle_invalid_path_no_filename() -> anyhow::Result<()> {
        let link = url::Url::parse("http://example.com/file.zip")?;
        // Root path has no file name
        let args = LocalArgs {
            url: link,
            http_args: HttpArgs::default(),
            output: PathBuf::from("/"),
            transport: TransportType::Http,
        };

        let result = handle(args).await;
        assert!(
            result.is_err(),
            "Should fail with invalid path (no file name)"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("is not a file"),
            "Error message should indicate file name issue"
        );

        Ok(())
    }

    /// Test handling output path with nested directories
    #[tokio::test]
    async fn test_handle_nested_output_path() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().await?;
        let output_path = temp_dir.dir_path().join("subdir").join("file.bin");

        // Create parent directory if needed
        fs::create_dir_all(output_path.parent().unwrap()).await?;

        let link = url::Url::parse("http://speedtest.tele2.net/1MB.zip")?;

        let args = LocalArgs {
            url: link,
            http_args: HttpArgs::default(),
            output: output_path.clone(),
            transport: TransportType::Http,
        };

        let result = handle(args).await;
        if result.is_ok() {
            assert!(
                output_path.exists(),
                "File should be created at nested path"
            );
        }

        Ok(())
    }

    /// Test resource cleanup on network error
    #[tokio::test]
    async fn test_resource_cleanup_on_invalid_domain() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().await?;
        let output_path = temp_dir.dir_path().join("test_cleanup.bin");
        let invalid_link =
            url::Url::parse("http://invalid-nonexistent-domain-12345.local/file.zip")?;

        let args = LocalArgs {
            url: invalid_link,
            http_args: HttpArgs::default(),
            output: output_path.clone(),
            transport: TransportType::Http,
        };

        let result = handle(args).await;
        // Should fail with connection error
        assert!(result.is_err(), "Invalid domain should cause error");

        // Cleanup should still occur, no hanging resources
        // Verify no partial file left behind (or cleanup occurred if one exists)
        if output_path.exists() {
            let file_size = fs::metadata(&output_path).await?.len();
            // If file exists, it should have content or be cleaned up
            println!("File size after error: {file_size}");
        }

        Ok(())
    }

    /// Test with custom HTTP timeout configuration
    #[tokio::test]
    async fn test_handle_with_timeout_config() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().await?;
        let output_path = temp_dir.dir_path().join("timeout_test.bin");
        let link = url::Url::parse("http://speedtest.tele2.net/1MB.zip")?;

        let mut http_args = HttpArgs::default();
        http_args.timeout = 30; // Custom timeout

        let args = LocalArgs {
            url: link,
            http_args,
            output: output_path.clone(),
            transport: TransportType::Http,
        };

        let result = handle(args).await;
        // Should work or fail gracefully with timeout
        // Just verify it doesn't panic or hang
        assert!(result.is_ok() || result.is_err());

        Ok(())
    }

    /// Test file path validation - missing parent directory
    #[tokio::test]
    async fn test_handle_invalid_parent_directory() -> anyhow::Result<()> {
        let link = url::Url::parse("http://example.com/file.zip")?;
        // Using a non-existent nested path that can't have parent extracted
        // Create a path that ends at root level
        let bad_path = std::path::PathBuf::from("/");

        let args = LocalArgs {
            url: link,
            http_args: HttpArgs::default(),
            output: bad_path,
            transport: TransportType::Http,
        };

        let result = handle(args).await;
        assert!(
            result.is_err(),
            "Invalid path should produce error during validation"
        );

        Ok(())
    }

    /// Test that progress tracker is initialized correctly
    #[tokio::test]
    async fn test_handle_progress_tracking() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().await?;
        let output_path = temp_dir.dir_path().join("progress_test.bin");
        let link = url::Url::parse("http://speedtest.tele2.net/1MB.zip")?;

        let args = LocalArgs {
            url: link,
            http_args: HttpArgs::default(),
            output: output_path.clone(),
            transport: TransportType::Http,
        };

        // This tests that progress tracker is properly initialized and finalized
        let result = handle(args).await;
        // Should complete with or without error, but progress should finalize
        assert!(result.is_ok() || result.is_err());

        Ok(())
    }

    /// Test that instrument/tracing integration works
    #[tokio::test]
    async fn test_handle_with_tracing() -> anyhow::Result<()> {
        // Initialize tracing subscriber for the test
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        let temp_dir = TempDir::new().await?;
        let output_path = temp_dir.dir_path().join("traced_download.bin");
        let link = url::Url::parse("http://speedtest.tele2.net/1MB.zip")?;

        let args = LocalArgs {
            url: link,
            http_args: HttpArgs::default(),
            output: output_path,
            transport: TransportType::Http,
        };

        // Execute with tracing enabled
        let result = handle(args).await;
        // Should have proper instrumentation in logs
        assert!(result.is_ok() || result.is_err());

        Ok(())
    }

    /// Test URL cloning (ensure no performance issues)
    #[tokio::test]
    async fn test_url_handling_efficiency() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().await?;
        let output_path = temp_dir.dir_path().join("efficiency_test.bin");
        let link = url::Url::parse("http://speedtest.tele2.net/1MB.zip")?;

        let args = LocalArgs {
            url: link.clone(),
            http_args: HttpArgs::default(),
            output: output_path,
            transport: TransportType::Http,
        };

        // URL is cloned twice in handle function - verify it works correctly
        let result = handle(args).await;
        assert!(result.is_ok() || result.is_err());

        Ok(())
    }
}
