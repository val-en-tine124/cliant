use std::time::Duration;
use reqwest::blocking::Response;
use anyhow::Result;
use colored::Colorize;
use log::{error, info};

pub fn retry_request<F>(max_retry_no: u8, function: F) -> Result<Response, anyhow::Error>
where
    F: Fn() -> Result<Response, reqwest::Error>,
{
    for current_retry in 1..=max_retry_no {
        match function() {
            Ok(response) => {
                return Ok(response);
            }
            Err(error) if error.is_connect() || error.is_timeout() || error.is_request() => {
                if let Some(err_url) = error.url() {
                    let url = err_url.clone();
                    info!("Can't get http response body for url {url}");
                }
                error!("Network error, retrying HTTP request {current_retry}...");
                std::thread::sleep(Duration::from_millis(10000));
                continue;
            }
            Err(error) => {
                if let Some(err_url) = error.url() {
                    let url = err_url.clone();
                    info!("Can't get http response body for url {url}");
                }

                return Err(error.into());
            }
        }
    }
    anyhow::bail!(format!("Spurious network operation timeout.").yellow());
}