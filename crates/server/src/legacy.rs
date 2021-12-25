use std::collections::HashSet;
use std::sync::Arc;

use rendezvous_common::{
    anyhow,
    data::*,
    ipc,
    nng::{
        self,
        options::{protocol::pair::Polyamorous, Options},
        Message, Pipe, PipeEvent, Protocol, Socket,
    },
    parking_lot::RwLock,
    serde_cbor,
    tracing::{self, debug, debug_span, info, instrument, warn},
};

pub fn serve() -> anyhow::Result<()> {
    info!("Initializing...");

    let bouncers = Arc::new(RwLock::default());

    let socket = Socket::new(Protocol::Pair1)?;
    socket.set_opt::<Polyamorous>(true)?;

    let b = bouncers.clone();
    socket.pipe_notify(move |pipe, event| handle_pipe_events(&mut b.write(), pipe, event))?;

    let address = ipc::address();
    socket.listen(&address)?;
    info!("Listening...");

    let span = debug_span!("main loop");
    let _enter = span.enter();

    loop {
        let mut msg = socket.recv()?;
        debug!("pipe: {:?}", msg.pipe());

        if let Ok(event) = serde_cbor::from_slice::<Event>(msg.as_slice()) {
            debug!("Event: {:#?}", event);
            for b in bouncers.read().iter().copied() {
                if Some(b) == msg.pipe() {
                    continue;
                }
                if let Err(e) = send_message(&socket, b, msg.as_slice()) {
                    warn!("{}", e);
                }
            }
        }
    }

    Ok(())
}

#[instrument]
fn send_message(socket: &Socket, pipe: Pipe, content: &[u8]) -> Result<(), nng::Error> {
    let mut m = Message::new();
    m.set_pipe(pipe);
    m.push_back(content);
    socket.send(m)?;
    Ok(())
}

#[instrument]
fn handle_pipe_events(bouncers: &mut HashSet<Pipe>, pipe: Pipe, event: PipeEvent) {
    match event {
        PipeEvent::AddPost => {
            info!("AddPost");
            bouncers.insert(pipe);
        }
        PipeEvent::RemovePost => {
            info!("RemovePost");
            bouncers.remove(&pipe);
        }
        _ => {}
    }
    info!("Bouncers connected: {}", bouncers.len());
}
