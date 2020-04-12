use std::pin::Pin;
use std::task::{Context, Poll};

use futures::prelude::*;
use pin_project::pin_project;
use tokio::sync::mpsc;

use rendezvous_common::nng;

pub async fn serve(
    socket: &nng::Socket,
    address: &str,
) -> nng::Result<impl Stream<Item = nng::Result<nng::Message>>> {
    Serve::new(socket, address).await
}

#[pin_project]
#[derive(Debug)]
pub struct Serve {
    aio: nng::Aio,
    #[pin]
    rx: mpsc::UnboundedReceiver<nng::Result<nng::Message>>,
}

impl Serve {
    async fn new(socket: &nng::Socket, address: &str) -> nng::Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let c = socket.clone();
        let aio = nng::Aio::new(move |aio, res| handle(&aio, &c, &tx, res).unwrap())?;
        socket.listen(address)?;
        socket.recv_async(&aio)?;
        tokio::task::yield_now().await;
        Ok(Serve { aio, rx })
    }
}

fn handle(
    aio: &nng::Aio,
    socket: &nng::Socket,
    tx: &mpsc::UnboundedSender<nng::Result<nng::Message>>,
    res: nng::AioResult,
) -> Result<(), nng::Error> {
    use nng::AioResult::*;
    match dbg!(res) {
        Recv(r) => {
            if let Err(_) = tx.send(r) {
                Ok(())
            } else {
                socket.recv_async(aio)
            }
        }
        _ => Ok(()),
    }
}

impl Stream for Serve {
    type Item = nng::Result<nng::Message>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        dbg!(&self);
        let rx = self.project().rx;
        Stream::poll_next(rx, cx)
    }
}
