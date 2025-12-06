//! utitlity module
use anyhow::Result;
use bytes::Bytes;
use futures::StreamExt;
use std::{future::Future, pin::Pin};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tracing::Level;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

type BoxedStream=Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>;

pub fn create_byte_stream<F,Fut>(
    buffer_size: usize,
    stream_producer: F,
) -> (
    BoxedStream,
    JoinHandle<()>,
) where F:FnOnce(mpsc::Sender<Result<Bytes>>)->Fut + Send + 'static,
    Fut:Future<Output=()> + Send + 'static,
    {
    let (tx, rx) = mpsc::channel::<Result<Bytes>>(buffer_size);

    let handle = tokio::spawn(async move {
        stream_producer(tx).await;
    });
    (ReceiverStream::new(rx).boxed(), handle)
}

///Intialize a logger for my tests.
/// # Arguements:
/// * level :This is the log level.
pub fn test_logger_init(level:Level){
    
    let filter = EnvFilter::builder()
        .with_default_directive(level.into()) // default = warn
        .from_env_lossy(); // respects RUST_LOG if user set it

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