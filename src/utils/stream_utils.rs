use anyhow::Result;
use bytes::Bytes;
use futures::StreamExt;
use std::{future::Future, pin::Pin};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_stream::{wrappers::ReceiverStream, Stream};

pub fn create_byte_stream<F,Fut>(
    buffer_size: usize,
    stream_producer: F,
) -> (
    Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>,
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
