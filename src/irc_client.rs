use std::collections::BTreeSet;
use std::sync::RwLock;

use actix::prelude::*;
use futures::{compat::*, prelude::*};
use irc::{
    client::{
        Client,
        IrcClient,
        data::Config as IrcConfig,
        ext::*,
    },
    error::IrcError,
    proto::{
        Command,
        Message as IrcMessage,
        Response as IrcResponse,
    },
};
use log::*;

use crate::{Error, fetch_config};
use crate::bus::{Bus, BusId, Subscribe};
use crate::message::{ChannelUpdated, MessageCreated};


pub struct Irc {
    client: IrcClient,
    channels: RwLock<BTreeSet<String>>,
    bus_id: Option<BusId>,
}

impl Irc {
    pub fn new() -> Result<Irc, Error> {
        let cfg = fetch_config();
        Irc::from_config(cfg.irc.clone())
    }

    pub fn from_config(cfg: IrcConfig) -> Result<Irc, Error> {
        let client = IrcClient::from_config(cfg)?;
        client.identify()?;
        Ok(Irc {
            client,
            channels: Default::default(),
            bus_id: None,
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
        let addr = ctx.address();
        let f = async move {
            if let Err(err) = await!(subscribe(addr)) {
                error!("Failed to subscribe: {}", err);
                addr.do_send(Terminate);
            }
        };
        Arbiter::spawn(f.boxed().unit_error().compat(TokioDefaultSpawn));
    }
}

async fn subscribe(addr: Addr<Irc>) -> Result<(), MailboxError> {
    let bus = Bus::from_registry();
    let recipient = addr.recipient::<MessageCreated>();
    let id = await!(bus.send(Subscribe::new(recipient)).compat())?;
    await!(addr.send(UpdateBusId(id)).compat())?;
    Ok(())
}

struct UpdateBusId(BusId);

impl actix::Message for UpdateBusId {
    type Result = ();
}

impl Handler<UpdateBusId> for Irc {
    type Result = ();
    fn handle(&mut self, msg: UpdateBusId, _: &mut Self::Context) -> Self::Result {
        self.bus_id = Some(msg.0);
    }
}

struct Terminate;

impl actix::Message for Terminate {
    type Result = ();
}

impl Handler<Terminate> for Irc {
    type Result = ();
    fn handle(&mut self, _: Terminate, ctx: &mut Self::Context) -> Self::Result {
        ctx.terminate();
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
                self.bus_id.map(|id| id.publish(m));
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
