use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use actix::{self, Actor, Addr, SyncContext};
use log::*;
use parking_lot::{Mutex, RwLock};
use serenity;
use serenity::{
    model::{
        channel,
        prelude::*,
    },
    prelude::*,
};
use typemap::{Key, ShareMap};

use crate::{
    Error,
    fetch_config,
    message::{Message, MessageCreated, Subscribe, SubscriptionList},
};


pub struct Discord {
    token: String,
    data: Option<Arc<Mutex<ShareMap>>>,
    subscribers: SubscriptionList<MessageCreated>,
}

struct User;

impl Key for User { type Value = CurrentUser; }

struct Guilds;

impl Key for Guilds { type Value = HashMap<GuildId, GuildStatus>; }

impl Discord {
    pub fn new() -> Result<Discord, Error> {
        let cfg = fetch_config();
        Ok(Discord {
            token: cfg.discord.bot_token.clone(),
            data: None,
        })
    }

    pub fn channels(&self) -> Result<Vec<GuildChannel>, Error> {
        let lock = self.data.lock();
        channels(&lock)
    }
}

struct Handler {
    addr: Addr<Discord>,
}

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        let mut lock = ctx.data.lock();
        debug!("ready: {:?}", ready);
        lock.insert::<User>(ready.user);
        lock.insert::<Guilds>(ready.guilds.into_iter().map(|g| (g.id(), g)).collect());
        // if let Ok(chan) = channels(&lock) {
        //     let s = self.subscribers.lock();
        //     let chan = chan.into_iter().map(|ch| ch.name).collect();
        //     s.send(Message::ChannelUpdated { channels: chan })
        //         .unwrap_or_else(|e| warn!("error occured: {}", e));
        // }
    }

    fn guild_update(
        &self,
        ctx: Context,
        guild: Option<Arc<RwLock<Guild>>>,
        partial_guild: PartialGuild,
    ) {
        debug!("guild_update: {:?}", partial_guild);
        let mut lock = ctx.data.lock();
        let g = if let Some(g) = guild {
            GuildStatus::OnlineGuild(g.read().clone())
        } else {
            GuildStatus::OnlinePartialGuild(partial_guild)
        };
        lock.get_mut::<Guilds>().map(|m| m.insert(g.id(), g));
        // if let Ok(chan) = channels(&lock) {
        //     let s = self.subscribers.lock();
        //     let chan = chan.into_iter().map(|ch| ch.name).collect();
        //     s.send(Message::ChannelUpdated { channels: chan });
        // }
    }

    fn message(&self, ctx: Context, msg: channel::Message) {
        debug!("{:?}", msg);
        if let Err(e) = on_message(&mut self.subscribers, ctx, msg) {
            error!("Error occured: {}", e);
        }
    }
}

fn on_message(
    sender: &mut SubscriptionList<MessageCreated>,
    ctx: serenity::client::Context,
    msg: channel::Message,
) -> Result<(), Error>
{
    if let Some(Channel::Guild(ch)) = msg.channel_id.find() {
        let data = ctx.data.lock();
        let c = ch.read();
        if let Some(u) = data.get::<User>() {
            if u.id == msg.author.id {
                return Ok(());
            }
        }
        let nickname = c.guild_id.member(msg.author.id)?.nick.unwrap_or(msg.author.name);
        let m = MessageCreated {
            nickname,
            channel: format!("#{}", c.name),
            content: msg.content,
        };
        sender.send(m);
    }
    Ok(())
}

impl Actor for Discord {
    type Context = SyncContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let handler = Handler {
            addr: ctx.addr(),
        };
        let mut client = serenity::Client::new(token, handler);
    }
}

impl actix::Handler<MessageCreated> for Discord {
    type Result = ();

    fn handle(&mut self, msg: MessageCreated, ctx: &mut Self::Context) {
        if !msg.channel.starts_with('#') {
            return;
        }
        let data = self.data.lock();
        if let Some(channel) = find_channel(&data, &msg.channel[1..]) {
            let m = format!("<{}> {}", msg.nickname, msg.content);
            while let Err(e) = channel.send_message(|msg| msg.content(&m)) {
                use serenity::Error::*;
                match e {
                    Http(..) | Hyper(..) | WebSocket(..) => {
                        thread::sleep(Duration::from_millis(100));
                    }
                    _ => {
                        error!("failed to send a message: {}", e);
                        break;
                    }
                }
            }
        }
    }
}

impl actix::Handler<Subscribe> for Discord {
    type Result = ();

    fn handle(&mut self, msg: Subscribe, ctx: &mut Self::Context) -> Self::Result {
        let mut subscribers = self.subscribers.lock();
        subscribers.add(msg.0);
    }
}


fn find_channel(data: &ShareMap, channel: &str) -> Option<GuildChannel> {
    for ch in channels(data).unwrap_or(vec![]) {  // 땜빵
        if ch.name == channel {
            return Some(ch);
        }
    }
    None
}

fn channels(data: &ShareMap) -> Result<Vec<GuildChannel>, Error> {
    if let Some(guilds) = data.get::<Guilds>() {
        let mut result = vec![];
        for g in guilds.values() {
            let channels = match *g {
                GuildStatus::Offline(ref g) => g.id.channels()?,
                GuildStatus::OnlineGuild(ref g) => g.channels()?,
                GuildStatus::OnlinePartialGuild(ref g) => g.channels()?,
            };
            let channels = channels.into_iter()
                .map(|(_, v)| v)
                .filter(|ch| ch.kind == ChannelType::Text);
            result.extend(channels);
        }
        Ok(result)
    } else {
        Ok(vec![])
    }
}
