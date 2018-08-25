use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use actix::prelude::*;
use futures::{compat::*, prelude::*};
use irc::{
    client::{
        Client,
        IrcClient,
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
use regex::Regex;

use crate::{AddrExt, Config, Error, fetch_config};
use crate::bus::{Bus, BusId};
use crate::message::{ChannelUpdated, MessageCreated, Terminate};


pub struct Irc {
    config: Arc<Config>,

    client: IrcClient,
    channels: HashSet<String>,
    bus_id: BusId,
    last_pong: Instant,
}

impl Irc {
    pub fn new() -> Result<Irc, Error> {
        let cfg = fetch_config();
        Irc::from_config(cfg)
    }

    pub fn from_config(config: Arc<Config>) -> Result<Irc, Error> {
        let client = IrcClient::from_config(config.irc.clone())?;
        client.identify()?;
        Ok(Irc {
            config,
            client,
            channels: Default::default(),
            bus_id: Bus::new_id(),
            last_pong: Instant::now(),
        })
    }

    pub fn bots(&self) -> &HashMap<String, Regex> {
        &self.config.bots
    }

    fn handle_response(
        &mut self,
        resp: &IrcResponse,
        _args: &[impl AsRef<str>],
        _suffix: Option<&str>,
    ) -> Result<(), Error> {
        match resp {
            IrcResponse::RPL_WELCOME => {
                self.channels.insert("langdev-temp".to_owned());
                self.sync_channel_joining()?;
            },
            _ => { }
        }
        Ok(())
    }

    fn sync_channel_joining(&mut self) -> Result<(), Error> {
        if self.channels.is_empty() {
            return Ok(());
        }
        let chanlist = self.channels.iter()
            .map(|n| format!("#{}", n))
            .collect::<Vec<_>>()
            .join(",");
        self.client.send_join(&chanlist)?;
        Ok(())
    }

    fn bridged<'m>(&self, nickname: &'m str, content: &'m str) -> Message<'m> {
        if let Some(pat) = self.bots().get(nickname) {
            if let Some(m) = pat.captures(content) {
                if let Some(name) = m.get(1) {
                    return Message {
                        nickname: name.as_str(),
                        content: m.get(2).map(|i| i.as_str()).unwrap_or(content),
                        origin: Some(nickname),
                    };
                }
            }
        }
        Message { nickname, content, origin: None }
    }
}

struct Message<'a> {
    nickname: &'a str,
    content: &'a str,
    origin: Option<&'a str>,
}

impl Actor for Irc {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.add_stream(self.client.stream());
        let addr = ctx.address();
        let f = async move {
            if let Err(err) = await!(addr.subscribe::<MessageCreated>()) {
                error!("Failed to subscribe: {}", err);
                addr.do_send(Terminate);
            }
        };
        Arbiter::spawn(f.boxed().unit_error().compat(TokioDefaultSpawn));
    }
}

impl_get_bus_id!(Irc);

impl Handler<Terminate> for Irc {
    type Result = ();
    fn handle(&mut self, _: Terminate, ctx: &mut Self::Context) -> Self::Result {
        ctx.terminate();
    }
}

impl Handler<ChannelUpdated> for Irc {
    type Result = ();
    fn handle(&mut self, msg: ChannelUpdated, _: &mut Self::Context) -> Self::Result {
        self.channels.extend(msg.channels);
        if let Err(e) = self.sync_channel_joining() {
            error!("failed to join a channel: {}", e);
        }
    }
}

impl Handler<MessageCreated> for Irc {
    type Result = ();

    fn handle(&mut self, msg: MessageCreated, _: &mut Self::Context) -> Self::Result {
        let m = format!("<{}> {}", msg.nickname, msg.content);
        if let Err(e) = self.client.send(Command::PRIVMSG(msg.channel, m)) {
            error!("failed to send a message: {}", e);
        }
    }
}

impl StreamHandler<IrcMessage, IrcError> for Irc {
    fn handle(&mut self, message: IrcMessage, _: &mut Context<Self>) {
        debug!("{}", message);
        match &message.command {
            Command::Response(resp, args, suffix) => {
                let suffix = suffix.as_ref().map(|s| s.as_ref());
                if let Err(e) = self.handle_response(resp, args, suffix) {
                    error!("failed to join a channel: {}", e);
                }
            }
            Command::PRIVMSG(target, content) => {
                let nickname = message.source_nickname().unwrap_or("");
                let Message {
                    nickname,
                    content,
                    origin,
                } = self.bridged(nickname, content);
                let m = MessageCreated::builder()
                    .nickname(nickname)
                    .channel(&target[..])
                    .content(content)
                    .origin(origin.map(From::from))
                    .build().unwrap();

                Bus::do_publish(self.bus_id, m);
            }
            Command::PONG(_, _) => {
                self.last_pong = Instant::now();
            }
            _ => { }
        }
    }
}
