mod discovery;
mod util;

use std::collections::HashSet;
use std::sync::Arc;

use futures::prelude::*;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing::init()?;

    info!("Initializing...");

    let bouncers = Arc::new(RwLock::default());

    let socket = Socket::new(Protocol::Pair1)?;
    socket.set_opt::<Polyamorous>(true)?;

    let b = bouncers.clone();
    socket.pipe_notify(move |pipe, event| handle_pipe_events(&mut b.write(), pipe, event))?;

    let address = ipc::address();
    let mut srv = util::serve(&socket, &address).await?;
    info!("Listening...");

    let span = debug_span!("main loop");
    let _enter = span.enter();

    while let Some(mut msg) = srv.try_next().await? {
        debug!("pipe: {:?}", msg.pipe());

        if let Ok(event) = serde_cbor::from_slice::<Event>(msg.as_slice()) {
            debug!("Event: {:#?}", event);
            process_message(&socket, &bouncers, &mut msg);
        }
    }

    Ok(())
}

fn process_message(socket: &Socket, bouncers: &RwLock<HashSet<Pipe>>, msg: &mut Message) {
    let mut broken_pipes = vec![];
    let sender = msg.pipe();
    for b in bouncers
        .read()
        .iter()
        .copied()
        .filter(|&b| sender != Some(b))
    {
        match send_message(&socket, b, msg.as_slice()) {
            Ok(()) => {}
            Err(nng::Error::Closed) => {
                broken_pipes.push(b);
            }
            Err(e) => {
                warn!("{}", e);
            }
        }
    }
    if !broken_pipes.is_empty() {
        bouncers.write().retain(|b| !broken_pipes.contains(b));
    }
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
