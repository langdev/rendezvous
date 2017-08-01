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
            let mut nickname = String::new();
            for message in c.iter() {
                let message = message.unwrap();
                debug!(log, "{}", message);
                nickname.clear();
                if let Some(nick) = message.source_nickname() {
                    nickname.push_str(nick);
                }
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
                        nickname = String::new();
                    }
                    _ => { }
                }
            }
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
