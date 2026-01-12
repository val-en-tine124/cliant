//! # Cliant
//!
//! A state-of-the-art HTTP client for embarrassingly parallel tasks.
//!
//! This module contains the main entry point for the `cliant` application. It
//! parses command-line arguments, configures the HTTP client, and starts the
//! download process.

use clap::{ArgAction, Parser, Subcommand};
use anyhow::Result;
use features::save_to_local::{cli::LocalArgs,handler::handle};
use tracing::Level;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
mod utils;
mod features;
mod shared;
#[derive(Clone,Parser)]
#[command(version="0.1.0",about="A state-of-the-art, high performance Data Mover for embarrassingly parallel tasks.",long_about=None)]
struct Cliant{
    #[command(subcommand)]
    command:Commands,
    /// Set the Logging level to quiet. Less information about download events are emitted i.e only Errors.
    #[arg(short = 'q', long = "quiet",)]
    pub quiet: bool,
    /// Set the Logging level to verbose. More information about download events are emitted.
    #[arg(short='v',long="verbose",action=ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand,Clone)]
enum Commands{
    Download(LocalArgs),
}

fn setup_tracing(args: &Cliant) {
    // Start with a base filter
    let mut filter = EnvFilter::builder()
        .with_default_directive(Level::WARN.into()) // default = warn
        .from_env_lossy(); // respects RUST_LOG if user set it

    // Override with -q / -v flags (unless user explicitly set RUST_LOG)
    if std::env::var("RUST_LOG").is_err() {
        if args.quiet {
            filter = filter.add_directive(Level::ERROR.into());
        } else {
            match args.verbose {
                0 => filter = filter.add_directive(Level::WARN.into()),
                1 => filter = filter.add_directive(Level::INFO.into()),
                2 => filter = filter.add_directive(Level::DEBUG.into()),
                _ => filter = filter.add_directive(Level::TRACE.into()),
            }
        }
}

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_ansi(true) // colors in terminal
                .with_target(false) // cleaner output
                .with_file(false)
                .with_line_number(false)
                .compact(),
        ) // one-line format, perfect for CLIs
        .init();
}

#[tokio::main]
async fn main()->Result<()>{
    human_panic::setup_panic!();
    let args= Cliant::parse();
    setup_tracing(&args);
    match args.command{
        Commands::Download(local_args)=>{
            handle(local_args).await?;
        }
    }
    Ok(())
}