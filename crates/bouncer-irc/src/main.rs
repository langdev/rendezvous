#![deny(rust_2018_idioms)]
#![deny(proc_macro_derive_resolution_fallback)]

use std::sync::mpsc;
use std::thread;

use failure::{Fallible, ResultExt as _};
use irc::client::prelude::*;

use rendezvous_common::{
    anyhow,
    data::*,
    ipc,
    tracing::{self, info},
};

fn main() -> anyhow::Result<()> {
    tracing::init()?;
    let (event_tx, event_rx) = mpsc::channel();
    let (msg_tx, msg_rx) = mpsc::channel();
    ipc::spawn_socket(msg_tx, event_rx)?;
    spawn(event_tx, msg_rx).compat()?;
    Ok(())
}

fn spawn(event_tx: mpsc::Sender<Event>, msg_rx: mpsc::Receiver<Event>) -> Fallible<()> {
    let client = IrcClient::new("config.toml")?;
    client.identify()?;
    info!("connected");
    spawn_event_handler(client.clone(), msg_rx);
    client.for_each_incoming(|irc_msg| {
        let nickname = irc_msg.source_nickname().unwrap_or("").into();
        match irc_msg.command {
            Command::PRIVMSG(channel, content) => {
                info!("privmsg");
                event_tx
                    .send(Event::MessageCreated {
                        nickname,
                        channel,
                        content,
                        origin: None,
                    })
                    .unwrap();
            }
            _ => {}
        }
    })?;
    Ok(())
}

fn spawn_event_handler(client: IrcClient, msg_rx: mpsc::Receiver<Event>) {
    thread::spawn(move || {
        for e in msg_rx {
            match e {
                Event::MessageCreated {
                    nickname,
                    channel,
                    content,
                    ..
                } => {
                    client
                        .send_privmsg(&channel, &format!("<{}> {}", nickname, content))
                        .unwrap();
                }
                _ => {}
            }
        }
    });
}
