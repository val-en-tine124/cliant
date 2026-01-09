//! # Cliant
//!
//! A state-of-the-art HTTP client for embarrassingly parallel tasks.
//!
//! This module contains the main entry point for the `cliant` application. It
//! parses command-line arguments, configures the HTTP client, and starts the
//! download process.

use crate::interfaces::cli::set_up_cli_app;
use anyhow::Result;

mod application;
mod domain;
mod infra;
mod interfaces;
mod utils;
mod features;
mod shared;

#[tokio::main]
async fn main()->Result<()>{
    human_panic::setup_panic!();
    set_up_cli_app().await?;
    Ok(())
}