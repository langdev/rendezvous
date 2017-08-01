use std::thread;

use futures::{Future, Stream, lazy};
use irc::client::prelude::*;
use slog;

use ::{Error, Result};
use message::{Bus, BusSender, Message, MessageCreated, Payload};


pub struct Irc {
    client: IrcServer,
}

impl Irc {
    pub fn from_config<L>(logger: L, bus: Bus, cfg: Config) -> impl Future<Item=Irc, Error=Error>
        where L: Into<Option<slog::Logger>>
    {
        let logger = logger.into().unwrap_or_else(|| slog::Logger::root(slog::Discard, o!()));
        let log = logger.new(o!());
        let sender = bus.sender();
        lazy::<_, Result<_>>(move || {
            let client = IrcServer::from_config(cfg)?;
            client.identify()?;
            spawn_actor(logger, client.clone(), bus);
            Ok(client)
        }).and_then(move |client| {
            let c = client.clone();
            listen_messages(c, log, sender)
                .map(move |_| client)
        }).map(move |client| {
            Irc {
                client,
            }
        })
    }
}

fn listen_messages(
    client: IrcServer,
    log: slog::Logger,
    sender: BusSender,
) -> impl Future<Item=(), Error=Error> {
    client.stream().for_each(move |message| {
        debug!(log, "{}", message);
        let nickname = message.source_nickname()
            .map(String::from)
            .unwrap_or_else(String::new);
        match message.command {
            Command::PRIVMSG(target, content) => {
                let mut m = Message::MessageCreated(MessageCreated {
                    nickname,
                    channel: target,
                    content,
                });
                while let Err(e) = sender.try_send(m) {
                    use std::sync::mpsc::TrySendError::*;
                    m = match e {
                        Full(p) => p,
                        Disconnected(_) => { panic!("bus closed"); }
                    };
                    thread::yield_now();
                }
            }
            _ => { }
        }
        Ok(())
    }).map_err(From::from)
}

fn spawn_actor(logger: slog::Logger, client: IrcServer, bus: Bus) {
    use message::Message::*;
    thread::spawn(move || {
        for Payload { message, .. } in bus {
            match message {
                MessageCreated(msg) => {
                    let m = format!("<{}> {}", msg.nickname, msg.content);
                    if let Err(e) = client.send(Command::PRIVMSG(msg.channel, m)) {
                        error!(logger, "failed to send a message: {}", e);
                    }
                }
            }
        }
    });
}
