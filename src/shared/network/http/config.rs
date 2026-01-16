use anyhow::{Error as AnyhowError, Result};
use cookie::Cookie;
use derive_getters::Getters;
use reqwest::{Client, ClientBuilder};
use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
use reqwest::{Proxy, redirect::Policy};
use secrecy::SecretString;
use std::str::FromStr;
use std::time::Duration;
use tracing::{error, info, warn};
use clap::{command,Args,arg};
#[derive(Debug,Args, Getters, Clone, Copy)]
pub struct RetryArgs {
    ///This is the maximum number of http request 
    /// retries that will be made to server incase a network issue occur.
    #[arg(short='r',long,default_value_t=10,)]
    pub max_no_retries: usize,
    ///This is the delay in seconds that will be made between each retry request, 
    /// NB: this application leverage exponential backoff with a fixed exponent for every retry. 
    #[arg(short='d',long,default_value_t=10,)]
    pub retry_delay_secs: usize,
}

impl RetryArgs {
    pub fn new(max_no_retries: usize, retry_delay_secs: usize) -> Self {
        Self { max_no_retries, retry_delay_secs }
    }
}

impl Default for RetryArgs {
    fn default() -> Self {
        Self { max_no_retries: 10, retry_delay_secs: 10 }
    }
}

#[derive(Args,Debug, Clone)]
pub struct HttpArgs {
    #[command(flatten)]
    pub retry_args:RetryArgs,
    /// Set http basic authentication username used for login to the site.
    #[arg(short='U',long,env="CLIANT_HTTP_USERNAME")]
    pub username: Option<String>,
    /// Set http basic authentication password to used for login to the site.
    #[arg(short='P',long,env="CLIANT_HTTP_PASSWORD")]
    pub password: Option<SecretString>,
    ///Maximum http redirects this application will make if need be.
    #[arg(long)]
    pub max_redirects: Option<usize>,
    /// Set http timeout(in secs) for all http request.
    #[arg(short='T',long,default_value_t=60)]
    pub timeout: usize,
    ///Only http proxies are supported currently.
    #[arg(short='p',long)]
    pub proxy_url: Option<String>,
    /// Use a column seperated key value pair e.g key1:value1,key2:value2 for request headers.
    #[arg(long)]
    pub request_headers: Option<String>,
    /// Add http cookies from previous http session.
    #[arg(long)]
    pub http_cookies: Option<String>,
    /// Set http version,supports up to  http version 1.1.
    #[arg(long)]
    pub http_version: Option<String>,
}

impl Default for HttpArgs {
    fn default() -> Self {
        Self {
            retry_args:RetryArgs::default(),
            username: None,
            password: None,
            max_redirects: None,
            timeout: 60,
            proxy_url: None,
            request_headers: None,
            http_cookies: None,
            http_version: None,
        }
    }
}

impl TryFrom<HttpArgs> for reqwest::Client {
    type Error = AnyhowError;

    /// Tries to convert an `HttpArgs` into a `reqwest::Client`.
    fn try_from(http_config: HttpArgs) -> Result<Self, Self::Error> {
        build_async_client(http_config)
    }
}


    fn build_client_base(http_config: HttpArgs) -> Result<Client, AnyhowError> {
        let mut client_config = ClientBuilder::new();
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
            client_config = if http_version.as_str() == "1.1" {
                info!("Still HTTP version 1.1.");
                client_config.http1_only()
            } else {
                warn!("Unsupported http version, using default http version 1.1.");
                client_config.http1_only()
            }
        }

        let mut request_header_headermap = HeaderMap::new();
        // comma seperated header value e.g name:johndoe,age:23
        if let Some(request_headers_str) = http_config.request_headers {
            info!("Setting up user-defined HTTP headers.");
            for header in request_headers_str.split(',').map(str::trim) {
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



fn build_async_client(
    http_config: HttpArgs,
) -> Result<reqwest::Client, AnyhowError> {
    build_client_base(http_config)
}