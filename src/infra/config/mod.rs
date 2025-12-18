use anyhow::{Error as AnyhowError, Result};
use cookie::Cookie;
use derive_getters::Getters;
use derive_setters::Setters;
use dirs::cache_dir;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
use reqwest::{Proxy, redirect::Policy};
use serde::Deserialize;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Debug, Getters, Clone, Copy)]
pub struct RetryConfig {
    max_no_retries: usize,
    retry_delay_secs: usize,
}

impl RetryConfig {
    pub fn new(max_no_retries: usize, retry_delay_secs: usize) -> Self {
        Self { max_no_retries, retry_delay_secs }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self { max_no_retries: 10, retry_delay_secs: 10 }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct HttpConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub max_redirects: Option<usize>,
    pub timeout: usize,
    pub proxy_url: Option<String>,
    pub request_headers: Option<String>,
    pub http_cookies: Option<String>,
    pub http_version: Option<String>,
    pub multipart_part_size: Option<usize>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            username: None,
            password: None,
            max_redirects: None,
            timeout: 60,
            proxy_url: None,
            request_headers: None,
            http_cookies: None,
            http_version: None,
            multipart_part_size: Some(256 * 1024),
        }
    }
}

impl TryFrom<HttpConfig> for Client {
    type Error = AnyhowError;

    /// Tries to convert an `HttpConfig` into a `reqwest::blocking::Client`.
    fn try_from(http_config: HttpConfig) -> Result<Self, Self::Error> {
        build_client(http_config)
    }
}

impl TryFrom<HttpConfig> for reqwest::Client {
    type Error = AnyhowError;

    /// Tries to convert an `HttpConfig` into a `reqwest::Client`.
    fn try_from(http_config: HttpConfig) -> Result<Self, Self::Error> {
        build_async_client(http_config)
    }
}

macro_rules! build_client_impl {
    ($builder:ty, $client:ty) => {
        fn build_client_base(http_config: HttpConfig) -> Result<$client, AnyhowError> {
            let mut client_config = <$builder>::new();
            info!("Initialized client builder.");

            let policy: Policy = if let Some(max_redirects) = http_config.max_redirects {
                info!("Maximum redirect has been set to {}", max_redirects);
                Policy::limited(max_redirects)
            } else {
                info!("Maximum redirect still Cliant default");
                Policy::default()
            };

            let timeout = {
                info!("Setting timeout to {}.", http_config.timeout);
                Duration::from_secs(http_config.timeout as u64)
            };

            client_config = client_config.timeout(timeout).redirect(policy);

            if let Some(proxy_url) = http_config.proxy_url {
                info!("Setting up user-defined proxy for Cliant");
                client_config = client_config.proxy(Proxy::all(proxy_url)?);
            } else {
                info!("No user defined proxy.");
                client_config = client_config.no_proxy();
            }

            if let Some(http_version) = http_config.http_version {
                client_config = match http_version.as_str() {
                    "1.1" => {
                        info!("Still HTTP version 1.1.");
                        client_config.http1_only()
                    }

                    _ => {
                        warn!("Unsupported http version, using default http version 1.1.");
                        client_config.http1_only()
                    }
                }
            }

            let mut request_header_headermap = HeaderMap::new();
            // comma seperated header value e.g name:johndoe,age:23
            if let Some(request_headers_str) = http_config.request_headers {
                info!("Setting up user-defined HTTP headers.");
                for header in request_headers_str.split(',').map(|s| s.trim()) {
                    let parts: Vec<&str> = header.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let name = parts[0].trim();
                        let value = parts[1].trim();
                        request_header_headermap.insert(
                            reqwest::header::HeaderName::from_str(name)?,
                            HeaderValue::from_str(value)?,
                        );
                    }
                }
            }

            if let Some(cookies_str) = http_config.http_cookies {
                info!("Setting up user-defined HTTP cookies.");
                match Cookie::parse(cookies_str) {
                    Ok(cookie) => {
                        request_header_headermap
                            .insert(COOKIE, HeaderValue::from_str(cookie.to_string().as_ref())?);
                    }
                    Err(err) => {
                        error!(error = %err, "Can't sanitize cookie");
                    }
                }
            }

            let client = client_config
                .default_headers(request_header_headermap)
                .build()?;
            info!("Built HTTP client with User configuration");

            Ok(client)
        }
    };
}

fn build_client(http_config: HttpConfig) -> Result<Client, AnyhowError> {
    build_client_impl!(ClientBuilder, Client);
    build_client_base(http_config)
}

fn build_async_client(
    http_config: HttpConfig,
) -> Result<reqwest::Client, AnyhowError> {
    build_client_impl!(reqwest::ClientBuilder, reqwest::Client);
    build_client_base(http_config)
}

#[derive(Clone, Getters)]
pub struct CliantDirConfig {
    pub cache_dir: Option<PathBuf>,
}
impl CliantDirConfig {
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        Self { cache_dir }
    }
}

impl Default for CliantDirConfig {
    fn default() -> Self {
        Self { cache_dir: default_cliant_cache_dir() }
    }
}

fn default_cliant_cache_dir() -> Option<PathBuf> {
    if let Some(home) = cache_dir() {
        Some(home.join(".cliant"))
    } else {
        error!("Can't get user cache directory.");
        std::process::exit(1);
    }
}
