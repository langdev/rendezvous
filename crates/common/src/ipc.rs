use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nng::{Error, Message, Protocol, Socket};
use serde::{de::DeserializeOwned, ser::Serialize};
use tracing::info;

use crate::data::Event;

pub fn address() -> String {
    std::env::var("RENDEZVOUS_ADDR").unwrap_or_else(|_| "ipc:///var/tmp/rendezvous.pipe".to_owned())
}

pub fn spawn_socket(
    address: &str,
    msg_tx: mpsc::Sender<Event>,
    event_rx: mpsc::Receiver<Event>,
) -> nng::Result<()> {
    let socket = Socket::new(Protocol::Pair1)?;
    socket.listen(address)?;
    info!("listen");

    let s = socket.clone();
    thread::spawn(move || receive_from_socket(s, msg_tx));
    thread::spawn(move || send_to_socket(socket, event_rx));

    Ok(())
}

pub(crate) fn dial(socket: &Socket, address: &str) -> nng::Result<()> {
    loop {
        match socket.dial(address) {
            Ok(_) => {
                break;
            }
            Err(Error::ConnectionRefused) => {
                thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                return Err(e);
            }
        }
        info!("Reconnecting...");
    }
    Ok(())
}

fn receive_from_socket<T: DeserializeOwned>(socket: Socket, tx: mpsc::Sender<T>) {
    loop {
        let msg = socket.recv().unwrap();
        info!("Recv");
        if let Ok(event) = serde_cbor::from_slice(msg.as_slice()) {
            if let Err(_) = tx.send(event) {
                info!("channel closed");
                break;
            }
        }
    }
}

fn send_to_socket<T: Serialize>(socket: Socket, rx: mpsc::Receiver<T>) {
    let mut msg = Message::new();
    for e in rx {
        msg.clear();
        if serde_cbor::to_writer(&mut msg, &e).is_ok() {
            info!("sending...");
            socket.send(msg.clone()).map_err(nng::Error::from).unwrap();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn socket_pair1_new() -> Result<Socket, Error> {
        let sock = Socket::new(Protocol::Pair1)?;
        Ok(sock)
    }

    #[test]
    fn receive_basic() {
        let (msg_tx, msg_rx) = mpsc::channel();

        let addr = "inproc://receive_basic";
        let server_socket = socket_pair1_new().unwrap();
        server_socket.listen(addr).unwrap();

        thread::sleep(Duration::from_secs(1));
        thread::spawn(move || {
            let client_socket = socket_pair1_new().unwrap();
            dial(&client_socket, addr).unwrap();
            info!("connected");
            let mut m = Message::new();
            m.push_back(b"\x6cHello server");
            client_socket.send(m).unwrap();
            thread::yield_now();
        });

        thread::sleep(Duration::from_secs(1));
        thread::spawn(move || receive_from_socket(server_socket, msg_tx));

        let s: String = msg_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(s, "Hello server");
    }

    #[test]
    fn send_basic() {
        let (event_tx, event_rx) = mpsc::channel();

        let addr = "inproc://send_basic";
        let server_socket = socket_pair1_new().unwrap();
        server_socket.listen(addr).unwrap();
        thread::spawn(move || send_to_socket(server_socket, event_rx));

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let client_socket = socket_pair1_new().unwrap();
            dial(&client_socket, addr).unwrap();
            let msg = client_socket.recv().unwrap();
            tx.send(msg).unwrap();
            client_socket.close();
        });

        event_tx.send("Hello client".to_owned()).unwrap();

        let m = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(m.as_slice(), b"\x6cHello client");
    }
}
