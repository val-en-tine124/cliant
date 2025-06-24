#![allow(unused)]
use std::borrow::Cow;
use std::{collections::HashMap, time::Duration};

use crate::types::HttpClientConfig;
use anyhow::{Context, Error, Result};
use colored::Colorize;
use rayon;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::cookie::Cookie;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_DISPOSITION, CONTENT_TYPE, RANGE};
use reqwest::{redirect::Policy, NoProxy, Proxy, Url};

fn build_client(http_config: HttpClientConfig) -> Result<()> {
    //Configure redirect policy.
    let policy: Policy = if http_config.follow_redirects {
        Policy::default()
    } else {
        Policy::none()
    };

    // Configure request timeout.
    let timeout = Duration::new(http_config.timeout.unwrap_or(60), 0);

    let mut client_config = ClientBuilder::new().timeout(timeout).redirect(policy);

    // Set proxy url if it's present else default to no proxy.
    if let Some(proxy_url) = http_config.proxy_url {
        client_config = client_config.proxy(
            Proxy::all(proxy_url).context("failed to proxy all traffic to the passed proxy url")?,
        );
    } else {
        client_config = client_config.no_proxy();
    }

    //conditionally set http version to use
    if http_config.http1 {
        client_config = client_config.http1_only();
    }
    if http_config.http2 {
        client_config = client_config.http2_prior_knowledge();
    }

    Ok(())

    // remember to build cargo to add reqwest cookies features and client_config.build().
}
///This function will infer file extension with infer crate.
fn infer_file_ext(buf: &[u8]) -> Option<String> {
    let inferred_type = infer::get(buf);
    if let Some(inferred_type) = inferred_type {
        return Some(inferred_type.extension().to_string());
    }
    None
}