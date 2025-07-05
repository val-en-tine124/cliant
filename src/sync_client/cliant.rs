#![allow(unused)]

use std::env;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::os::windows::fs::MetadataExt;
use std::path::{absolute, Path};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result};
use colored::Colorize;
use log::{error, info, log, warn, Level};
use regex::Regex;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{
    HeaderMap, HeaderValue, ACCESS_CONTROL_REQUEST_HEADERS, CONTENT_DISPOSITION, CONTENT_LENGTH,
    CONTENT_TYPE, COOKIE, RANGE,
};
use reqwest::{redirect::Policy, Proxy, Url};

use crate::types::HttpClientConfig;
use crate::check_name::check_name;


fn download_file(
    url: Url,
    client: Client,
    filename: String,
    downloaded_size: u64,
    preferred_folder: &Path,
) -> Result<()> {
    info!("Downloading file:{filename} in folder {preferred_folder:?}...");
    info!("Got downloaded file size {downloaded_size}\n, creating httpx Headers object with Range {downloaded_size}.");
    let header_string = format!("bytes={downloaded_size}-");

    let mut response = client
        .get(url.as_ref())
        .header(RANGE, &header_string)
        .send()
        .context("Can't result Download.")?;
    let content_length = response
        .content_length()
        .context("Can't get content length")?;


    info!("Got content length:{content_length} for {url}");

    let cliant_root = env::var("CLIENT_ROOT")?;

    let resume_download = if downloaded_size > 0 { true } else { false };

    let file_path = Path::new(&cliant_root).join(filename);

    let mut file_handle = if resume_download {
        fs::OpenOptions::new().append(true).open(&file_path)
    } else {
        fs::OpenOptions::new().create(true).open(&file_path)
    }
    .context(format!("Can't open file:{:?}", &file_path).yellow())?;

    let mut buffer = [0; 1024];
    while let Ok(bytes) = response.read(&mut buffer) {
        if bytes == 0 {
            break;
        }
        file_handle.write(&buffer[..bytes])?;
        info!("writing {bytes} to {file_path:?}");

        
    }
    info!("Downloading completed {content_length} bytes has been downloaded.");

    Ok(())
}

#[test]
fn check_buffer_type_inference() -> Result<()> {
    // This is safe on Windows single threaded and multi-threaded programs.

    unsafe {
        env::set_var("RUST_BACKTRACE", "0");
    }

    let archive_path = std::path::Path::new("my_archive.zip"); // should be a valid path to an arhive
    let handle =
        std::fs::File::open(archive_path).context("couldn't open file my_archive.zip".yellow())?;
    let mut buf_reader = std::io::BufReader::new(handle);
    let mut buf = [0; 2048];
    buf_reader.read_exact(&mut buf)?;
    assert_eq!(infer_file_ext(&buf), Some("zip".to_string()));
    Ok(())
}

#[test]
fn test_check_fs_file() -> Result<()> {
    unsafe {
        env::set_var("RUST_BACKTRACE", "0");
    }

    let size = file_on_fs("new_file.py".into(), Path::new("C:\\new_rs_folder"), true)?;
    println!("file size is:{size} bytes");
    let cliant_root = env::var("CLIANT_ROOT")?;
    println!("CLIANT ROOT IS :{}", cliant_root);
    Ok(())
}

#[test]
fn test_download_file()->Result<()>{
    unsafe {
        env::set_var("RUST_BACKTRACE", "0");
    }

    let client=Client::new();
    let url= Url::from_str("https://sth.com")?;
    let filename="my_file.html".to_string();
    download_file(url, client,filename,0,Path::new("C:\\"));
    
    Ok(())

}