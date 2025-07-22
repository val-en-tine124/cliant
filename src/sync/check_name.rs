use anyhow::{Context, Result};
use colored::Colorize;
use log::{error, info};
use regex::Regex;
use reqwest::blocking::Client;

use rand::Rng;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_DISPOSITION, CONTENT_TYPE, RANGE};
use reqwest::Url;
use super::retry_request;


///This function will infer file extension with infer crate.
fn infer_file_ext(buf: &[u8]) -> Option<String> {
    let inferred_type = infer::get(buf);
    if let Some(inferred_type) = inferred_type {
        info!(
            "Inferred type '{}' for download file",
            inferred_type.extension()
        );
        return Some(inferred_type.extension().to_string());
    }
    None
}

fn get_file_extension(url: Url, client: &Client) -> Result<String> {
    //construct headermap.
    let mut header_map = HeaderMap::new();
    info!("Getting a part of download file for type inference.");
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
        .context("Can't send request to target url.".yellow())?;
    // check for invalid status code.
    let response_status = response.error_for_status_ref();
    if let Err(err) = response_status {
        let status = err
            .status()
            .map_or_else(|| "No status code".red(), |status| status.as_str().blue());
        error!("Got invalid status code {}", &status);
        eprintln!(
            "{}",
            format!(
                "Got status code {},it originated from url : {}",
                err.url().unwrap_or(&url),
                status,
            )
            .yellow()
        );
    }

    //Get content type for extension inference.
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .context("Can't get CONTENT_TYPE from http response header.".yellow())?
        .clone();

    //Get bytes for memory buffer bases type inference.
    let bytes = response
        .bytes()
        .context("Can't get response body as bytes".yellow())?;

    let ext = infer_file_ext(&bytes);
    if let Some(ext) = ext {
        return Ok(ext);
    } else {
        info!("Could'nt infer file type from downloaded file chunk.");

        let mut split_content_type = content_type
            .to_str()
            .context("Can't covert content-type string to valid ASCII".yellow())?
            .split("/")
            .collect::<Vec<&str>>();
        let ext = split_content_type
            .pop()
            .context("Can't parse content type string.".yellow())?
            .trim_matches('"');
        info!("Parsing content type header.");
        return Ok(ext.into());
    }
}

pub fn check_name(url: Url, client: &Client) -> Result<String> {
    let response_result_fn = || client.head(url.as_ref()).send();
    let resp_retry_result = retry_request(3, response_result_fn);
    match resp_retry_result {
        Ok(response) => {
            info!("Checking file name from server.");
            if let Some(download_file_name) = response.headers().get(CONTENT_DISPOSITION) {
                let filename_str = download_file_name.to_str().context(
                    " Could'nt yield a string, http header value contains no visible ASCII characters",
                )?;

                let regex = Regex::new(r#"filename="?([^"\s]+)"?"#)?;

                if let Some(captures) = regex.captures(filename_str) {
                    if let Some(filename) = captures.get(1) {
                        info!("Got filename from server {}.", filename.as_str());
                        return Ok(filename.as_str().into());
                    }
                }
            }

            info!("Can't get filename from server generating random name.");
            let file_ext =
                get_file_extension(url, client).context("Could'nt get file extension")?;
            let random_no: u32 = rand::thread_rng().gen();
            let filename: String = format!("{}", random_no) + "." + file_ext.as_str();
            return Ok(filename.into());
        }

        Err(error) => {
            return Err(error);
        }
    }
}

#[test]
fn check_buffer_type_inference() -> Result<()> {
    use std::io::Read;

    let archive_path = std::path::Path::new("temp_test_dir/archive.zip"); // should be a valid path to an arhive
    let mut file = std::fs::File::open(archive_path)?;
    let mut buffer = Vec::with_capacity(2048);
    file.read_to_end(&mut buffer)?;
    let ext = infer_file_ext(&buffer).unwrap();
    println!("ext is:{}", ext);
    assert_eq!(ext, "zip");
    Ok(())
}
