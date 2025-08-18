// use crate::errors::CliantError;
// use crate::sync::DiskFileSystem;
// use anyhow::Result;
// use reqwest::{Client, Url};
// use std::path::PathBuf;

// pub struct DownloadManager {
//     urls: Vec<Url>,
//     client: Client,
//     max_concurrent_parts: Option<u32>,
//     max_retries: u32,
//     download_path: PathBuf,
//     file_system: DiskFileSystem,
// }

// impl DownloadManager {
//     pub fn new(
//         urls: Vec<Url>,
//         client: Client,
//         max_concurrent_parts: Option<u32>,
//         max_retries: u32,
//         download_path: Option<PathBuf>,
//         file_system: DiskFileSystem,
//     ) -> Result<Self> {
//         let download_path = download_path.unwrap_or(std::env::current_dir()?);
//         Ok(Self {
//             urls,
//             client,
//             max_concurrent_parts,
//             max_retries,
//             download_path,
//             file_system,
//         })
//     }

//     pub async fn start_tasks(&self) -> Result<(), CliantError> {
//         let tasks = self.urls.iter().map(|_url| {
//             let _client = self.client.clone();
//             let _download_path = self.download_path.clone();
//             let _file_system = self.file_system.clone();
//             tokio::spawn(async move {
//                 // TODO: Implement the download logic for a single URL
//             })
//         });

//         futures::future::join_all(tasks).await;

//         Ok(())
//     }
// }
