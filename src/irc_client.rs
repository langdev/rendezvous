use std::collections::BTreeSet;
use std::sync::RwLock;

use actix::prelude::*;
use irc::{
    client::prelude::*,
    error::IrcError,
    proto::Message as IrcMessage,
    proto::Response as IrcResponse,
};
use log::*;

use crate::{fetch_config, Error};
use crate::message::{ChannelUpdated, MessageCreated, Subscribe, SubscriptionList};


pub struct Irc {
    client: IrcClient,
    channels: RwLock<BTreeSet<String>>,
    subscribers: SubscriptionList<MessageCreated>,
}

impl Irc {
    pub fn new() -> Result<Irc, Error> {
        let cfg = fetch_config();
        Irc::from_config(cfg.irc.clone())
    }

    pub fn from_config(cfg: Config) -> Result<Irc, Error> {
        let client = IrcClient::from_config(cfg)?;
        client.identify()?;
        Ok(Irc {
            client,
            channels: Default::default(),
            subscribers: Default::default(),
        })
    }

    fn sync_channel_joining(&self, new_channels: Vec<String>) -> Result<(), Error> {
        let mut ch = self.channels.write()?;
        ch.extend(new_channels);
        if ch.len() <= 0 { return Ok(()); }
        let chanlist = ch.iter()
            .map(|n| format!("#{}", n))
            .collect::<Vec<_>>()
            .join(",");
        self.client.send_join(&chanlist)?;
        Ok(())
    }
}

impl Actor for Irc {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.add_stream(self.client.stream());
    }
}

impl Handler<ChannelUpdated> for Irc {
    type Result = ();
    fn handle(&mut self, msg: ChannelUpdated, ctx: &mut Self::Context) -> Self::Result {
        if let Err(e) = self.sync_channel_joining(msg.channels) {
            error!("failed to join a channel: {}", e);
        }
    }
}

impl Handler<MessageCreated> for Irc {
    type Result = ();

    fn handle(&mut self, msg: MessageCreated, ctx: &mut Self::Context) -> Self::Result {
        let m = format!("<{}> {}", msg.nickname, msg.content);
        if let Err(e) = self.client.send(Command::PRIVMSG(msg.channel, m)) {
            error!("failed to send a message: {}", e);
        }
    }
}

impl Handler<Subscribe> for Irc {
    type Result = ();

    fn handle(&mut self, msg: Subscribe, _: &mut Self::Context) -> Self::Result {
        self.subscribers.add(msg.0);
    }
}

impl StreamHandler<IrcMessage, IrcError> for Irc {
    fn handle(&mut self, message: IrcMessage, _: &mut Context<Self>) {
        debug!("{}", message);
        let nickname = message.source_nickname()
            .map(String::from)
            .unwrap_or_else(String::new);
        match message.command {
            Command::Response(resp, _, _) => {
                match resp {
                    IrcResponse::RPL_WELCOME => {
                        if let Err(e) = self.sync_channel_joining(Vec::new()) {
                            error!("failed to join a channel: {}", e);
                        }
                    },
                    _ => { }
                }
            }
            Command::PRIVMSG(target, content) => {
                let m = MessageCreated {
                    nickname,
                    channel: target,
                    content,
                };
                self.subscribers.send(m);
            }
            // Command::PONG(_, _) => {
            //     if !pong_received {
            //         pong_received = true;
            //         debug!("woah");
            //     }
            // }
            _ => { }
        }
    }
}
