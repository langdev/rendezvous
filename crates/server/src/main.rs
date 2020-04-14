// #![warn(clippy::all)]

mod discovery;
mod util;

use std::collections::HashMap;
use std::sync::Arc;

use async_std::task;
use futures::{channel::mpsc, prelude::*, select_biased};

use rendezvous_common::{
    anyhow,
    data::*,
    nng::{self, Message, Protocol, Socket},
    serde_cbor,
    tracing::{self, debug, debug_span, info, warn},
};

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    tracing::init()?;

    info!("Initializing...");

    let (tx, mut rx) = mpsc::unbounded();
    task::spawn_blocking(move || discovery::start(tx));

    let mut bouncers = HashMap::new();

    let span = debug_span!("main loop");
    let _enter = span.enter();

    let (msg_tx, mut msg_rx) = mpsc::unbounded();
    loop {
        select_biased! {
            srv_map = rx.select_next_some() => {
                update_services(&mut bouncers, &srv_map, &msg_tx).await;
                info!("bouncers: {}", bouncers.len());
            }
            i = msg_rx.select_next_some() => {
                let (sender, msg) = i;
                if let Ok(event) = serde_cbor::from_slice::<Event>(msg.as_slice()) {
                    debug!("Event: {:#?}", event);
                    process_message(&mut bouncers, sender, &msg).await;
                }
            }
            complete => break,
        }
    }

    Ok(())
}

struct Bouncer {
    socket: Socket,
    join_handle: futures::future::Fuse<task::JoinHandle<()>>,
}

async fn update_services(
    bouncers: &mut HashMap<Arc<str>, Bouncer>,
    srv_map: &HashMap<String, Vec<String>>,
    msg_tx: &mpsc::UnboundedSender<(Arc<str>, Message)>,
) {
    if let Some(s) = srv_map.get("rdv.bnc.compat") {
        for addr in s {
            let addr = &addr[..];
            if bouncers.contains_key(addr) {
                continue;
            }
            let addr: Arc<_> = addr.into();
            match establish(Arc::clone(&addr), msg_tx).await {
                Ok(s) => {
                    bouncers.insert(addr, s);
                }
                Err(e) => {
                    warn!("{}", e);
                }
            }
        }
        bouncers.retain(|addr, _| s.iter().find(|a| &a[..] == &addr[..]).is_some());
    }
}

async fn establish(
    addr: Arc<str>,
    tx: &mpsc::UnboundedSender<(Arc<str>, Message)>,
) -> nng::Result<Bouncer> {
    let socket = Socket::new(Protocol::Pair1)?;
    socket.dial_async(&addr)?;
    let mut tx = tx.clone();
    let mut msgs = util::messages(&socket).await?;
    let join_handle = task::spawn(async move {
        while let Some(Ok(i)) = msgs.next().await {
            if tx.send((addr.clone(), i)).await.is_err() {
                break;
            }
        }
    })
    .fuse();
    Ok(Bouncer {
        socket,
        join_handle,
    })
}

async fn process_message(
    bouncers: &mut HashMap<Arc<str>, Bouncer>,
    sender: Arc<str>,
    msg: &Message,
) {
    let mut broken_pipes = vec![];
    for (addr, b) in &*bouncers {
        if *addr == sender {
            continue;
        }
        match send_message(&b.socket, msg.as_slice()).await {
            Ok(()) => {}
            Err(nng::Error::Closed) => {
                broken_pipes.push(addr.clone());
            }
            Err(e) => {
                warn!("{}", e);
            }
        }
    }
    if !broken_pipes.is_empty() {
        bouncers.retain(|addr, _| !broken_pipes.contains(addr));
    }
}

async fn send_message(socket: &Socket, content: &[u8]) -> Result<(), nng::Error> {
    let mut m = Message::new();
    m.push_back(content);
    util::send(socket, m).await?;
    Ok(())
}
