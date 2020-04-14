use std::pin::Pin;
use std::task::{Context, Poll};

use async_std::task;
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use pin_project::pin_project;

use rendezvous_common::{
    nng::{self, Aio, AioResult, Message, Socket},
    parking_lot::Mutex,
};

pub async fn messages(socket: &Socket) -> nng::Result<impl Stream<Item = nng::Result<Message>>> {
    let (tx, rx) = mpsc::unbounded();
    let s = socket.clone();
    let aio = Aio::new(move |aio, res| handle(&aio, &s, &tx, res).unwrap())?;
    socket.recv_async(&aio)?;
    task::yield_now().await;
    Ok(Wrap { aio, inner: rx })
}

fn handle(
    aio: &Aio,
    socket: &Socket,
    tx: &mpsc::UnboundedSender<nng::Result<Message>>,
    res: AioResult,
) -> nng::Result<()> {
    use nng::AioResult::*;
    if tx.is_closed() {
        return Ok(());
    }
    if let Recv(r) = res {
        if tx.unbounded_send(r).is_ok() {
            return socket.recv_async(aio);
        }
    }
    Ok(())
}

pub async fn send(socket: &Socket, msg: impl Into<Message>) -> Result<(), (Message, nng::Error)> {
    let (tx, rx) = oneshot::channel();
    let tx = Mutex::new(Some(tx));
    let aio = match nng::Aio::new(move |_, res| {
        let _ = handle_send(&tx, res);
    }) {
        Ok(a) => a,
        Err(e) => {
            return Err((msg.into(), e));
        }
    };
    socket.send_async(&aio, msg)?;
    task::yield_now().await;
    Wrap { aio, inner: rx }.await.unwrap()
}

fn handle_send(
    tx: &Mutex<Option<oneshot::Sender<Result<(), (Message, nng::Error)>>>>,
    res: AioResult,
) -> Option<()> {
    use nng::AioResult::*;
    if let Send(r) = res {
        let mut lock = tx.try_lock()?;
        let tx = lock.take()?;
        let _ = tx.send(r);
    }
    Some(())
}

#[pin_project]
#[derive(Debug)]
struct Wrap<T> {
    aio: Aio,
    #[pin]
    inner: T,
}

impl<T: Future> Future for Wrap<T> {
    type Output = <T as Future>::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        Future::poll(self.project().inner, cx)
    }
}

impl<T: Stream> Stream for Wrap<T> {
    type Item = <T as Stream>::Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        Stream::poll_next(self.project().inner, cx)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[async_std::test]
    async fn messages_basic() {
        let address = "inproc://messages_basic";

        let socket = Socket::new(nng::Protocol::Pair1).unwrap();
        socket.listen(address).unwrap();
        let msgs = messages(&socket).await.unwrap();

        task::spawn_blocking(move || {
            use std::io::Write as _;
            let socket = Socket::new(nng::Protocol::Pair1).unwrap();
            socket.dial(address).unwrap();
            let mut msg = Message::new();
            for i in 1..=3 {
                msg.clear();
                write!(msg, "{}", i).unwrap();
                socket.send(msg.clone()).unwrap();
            }
        });

        let data = msgs.take(3).try_collect::<Vec<_>>().await.unwrap();
        assert_eq!(data[0].as_slice(), b"1");
        assert_eq!(data[1].as_slice(), b"2");
        assert_eq!(data[2].as_slice(), b"3");
    }

    #[async_std::test]
    async fn send_basic() {
        let address = "inproc://send_basic";
        let res = task::spawn_blocking(move || {
            let socket = Socket::new(nng::Protocol::Pair1)?;
            socket.listen(address)?;
            socket.recv()
        });

        let socket = Socket::new(nng::Protocol::Pair1).unwrap();
        socket.dial_async(address).unwrap();
        let mut msg = Message::new();
        msg.push_back(b"Hello");
        send(&socket, msg).await.unwrap();

        let recv = res.await.unwrap();
        assert_eq!(recv.as_slice(), b"Hello");
    }
}
