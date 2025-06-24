#![allow(unused)]
use std::borrow::Cow;
use std::io::Read;
use std::rc::Rc;
use std::{collections::HashMap, time::Duration};

use crate::types::HttpClientConfig;
use anyhow::{Context, Error, Result};
use colored::Colorize;
use rayon;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::cookie::Cookie;
use reqwest::header::{
    HeaderMap, HeaderValue, ACCESS_CONTROL_REQUEST_HEADERS, CONTENT_DISPOSITION, CONTENT_TYPE,
    RANGE, SET_COOKIE,
};
use reqwest::{redirect::Policy, NoProxy, Proxy, Url};

fn build_client(http_config: HttpClientConfig) -> Result<()> {
    let mut client_config = ClientBuilder::new();

    //Configure redirect policy.

    let policy: Policy = if let Some(max_redirects) = http_config.max_redirects {
        Policy::limited(max_redirects)
    } else {
        Policy::default()
    };
    // Configure request timeout default to sixty seconds.
    let timeout = Duration::new(http_config.timeout.unwrap_or(60), 0);

    client_config = client_config.timeout(timeout).redirect(policy);

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

fn get_file_extension(url: Url, client: Client) -> Result<String> {
    //construct headermap.
    let mut header_map = HeaderMap::new();
    header_map.insert(
        RANGE,
        HeaderValue::from_str("bytes=0-2048")
            .context("can't convert supplied bytes range to ASCII text")?,
    );
    //make http request to site.
    let response = client
        .get(url.as_ref())
        .headers(header_map)
        .send()
        .context(format!(
            "{} can't send request to target url.",
            "Error:".red()
        ))?;
    // check for invalid status code.
    let response_status = response.error_for_status_ref();
    if let Err(err) = response_status {
        eprintln!(
            "{}",
            format!(
                "{} got status code {},it originated from url : {}",
                "Error:".red(),
                err.status().map_or_else(
                    || { "No status".red() },
                    |status| { status.as_str().blue() }
                ),
                err.url().unwrap_or(&url)
            )
        );
    }

    //Get content type for extension inference.
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .context(format!(
            "{} Can't get CONTENT_TYPE from http response header.",
            "Error:".red()
        ))?
        .clone();

    //Get bytes for memory buffer bases type inference.
    let bytes = response.bytes().context(format!(
        "{} Can't get response body as bytes",
        "Error:".red()
    ))?;

    let ext = infer_file_ext(&bytes);
    if let Some(ext) = ext {
        return Ok(ext);
    } else {
        let mut split_content_type = content_type
            .to_str()
            .context(format!(
                "{} Can't covert content-type string to valid ASCII",
                "Error:".red(),
            ))?
            .split("/")
            .collect::<Vec<&str>>();
        let ext = split_content_type
            .pop()
            .context(format!(
                "{} Can't parse content type string.",
                "Error:".red()
            ))?
            .trim_matches('"');
        return Ok(ext.into());
    }
}

fn check_name(url: Url, client: Client) -> Result<String> {
    let response = client.head(url.as_ref()).send().context(format!(
        "{} Can't get http response body for url {}",
        "Error".red(),
        &url
    ))?;

    let download_file_name = response.headers().get(CONTENT_DISPOSITION);
    if let Some(download_file_name) = download_file_name {
        let filename_str = download_file_name.to_str().context(format!(
            " {} Could'nt yield a string http header value contains no visible ASCII characters",
            "Error:".red()
        ))?;
        let mut header_split_vector = filename_str.split("filename=").collect::<Vec<&str>>();
        let filename = header_split_vector
            .pop()
            .context(format!(
                "{} Can't parse filename from context disposition header.",
                "Error".red()
            ))?
            .trim_matches('"');
        return Ok(filename.into());
    } else {
        let file_ext = get_file_extension(url, client)
            .context(format!("{} Could'nt get file extension", "Error:".red()))?;
        let random_no = rand::random_range(0..100_000_000); // remember to encode the random in base64.
        let filename: String = format!("{}", random_no) + "." + file_ext.as_str();
        return Ok(file_ext);
    }
}

#[test]
fn check_buffer_type_inference() -> Result<()> {
    let archive_path = std::path::Path::new("my_archive.zip"); // should be a valid path to an arhive
    let mut handle =
        std::fs::File::open(archive_path).context("couldn't open file my_archive.zip")?;
    let mut buf_reader = std::io::BufReader::new(handle);
    let mut buf = [0; 2048];
    buf_reader.read_exact(&mut buf);
    assert_eq!(infer_file_ext(&buf), Some("zip".to_string()));
    Ok(())
}
