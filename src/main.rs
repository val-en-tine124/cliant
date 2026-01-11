//! # Cliant
//!
//! A state-of-the-art HTTP client for embarrassingly parallel tasks.
//!
//! This module contains the main entry point for the `cliant` application. It
//! parses command-line arguments, configures the HTTP client, and starts the
//! download process.

use clap::{Subcommand,Parser};
use anyhow::Result;
use features::save_to_local::{cli::LocalArgs,handler::handle};

use crate::interfaces::cli::set_up_cli_app;
mod application;
mod domain;
mod infra;
mod interfaces;
mod utils;
mod features;
mod shared;
#[derive(Clone,Parser)]
#[command(version="0.1.0",about="A state-of-the-art,high performance Data Mover for embarrassingly parallel tasks.",long_about=None)]
struct Cliant{
    #[command(subcommand)]
    command:Commands,
}

#[derive(Subcommand,Clone)]
enum Commands{
    Download(LocalArgs),
}


#[tokio::main]
async fn main()->Result<()>{
    human_panic::setup_panic!();
    set_up_cli_app().await?;
    Ok(())
}