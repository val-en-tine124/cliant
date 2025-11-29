use clap::{Parser, arg, command};
use std::path::PathBuf;
use url::Url;

use crate::infra::config::http_config::HttpConfig;

#[derive(Parser)]
#[command(version,about,long_about=None)]
pub struct Cliant {
    #[arg(value_parser=parse_url)]
    pub url: Vec<Url>,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(short = 'H', long)]
    pub home_dir: Option<PathBuf>,
    #[arg(short = 't', long)]
    pub timeout: Option<usize>,
    #[arg(short = 'r', long)]
    pub max_no_retries: Option<usize>,
    #[arg(short = 'd', long)]
    pub retry_delay_secs: Option<usize>,
    #[arg(short = 'u', long)]
    pub username: Option<String>,
    #[arg(short = 'p', long)]
    pub password: Option<String>,
    #[arg(short = 'm', long)]
    pub max_redirects: Option<usize>,
    #[arg(short = 'P', long)]
    pub proxy_url: Option<String>,
    #[arg(short, long)]
    pub request_headers: Option<String>,
    #[arg(short = 'c', long)]
    pub http_cookies: Option<String>,
    #[arg(short = 'c', long)]
    pub http_version: Option<String>,
}

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
