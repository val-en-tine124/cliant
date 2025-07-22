mod sync_client;
mod types;

use anyhow::Result;
use clap::Parser;
use reqwest::Url;
use sync_client::cliant::{DiskFileSystem, DownloadManager};
use types::HttpClientConfig;

#[derive(Parser)]
#[command(
    author,
    version,
    about,
    long_about = "A state of the art HTTP client for embarrasingly parallel tasks."
)]
struct Cli {
    #[arg(short = 'u', long)]
    url: String,

    #[arg(long)]
    username: Option<String>,

    #[arg(short = 'p', long)]
    password: Option<String>,

    #[arg(long)]
    max_redirects: Option<usize>,

    #[arg(short = 't', long)]
    timeout: Option<u64>,

    #[arg(long)]
    proxy_url: Option<String>,

    #[arg(short = 'H', long)]
    request_headers: Option<String>,

    #[arg(short = 'c', long)]
    http_cookies: Option<String>,

    #[arg(long)]
    http_version: Option<String>,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    let url = Url::parse(&cli.url)?;
    let client = HttpClientConfig::new(
        cli.username,
        cli.password,
        cli.max_redirects,
        cli.timeout,
        cli.proxy_url,
        cli.request_headers,
        cli.http_cookies,
        cli.http_version,
    )
    .try_into()?;

    let download_manager = DownloadManager::new(vec![url], client, 10, 5, DiskFileSystem)?;
    download_manager.start_tasks()?;

    Ok(())
}
