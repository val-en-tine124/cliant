use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::Url;
use std::collections::BTreeSet;
use std::env;
use std::path:: PathBuf;

use super::{FileSystemIO,check_name::check_name};
use super::download_task::DownloadTask;

pub struct DownloadManager<F: FileSystemIO> {
    tasks: BTreeSet<DownloadTask<F>>,
}

impl<F: FileSystemIO + Clone + Send + Sync + 'static> DownloadManager<F> {
    pub fn new(
        urls: Vec<Url>,
        client: Client,
        max_concurrent_part: u32,
        split_part_min_mb: u32,
        fs: F,
    ) -> Result<DownloadManager<F>> {
        let mut tasks = BTreeSet::new();
        for url in urls {
            let filename = check_name(url.clone(), &client)?;
            let cliant_root = env::var("CLIANT_ROOT").unwrap_or(".".to_string());
            let download_path = PathBuf::from(cliant_root).join(&filename);
            let _ = tasks.insert(DownloadTask::new(
                url,
                client.clone(),
                max_concurrent_part,
                split_part_min_mb,
                download_path,
                fs.clone(),
            )?);
        }
        Ok(DownloadManager { tasks: tasks })
    }

    pub fn start_tasks(&self) -> Result<()> {
        for task in self.tasks.iter().rev() {
            task.start()?;
        }
        Ok(())
    }
}
