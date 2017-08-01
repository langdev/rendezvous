use std::thread;

use futures::{BoxFuture, Future, Stream, lazy};
use futures::future::{self, Loop, loop_fn};
use irc::client::prelude::*;
use slog;

use ::{Error, Result};
use message::{Bus, BusSender, Message, MessageCreated, Payload};


pub struct Irc {
    client: IrcServer,
}

impl Irc {
    pub fn from_config<L>(logger: L, bus: Bus, cfg: Config) -> BoxFuture<Irc, Error>
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
        }).boxed()
    }
}

fn listen_messages(
    client: IrcServer,
    log: slog::Logger,
    sender: BusSender,
) -> BoxFuture<(), Error> {
    client.stream().map_err(From::from).boxed().for_each(move |message| {
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
                let s = sender.clone();
                loop_fn(sender.try_send(m), move |res| {
                    use std::sync::mpsc::TrySendError::*;
                    Ok(match res {
                        Ok(..) => Loop::Break(()),
                        Err(Full(p)) => Loop::Continue(s.try_send(p)),
                        Err(Disconnected(_)) => { panic!("bus closed"); }
                    })
                }).boxed()
            }
            _ => { future::ok(()).boxed() }
        }
    }).boxed()
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
