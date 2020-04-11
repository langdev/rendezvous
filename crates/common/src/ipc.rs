use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nng::{
    options::{protocol::pair::Polyamorous, Options},
    Error, Message, PipeEvent, Protocol, Socket,
};
use serde::{de::DeserializeOwned, ser::Serialize};
use tracing::info;

use crate::data::Event;

pub fn address() -> String {
    std::env::var("RENDEZVOUS_ADDR").unwrap_or_else(|_| "ipc:///var/tmp/rendezvous.pipe".to_owned())
}

pub fn spawn_socket(
    msg_tx: mpsc::Sender<Event>,
    event_rx: mpsc::Receiver<Event>,
) -> Result<(), Error> {
    let address = address();
    spawn_socket_impl(&address, msg_tx, event_rx)
}

fn spawn_socket_impl(
    address: &str,
    msg_tx: mpsc::Sender<Event>,
    event_rx: mpsc::Receiver<Event>,
) -> Result<(), Error> {
    let socket = Socket::new(Protocol::Pair1)?;
    socket.set_opt::<Polyamorous>(true)?;

    socket.pipe_notify(move |_, event| match event {
        PipeEvent::AddPre => info!("AddPre"),
        PipeEvent::AddPost => info!("AddPost"),
        PipeEvent::RemovePost => info!("RemovePost"),
        _ => {}
    })?;

    dial(&socket, address)?;

    let s = socket.clone();
    thread::spawn(move || receive_from_socket(s, msg_tx));
    thread::spawn(move || send_to_socket(socket, event_rx));

    Ok(())
}

fn dial(socket: &Socket, address: &str) -> Result<(), Error> {
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
    use parking_lot::Mutex;

    use super::*;

    fn socket_pair1_new() -> Result<Socket, Error> {
        let sock = Socket::new(Protocol::Pair1)?;
        sock.set_opt::<Polyamorous>(true)?;
        Ok(sock)
    }

    enum Command {
        Notify {
            pipe: nng::Pipe,
            event: nng::PipeEvent,
        },
        Close,
    }

    #[test]
    fn receive_basic() {
        let (msg_tx, msg_rx) = mpsc::channel();

        let (tx, rx) = mpsc::channel();
        let addr = "inproc://receive_basic";
        let pipe_tx = Mutex::new(Some(tx.clone()));
        thread::spawn(move || {
            let srv_sock = socket_pair1_new().unwrap();
            srv_sock
                .pipe_notify(move |pipe, event| {
                    let mut tx = pipe_tx.lock();
                    if let Some(t) = tx.as_ref() {
                        if let Err(_) = t.send(Command::Notify { pipe, event }) {
                            *tx = None;
                        }
                    }
                })
                .unwrap();
            srv_sock.listen(addr).unwrap();
            for cmd in rx {
                match cmd {
                    Command::Notify {
                        pipe,
                        event: PipeEvent::AddPost,
                    } => {
                        let mut m = Message::new();
                        m.set_pipe(pipe);
                        m.push_back(b"\x6cHello client");
                        srv_sock.send(m).unwrap();
                    }
                    Command::Close => {
                        break;
                    }
                    _ => {}
                }
            }
            srv_sock.close();
        });
        let _guard = scopeguard::guard((), move |_| {
            let _ = tx.send(Command::Close);
        });

        let client_sock = socket_pair1_new().unwrap();
        dial(&client_sock, addr).unwrap();
        thread::spawn(move || receive_from_socket(client_sock, msg_tx));

        let s: String = msg_rx.recv().unwrap();
        assert_eq!(s, "Hello client");
    }

    #[test]
    fn send_basic() {
        let (event_tx, event_rx) = mpsc::channel();

        let (tx, rx) = mpsc::channel();
        let addr = "inproc://send_basic";
        thread::spawn(move || {
            let srv_sock = socket_pair1_new().unwrap();
            srv_sock.listen(addr).unwrap();
            let msg = srv_sock.recv().unwrap();
            tx.send(msg).unwrap();
            srv_sock.close();
        });

        let client_sock = socket_pair1_new().unwrap();
        dial(&client_sock, addr).unwrap();
        thread::spawn(move || send_to_socket(client_sock, event_rx));
        event_tx.send("Hello server".to_owned()).unwrap();

        let m = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(m.as_slice(), b"\x6cHello server");
    }
}
