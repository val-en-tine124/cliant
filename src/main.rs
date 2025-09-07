//! # Cliant-rs
//!
//! A state-of-the-art HTTP client for embarrassingly parallel tasks.
//!
//! This module contains the main entry point for the `cliant-rs` application. It
//! parses command-line arguments, configures the HTTP client, and starts the
//! download process.

pub mod errors;
mod interfaces;
mod domain;
mod application;
mod infrastructure;

// use clap::Parser;
// use errors::CliantError;
// use reqwest::Url;
// use std::path::PathBuf;
// use std::process::exit;


// #[derive(Parser)]
// #[command(author, version, about, long_about = None)]
// struct Cli {
//     #[arg(short, long)]
//     url: Url,
//     #[arg(short, long)]
//     ///Path to save the file.
//     path: Option<PathBuf>,
//     #[arg(short, long, default_value_t = false)]
//     verbose: bool,
//     #[arg(short = 'U', long)]
//     username: Option<String>,
//     #[arg(short = 'P', long)]
//     password: Option<String>,
//     #[arg(long)]
//     max_redirects: Option<usize>,
//     #[arg(short = 't', long)]
//     timeout: Option<u64>,
//     #[arg(long)]
//     proxy_url: Option<String>,

//     /// A semi-column seperated key value pair e.g key1=value1;key2=value2.
//     #[arg(short = 'H', long)]
//     request_headers: Option<String>,
    
//     #[arg(short = 'c', long)]
//     http_cookies: Option<String>,
//     #[arg(long)]
//     http_version: Option<String>,
//     #[arg(short = 'M', long)]
//     max_concurrent_part: Option<u32>,
// }

// fn main() -> Result<(), CliantError> {
//     let cli = Cli::parse();

//     if cli.verbose {
//         //std::env::set_var("RUST_LOG", "debug");
//         env_logger::init();
//     }

//     ctrlc::set_handler(move || {
//         println!("Received Ctrl+C! Gracefully shutting down...");
//         exit(CliantError::UserCancelled.exit_code());
//     })
//     .expect("Error setting Ctrl-C handler");

//     //let url = Url::parse(&cli.url)?;
//     //remember to improve cli to accept multiple arguments.
    
//     sync_start(cli.url.clone(), cli)?;

//     Ok(())
// }

// fn sync_start(url: Url, cli: Cli) -> Result<(), CliantError> {
//     let client: reqwest::blocking::Client = HttpClientConfig::new(
//         cli.username,
//         cli.password,
//         cli.max_redirects,
//         cli.timeout,
//         cli.proxy_url,
//         cli.request_headers,
//         cli.http_cookies,
//         cli.http_version,
//     )
//     .try_into()?;
//     let download_manager = sync::download_manager::DownloadManager::new(
//         vec![url],
//         client,
//         cli.max_concurrent_part,
//         3,
//         cli.path,
//         sync::DiskFileSystem,
//     )?;
//     download_manager.start_tasks()?;
//     Ok(())
// }
fn main(){
    //
}

