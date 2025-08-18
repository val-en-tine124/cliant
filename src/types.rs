use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use log::{info, warn};
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{HeaderMap, HeaderValue, ACCESS_CONTROL_REQUEST_HEADERS, COOKIE};
use reqwest::{redirect::Policy, Proxy};
use std::time::Duration;

/// Configuration for building an HTTP client.
///
/// This struct holds various options for customizing the behavior of the HTTP
/// client, such as authentication, redirects, timeouts, proxies, and headers.
pub struct HttpClientConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub max_redirects: Option<usize>,
    pub timeout: Option<u64>,
    pub proxy_url: Option<String>,
    pub request_headers: Option<String>,
    pub http_cookies: Option<String>,
    pub http_version: Option<String>,
}

impl HttpClientConfig {
    /// Creates a new `HttpClientConfig` with the given parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        username: Option<String>,
        password: Option<String>,
        max_redirects: Option<usize>,
        timeout: Option<u64>,
        proxy_url: Option<String>,
        request_headers: Option<String>,
        http_cookies: Option<String>,
        http_version: Option<String>,
    ) -> HttpClientConfig {
        HttpClientConfig {
            username,
            password,
            max_redirects,
            timeout,
            proxy_url,
            request_headers,
            http_cookies,
            http_version,
        }
    }
}

impl TryFrom<HttpClientConfig> for Client {
    type Error = anyhow::Error;

    /// Tries to convert an `HttpClientConfig` into a `reqwest::blocking::Client`.
    fn try_from(http_config: HttpClientConfig) -> Result<Self, Self::Error> {
        build_client(http_config)
    }
}

impl TryFrom<HttpClientConfig> for reqwest::Client {
    type Error = anyhow::Error;

    /// Tries to convert an `HttpClientConfig` into a `reqwest::Client`.
    fn try_from(http_config: HttpClientConfig) -> Result<Self, Self::Error> {
        build_async_client(http_config)
    }
}

macro_rules! build_client_impl {
    ($builder:ty, $client:ty) => {
        fn build_client_base(http_config: HttpClientConfig) -> Result<$client> {
            let mut client_config = <$builder>::new();
            info!("Initialized client builder.");

            let policy: Policy = if let Some(max_redirects) = http_config.max_redirects {
                warn!("Maximum redirect has been set to {}", max_redirects);
                Policy::limited(max_redirects)
            } else {
                info!("Maximum redirect still Cliant default");
                Policy::default()
            };

            let timeout = if let Some(timeout) = http_config.timeout {
                info!("Setting user-defined timeout {}.", timeout);
                Duration::new(timeout, 0)
            } else {
                info!("No user-defined timeout, setting timeout to default {}", 60);
                Duration::new(120, 0)
            };

            client_config = client_config.timeout(timeout).redirect(policy);

            if let Some(proxy_url) = http_config.proxy_url {
                info!("Setting up user-defined proxy for Cliant");
                client_config = client_config.proxy(
                    Proxy::all(proxy_url).context("failed to proxy all traffic to the passed proxy url")?,
                );
            } else {
                info!("No user defined proxy.");
                client_config = client_config.no_proxy();
            }

            if let Some(http_version) = http_config.http_version {
                client_config = match http_version.as_str() {
                    "1.1" => {
                        info!("Still HTTP version 1.1.");
                        Ok(client_config.http1_only())
                    }
                    "2" => {
                        info!("Switching HTTP version to version 2.");
                        Ok(client_config.http2_prior_knowledge())
                    }
                    _ => Err(anyhow!("Unsupported http version")),
                }?
            }

            let mut request_header_headermap = HeaderMap::new();

            if let Some(request_header_str) = http_config.request_headers {
                info!("Setting up user defined HTTP header.");
                let request_headervalue = HeaderValue::from_str(&request_header_str)?;
                request_header_headermap.insert(ACCESS_CONTROL_REQUEST_HEADERS, request_headervalue);
            }

            if let Some(cookies_str) = http_config.http_cookies {
                info!("Got user-defined cookies.");
                let cookie_value = HeaderValue::from_str(&cookies_str)?;
                request_header_headermap.insert(COOKIE, cookie_value);
            }

            let client = client_config
                .default_headers(request_header_headermap)
                .build()
                .context("Can't build http client from http configuration.".yellow())?;
            info!("Built HTTP client with User configuration");

            Ok(client)
        }
    };
}

fn build_client(http_config: HttpClientConfig) -> Result<Client> {
    build_client_impl!(ClientBuilder, Client);
    build_client_base(http_config)
}

fn build_async_client(http_config: HttpClientConfig) -> Result<reqwest::Client> {
    build_client_impl!(reqwest::ClientBuilder, reqwest::Client);
    build_client_base(http_config)
}

#[test]
fn test_config_to_client() -> Result<()> {
    let config = HttpClientConfig::new(None, None, None, None, None, None, None, None);
    let client = Client::try_from(config)?;
    client.get("https://www.google.com").send()?;
    Ok(())
}
