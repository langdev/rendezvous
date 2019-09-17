use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nng::{
    options::{protocol::pair::Polyamorous, Options},
    Error, Message, PipeEvent, Protocol, Socket,
};
use tracing::info;

use crate::data::Event;

pub fn address() -> String {
    std::env::var("RENDEZVOUS_ADDR").unwrap_or_else(|_| "ipc:///var/tmp/rendezvous.pipe".to_owned())
}

pub fn spawn_socket(
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

    let address = address();

    loop {
        match socket.dial(&address) {
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

    let mut msg = Message::new()?;

    let s = socket.clone();
    thread::spawn(move || loop {
        let msg = s.recv().unwrap();
        info!("Recv");
        if let Ok(event) = serde_cbor::from_slice(msg.as_slice()) {
            msg_tx.send(event).unwrap();
        }
    });

    thread::spawn(move || {
        for e in event_rx {
            msg.clear();
            if serde_cbor::to_writer(&mut msg, &e).is_ok() {
                info!("sending...");
                socket.send(msg.clone()).map_err(nng::Error::from).unwrap();
            }
        }
    });

    Ok(())
}
