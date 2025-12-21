use crate::application::services::downloader_service::HttpCliService;
use crate::infra::config::{HttpConfig, RetryConfig};
use anyhow::Result;
use clap::{ArgAction, Parser, arg, command};
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use url::Url;

#[derive(Clone,Parser)]
#[command(version,about,long_about=None)]
///A state-of-the-art HTTP client for embarrassingly parallel tasks.
pub struct Cliant {
    #[arg(value_parser=parse_url)]
    ///This is the url of the download.
    pub url: Vec<Url>,
    ///This is the output file for the download.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(short = 't', long, default_value_t = 60)]
    /// Set http timeout(in secs) for all http request.
    pub timeout: usize,
    ///Set the maximum no of retry for each http request.
    #[arg(short = 'r', long, default_value_t = 10)]
    pub max_no_retries: usize,
    /// Set the interval between each retry delay(in secs).
    #[arg(short = 'd', long, default_value_t = 10)]
    pub retry_delay_secs: usize,
    /// Set http basic auth password to site.
    #[arg(short = 'u', long)]
    pub username: Option<String>,
    /// Set http basic auth password to site.
    #[arg(short = 'p', long)]
    pub password: Option<String>,
    #[arg(short = 'm', long)]
    pub max_redirects: Option<usize>,
    ///Only http proxies are supported currently.
    #[arg(short = 'P', long)]
    pub proxy_url: Option<String>,
    /// A semi-column seperated key value pair e.g key1=value1;key2=value2.
    #[arg(long)]
    pub request_headers: Option<String>,
    /// Add http cookies from previous session.
    #[arg(short = 'c', long)]
    pub http_cookies: Option<String>,
    /// Set the http version for current requests.
    #[arg(long)]
    pub http_version: Option<String>,
    // Set the size of the chunk for multipart download.
    #[arg(short = 'C', long = "chunk_size")]
    pub multipart_part_size: Option<usize>,
    /// Set the Logging level to quiet less information about download events are emitted i.e only Errors.
    #[arg(short = 'q', long = "quiet", default_value_t = true)]
    pub quiet: bool,
    /// Set the Logging level to verbose more information about download events are emitted.
    #[arg(short='v',long="verbose",action=ArgAction::Count)]
    pub verbose: u8,
}

///This method takes a url as a string literal,checks and validate http
/// scheme in the url,parses it and return a Result Url or String
/// type if any error occur.
fn parse_url(url: &str) -> Result<Url, String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        let parsed_url =
            Url::parse(url).map_err(|e| format!("Invalid Url {url} {e}"));
        return parsed_url;
    }
    let new_url = format!("https://{url}");
    Url::parse(&new_url).map_err(|e| format!("Invalid Url {url} {e}"))
}

impl From<Cliant> for HttpConfig {
    fn from(value: Cliant) -> Self {
        HttpConfig {
            username: value.username,
            password: value.password,
            max_redirects: value.max_redirects,
            timeout: value.timeout,
            proxy_url: value.proxy_url,
            request_headers: value.request_headers,
            http_cookies: value.http_cookies,
            http_version: value.http_version,
            multipart_part_size: value.multipart_part_size,
        }
    }
}

impl From<Cliant> for RetryConfig {
    fn from(value: Cliant) -> Self {
        RetryConfig {
            max_no_retries: value.max_no_retries,
            retry_delay_secs: value.retry_delay_secs,
        }
    }
}

fn setup_tracing(args: &Cliant) {
    // Start with a base filter
    let mut filter = EnvFilter::builder()
        .with_default_directive(Level::WARN.into()) // default = warn
        .from_env_lossy(); // respects RUST_LOG if user set it

    // Override with -q / -v flags (unless user explicitly set RUST_LOG)
    if std::env::var("RUST_LOG").is_err() {
        if args.quiet {
            filter = filter.add_directive(Level::ERROR.into());
        } else {
            match args.verbose {
                0 => filter = filter.add_directive(Level::WARN.into()),
                1 => filter = filter.add_directive(Level::INFO.into()),
                2 => filter = filter.add_directive(Level::DEBUG.into()),
                _ => filter = filter.add_directive(Level::TRACE.into()),
            }
        }
    }

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_ansi(true) // colors in terminal
                .with_target(false) // cleaner output
                .with_file(false)
                .with_line_number(false)
                .compact(),
        ) // one-line format, perfect for CLIs
        .init();
}

pub async fn set_up_cli_app() -> Result<()> {
    let cliant = Cliant::parse();
    let output_file = cliant.output.clone();
    setup_tracing(&cliant); //Setup logging and tracing
    let urls= cliant.url.clone();
    let http_config = HttpConfig::from(cliant.clone());
    let retry_config = RetryConfig::from(cliant.clone());
    HttpCliService::new(urls, output_file, http_config, retry_config)
        .start_download()
        .await?;
    Ok(())
}
