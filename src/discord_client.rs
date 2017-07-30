use std::sync::{Arc, RwLock};
use std::thread;

use discord;
use discord::{ChannelRef, State};
use discord::model::{ChannelId, Event, Member, PublicChannel, UserId};
use slog;

use ::{Result};
use message::{Bus, BusSender, Message};


pub struct Discord {
    client: discord::Discord,
    state: Arc<RwLock<State>>,
}

impl Discord {
    pub fn new<L>(logger: L, bus: Bus, token: &str) -> Result<Discord>
        where L: Into<Option<slog::Logger>>
    {
        let logger = logger.into().unwrap_or_else(|| slog::Logger::root(slog::Discard, o!()));
        let client = discord::Discord::from_bot_token(token)?;

        let (conn, ready) = client.connect()?;
        let state = Arc::new(RwLock::new(State::new(ready)));
        spawn_listener(conn, bus.sender(), state.clone(), logger.new(o!()));
        Ok(Discord {
            client,
            state,
        })
    }

    pub fn send(&self, message: Message) -> Result<()> {
        if message.channel.starts_with('#') {
            if let Some(channel) = self.find_channel(&message.channel[1..]) {
                let m = format!("<{}> {}", message.nickname, message.content);
                self.client.send_message(channel, &m, "", false)?;
            }
            Ok(())
        } else {
            Ok(())
        }
    }

    fn find_channel(&self, channel: &str) -> Option<ChannelId> {
        let s = self.state.read().expect("unexpected poisoned lock");
        for srv in s.servers() {
            for ch in &srv.channels {
                if ch.name == channel {
                    return Some(ch.id);
                }
            }
        }
        None
    }

    pub fn channels(&self) -> Vec<PublicChannel> {
        let s = self.state.read().expect("unexpected poisoned lock");
        let mut channels = vec![];
        for srv in s.servers() {
            channels.extend_from_slice(&srv.channels);
        }
        channels
    }
}

fn spawn_listener(
    mut conn: discord::Connection,
    sender: BusSender,
    st: Arc<RwLock<State>>,
    log: slog::Logger,
) {
    thread::spawn(move || {
        loop {
            let ev = conn.recv_event().unwrap();
            {
                let mut s = st.write().expect("unexpected poisoned lock");
                s.update(&ev);
            }
            debug!(log, "{:?}", ev);
            match ev {
                Event::MessageCreate(m) => {
                    let st = st.read().expect("unexpected poisoned lock");
                    if st.user().id == m.author.id {
                        continue;
                    }
                    if let Some(ChannelRef::Public(s, c)) = st.find_channel(m.channel_id) {
                        let mut nickname = m.author.name;
                        if let Some(nick) = find_nickname(&s.members, m.author.id) {
                            nickname = nick.to_owned();
                        }
                        let mut m = Message {
                            nickname,
                            channel: format!("#{}", c.name),
                            content: m.content,
                        };
                        while let Err(e) = sender.try_send(m) {
                            use std::sync::mpsc::TrySendError::*;
                            m = match e {
                                Full(p) => p,
                                Disconnected(_) => { panic!("bus closed"); }
                            };
                            thread::yield_now();
                        }
                    }
                }
                _ => { }
            }
        }
    });
}

fn find_nickname(members: &[Member], id: UserId) -> Option<&str> {
    members.iter()
        .find(|m| m.user.id == id)
        .and_then(|m| m.nick.as_ref().map(AsRef::as_ref))
}
