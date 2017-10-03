use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serenity;
use serenity::model::{Channel, CurrentUser, GuildChannel, GuildId, GuildStatus};
use slog;
use typemap::{Key, ShareMap};

use ::{Result};
use message::{Bus, BusSender, Message, MessageCreated, Payload};


pub struct Discord {
    data: Arc<Mutex<ShareMap>>,
}

struct User;

impl Key for User { type Value = CurrentUser; }

struct Guilds;

impl Key for Guilds { type Value = HashMap<GuildId, GuildStatus>; }

impl Discord {
    pub fn new<L>(logger: L, bus: Bus, token: &str) -> Result<Discord>
        where L: Into<Option<slog::Logger>>
    {
        let logger = logger.into().unwrap_or_else(|| slog::Logger::root(slog::Discard, o!()));
        let client = serenity::Client::new(token);
        let data = client.data.clone();

        spawn_listener(client, bus.sender(), logger.new(o!()));
        spawn_actor(logger, data.clone(), bus);
        Ok(Discord {
            data,
        })
    }

    pub fn channels(&self) -> Result<Vec<GuildChannel>> {
        let lock = self.data.lock().expect("unexpected poisoned lock");
        channels(&lock)
    }
}

fn spawn_listener(
    mut client: serenity::Client,
    sender: BusSender,
    log: slog::Logger,
) {
    let l2 = log.clone();
    let l3 = log.clone();
    let sender = Arc::new(Mutex::new(sender));
    let sender3 = sender.clone();
    client.on_ready(move |ctx, ready| {
        let mut lock = ctx.data.lock().expect("unexpected poisoned lock");
        debug!(l2, "ready: {:?}", ready);
        lock.insert::<User>(ready.user);
        lock.insert::<Guilds>(ready.guilds.into_iter().map(|g| (g.id(), g)).collect());
        if let Ok(chan) = channels(&lock) {
            let s = sender3.lock().unwrap();
            let chan = chan.into_iter().map(|ch| ch.name).collect();
            s.try_send(Message::ChannelUpdated { channels: chan });
        }
    });
    let sender2 = sender.clone();  // it sucks
    client.on_guild_update(move |ctx, guild, partial_guild| {
        debug!(l3, "guild_update: {:?}", partial_guild);
        let mut lock = ctx.data.lock().expect("unexpected poisoned lock");
        let g = if let Some(g) = guild {
            GuildStatus::OnlineGuild(g.read().expect("unexpected poisoned lock").clone())
        } else {
            GuildStatus::OnlinePartialGuild(partial_guild)
        };
        lock.get_mut::<Guilds>().map(|m| m.insert(g.id(), g));
        if let Ok(chan) = channels(&lock) {
            let s = sender2.lock().unwrap();
            let chan = chan.into_iter().map(|ch| ch.name).collect();
            s.try_send(Message::ChannelUpdated { channels: chan });
        }
    });
    client.on_message(move |ctx, msg| {
        debug!(log, "{:?}", msg);
        let s = sender.lock().unwrap();
        if let Err(e) = on_message(&s, ctx, msg) {
            error!(log, "Error occured: {}", e);
        }
    });
    thread::spawn(move || {
        client.start().unwrap();
    });
}

fn on_message(sender: &BusSender, ctx: serenity::client::Context, msg: serenity::model::Message) -> Result<()> {
    if let Some(Channel::Guild(ch)) = msg.channel_id.find() {
        let data = ctx.data.lock().expect("unexpected poisoned lock");
        let c = ch.read().expect("unexpected poisoned lock");
        if let Some(u) = data.get::<User>() {
            if u.id == msg.author.id {
                return Ok(());
            }
        }
        let nickname = c.guild_id.member(msg.author.id)?.nick.unwrap_or(msg.author.name);
        let mut m = Message::MessageCreated(MessageCreated {
            nickname,
            channel: format!("#{}", c.name),
            content: msg.content,
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
    Ok(())
}

fn spawn_actor(
    logger: slog::Logger,
    data: Arc<Mutex<ShareMap>>,
    bus: Bus,
) {
    use message::Message::*;
    thread::spawn(move || {
        for Payload { message, .. } in bus {
            match message {
                MessageCreated(msg) => {
                    if !msg.channel.starts_with('#') {
                        continue;
                    }
                    let data = data.lock().expect("unexpected poisoned lock");
                    if let Some(channel) = find_channel(&data, &msg.channel[1..]) {
                        let m = format!("<{}> {}", msg.nickname, msg.content);
                        while let Err(e) = channel.send_message(|msg| msg.content(&m)) {
                            use serenity::Error::*;
                            match e {
                                Http(..) | Hyper(..) | WebSocket(..) => {
                                    thread::sleep(Duration::from_millis(100));
                                }
                                _ => {
                                    error!(logger, "failed to send a message: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
                _ => { }
            }
        }
    });
}

fn find_channel(data: &ShareMap, channel: &str) -> Option<GuildChannel> {
    for ch in channels(data).unwrap_or(vec![]) {  // 땜빵
        if ch.name == channel {
            return Some(ch);
        }
    }
    None
}

fn channels(data: &ShareMap) -> Result<Vec<GuildChannel>> {
    if let Some(guilds) = data.get::<Guilds>() {
        let mut result = vec![];
        for g in guilds.values() {
            let channels = match *g {
                GuildStatus::Offline(ref g) => g.id.channels()?,
                GuildStatus::OnlineGuild(ref g) => g.channels()?,
                GuildStatus::OnlinePartialGuild(ref g) => g.channels()?,
            };
            result.extend(channels.into_iter().map(|(_, v)| v));
        }
        Ok(result)
    } else {
        Ok(vec![])
    }
}
