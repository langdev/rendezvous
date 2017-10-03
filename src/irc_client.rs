use std::thread;

use irc::client::prelude::*;
use slog;

use ::Result;
use message::{Bus, Message, MessageCreated, Payload};


pub struct Irc {
    client: IrcServer,
}

impl Irc {
    pub fn from_config<L>(logger: L, bus: Bus, cfg: Config) -> Result<Irc>
        where L: Into<Option<slog::Logger>>
    {
        let logger = logger.into().unwrap_or_else(|| slog::Logger::root(slog::Discard, o!()));
        let client = IrcServer::from_config(cfg)?;
        client.identify()?;
        let log = logger.new(o!());
        let c = client.clone();
        let sender = bus.sender();
        thread::spawn(move || {
            let mut pong_received = false;
            c.for_each_incoming(|message| {
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
                    Command::PONG(_, _) => {
                        if !pong_received {
                            pong_received = true;
                            debug!(log, "woah");
                        }
                    }
                    _ => { }
                }
            }).unwrap();
        });
        spawn_actor(logger, client.clone(), bus);
        Ok(Irc {
            client,
        })
    }
}

fn spawn_actor(logger: slog::Logger, client: IrcServer, bus: Bus) {
    use message::Message::*;
    thread::spawn(move || {
        for Payload { message, .. } in bus {
            match message {
                ChannelUpdated { channels } => {
                    let chanlist = channels.into_iter()
                        .map(|n| format!("#{}", n))
                        .collect::<Vec<_>>()
                        .join(",");
                    if let Err(e) = client.send_join(&chanlist) {
                        error!(logger, "failed to join a channel: {}", e);
                    }
                }
                MessageCreated(msg) => {
                    let m = format!("<{}> {}", msg.nickname, msg.content);
                    if let Err(e) = client.send(Command::PRIVMSG(msg.channel, m)) {
                        error!(logger, "failed to send a message: {}", e);
                    }
                }
                _ => { }
            }
        }
    });
}
