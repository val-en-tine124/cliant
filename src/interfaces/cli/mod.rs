use clap::{Parser, arg, command};
use std::path::PathBuf;
use url::Url;

use crate::infra::config::HttpConfig;

#[derive(Parser)]
#[command(version,about,long_about=None)]
///A state-of-the-art HTTP client for embarrassingly parallel tasks.
pub struct Cliant {

    #[arg(value_parser=parse_url)]
    ///This is the url of the download.
    pub url: Vec<Url>,
    ///This is the output file for the download.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(short = 'H', long)]
    ///This is the home directory for all downloads.
    pub home_dir: Option<PathBuf>,
    #[arg(short = 't', long,default_value_t=60)]
    /// Set http timeout(in secs) for all http request.
    pub timeout: usize,
    ///Set the maximum no of retry for each http request.
    #[arg(short = 'r', long,default_value_t=10)]
    pub max_no_retries: usize,
    /// Set the interval between each retry delay(in secs).
    #[arg(short = 'd', long,default_value_t=10)]
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
}

///This method takes a url as a string literal,checks and validate http 
/// scheme in the url,parses it and return a Result Url or String 
/// type if any error occur.
fn parse_url(url: &str) -> Result<Url, String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        let parsed_url = Url::parse(url).map_err(|e| format!("Invalid Url {url} {e}"));
        return parsed_url;
    }
    let new_url = format!("https://{url}");
    let parsed_url = Url::parse(&new_url).map_err(|e| format!("Invalid Url {url} {e}"));
    parsed_url
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
        }
    }
}
