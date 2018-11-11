use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ::actix::actors::signal;
use ::irc::{
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
use ::regex::Regex;

use crate::{
    AddrExt,
    Config,
    Error,
    fetch_config,
    bus::{Bus, BusId},
    message::{ChannelUpdated, IrcReady, MessageCreated, Terminate},
    prelude::*,
};


pub struct Irc {
    config: Arc<Config>,

    client: IrcClient,
    connected: bool,
    channels_to_join: Vec<String>,
    bus_id: BusId,
    last_pong: Instant,
    refresh_handle: Option<SpawnHandle>,
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
            connected: false,
            channels_to_join: Default::default(),
            bus_id: Bus::new_id(),
            last_pong: Instant::now(),
            refresh_handle: None,
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
                info!("Connected");
                Bus::do_publish(self.bus_id, IrcReady);
                self.connected = true;
                self.sync_channel_joining()?;
            },
            _ => { }
        }
        Ok(())
    }

    fn sync_channel_joining(&mut self) -> Result<(), Error> {
        if self.channels_to_join.is_empty() {
            return Ok(());
        }
        if !self.connected {
            return Ok(());
        }
        debug!("sync_channel_joining: channels = {:?}", self.channels_to_join);
        let chanlist = self.channels_to_join.drain(..)
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
        signal::ProcessSignals::from_registry()
            .do_send(signal::Subscribe(ctx.address().recipient()));

        ctx.add_stream(self.client.stream());
        let addr = ctx.address();
        async fn subscribe(addr: &Addr<Irc>) -> Result<(), MailboxError> {
            await!(addr.subscribe::<ChannelUpdated>())?;
            await!(addr.subscribe::<MessageCreated>())?;
            Ok(())
        }
        Arbiter::spawn_async(async move {
            if let Err(err) = await!(subscribe(&addr)) {
                error!("Failed to subscribe: {}", err);
                addr.do_send(Terminate);
            }
        }.boxed());
        ctx.run_interval(Duration::from_secs(5), |this, _ctx| {
            let _ = this.sync_channel_joining();
        });
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        self.refresh_handle.take().map(|h| ctx.cancel_future(h));
        let _ = self.client.send_quit("");
        Bus::do_publish(self.bus_id, Terminate);
    }
}

impl_get_bus_id!(Irc);

impl Handler<signal::Signal> for Irc {
    type Result = ();

    fn handle(&mut self, msg: signal::Signal, ctx: &mut Self::Context) {
        use self::signal::SignalType::*;
        match msg.0 {
            Int | Term | Quit => { ctx.stop(); }
            _ => { }
        }
    }
}

impl Handler<Terminate> for Irc {
    type Result = ();
    fn handle(&mut self, _: Terminate, ctx: &mut Self::Context) -> Self::Result {
        ctx.terminate();
    }
}

impl Handler<ChannelUpdated> for Irc {
    type Result = ();
    fn handle(&mut self, msg: ChannelUpdated, _: &mut Self::Context) -> Self::Result {
        debug!("Irc receives ChannelUpdated: channels = {:?}", msg.channels);
        self.channels_to_join.extend(msg.channels);
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
        // debug!("{}", message);
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
