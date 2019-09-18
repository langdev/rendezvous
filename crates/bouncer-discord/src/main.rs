#![deny(rust_2018_idioms)]
#![deny(proc_macro_derive_resolution_fallback)]

mod channel;
mod guild;

use std::sync::{mpsc, Arc};
use std::thread;

use rendezvous_common::{
    data::*,
    ipc,
    parking_lot::RwLock,
    tracing::{self, debug, info, info_span, warn},
    Fallible,
};
use serenity::{
    http::Http,
    model::{
        self,
        channel::{ChannelType, GuildChannel, Message, MessageType},
        guild::{Guild, Member},
        id::GuildId,
    },
    prelude::*,
};

use crate::{
    channel::{Channel, ChannelList},
    guild::{author_name, GuildData, GuildMap, UserData},
};

fn main() -> Fallible<()> {
    tracing::init()?;

    let token = std::env::var("RENDEZVOUS_DISCORD_BOT_TOKEN")?;

    let (msg_tx, msg_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::sync_channel(8);

    let handler = Handler::new(event_tx);
    let channels = Arc::clone(&handler.channels);
    let mut client = Client::new(&token, handler)?;
    ipc::spawn_socket(msg_tx, event_rx)?;

    let http = Arc::clone(&client.cache_and_http.http);
    thread::spawn(move || {
        for m in msg_rx {
            handle_ipc_event(&http, &channels, m).unwrap();
        }
    });

    client.start_autosharded()?;

    Ok(())
}

fn handle_ipc_event(http: &Http, channels: &RwLock<ChannelList>, e: Event) -> Fallible<()> {
    match e {
        Event::MessageCreated {
            nickname,
            channel,
            content,
            ..
        } => {
            if !channel.starts_with("#") {
                return Ok(());
            }
            let channels = channels.read();
            debug!(
                "channels: {:?}",
                channels.iter().map(|ch| ch.name()).collect::<Vec<_>>()
            );
            if let Some(ch) = channels.get_by_name(&channel[1..]) {
                debug!("{:?}", ch);
                ch.id()
                    .send_message(http, |m| m.content(format!("<{}> {}", nickname, content)))?;
            } else {
                warn!("unknown channel: {:?}", channel);
            }
        }
        _ => {}
    }
    Ok(())
}

struct Handler {
    guilds: RwLock<GuildMap>,
    channels: Arc<RwLock<ChannelList>>,
    current_user: RwLock<Option<model::user::CurrentUser>>,
    event_tx: mpsc::SyncSender<Event>,
}

impl Handler {
    fn new(event_tx: mpsc::SyncSender<Event>) -> Self {
        Handler {
            guilds: Default::default(),
            channels: Default::default(),
            current_user: Default::default(),
            event_tx,
        }
    }

    fn insert_guild(&self, guild: Guild) -> Option<GuildData> {
        let mut lock = self.guilds.write();
        lock.insert(guild.id, guild.into())
    }

    fn register_guild_channel<'a>(&'a self, channel: &GuildChannel) -> bool {
        if channel.kind != ChannelType::Text {
            return false;
        }
        self.channels
            .write()
            .get_or_insert_with(channel.id, || Channel::Guild(channel.clone()));
        true
    }
}

impl EventHandler for Handler {
    fn ready(
        &self,
        _ctx: Context,
        model::gateway::Ready {
            user,
            guilds,
            private_channels,
            ..
        }: model::gateway::Ready,
    ) {
        use model::guild::GuildStatus::*;
        *self.current_user.write() = Some(user);
        self.guilds
            .write()
            .extend(guilds.into_iter().filter_map(|g| match g {
                OnlineGuild(g) => Some((g.id, g.into())),
                _ => None,
            }));
        self.channels.write().extend(
            private_channels
                .into_iter()
                .filter_map(|(_, ch)| Channel::from_discord(ch)),
        );
    }

    fn guild_create(&self, _ctx: Context, guild: Guild) {
        let mut new_channels = vec![];
        for channel in guild.channels.values() {
            let chan = channel.read();
            if self.register_guild_channel(&chan) {
                new_channels.push(chan.name.clone());
            }
        }
        self.insert_guild(guild);
    }

    fn guild_member_addition(&self, _ctx: Context, guild_id: GuildId, new_member: Member) {
        if let Some(g) = self.guilds.write().get_mut(&guild_id) {
            let id = new_member.user.read().id;
            g.members.insert(id, new_member.into());
        }
    }

    fn guild_member_removal(&self, _ctx: Context, guild_id: GuildId, user: model::user::User) {
        if let Some(g) = self.guilds.write().get_mut(&guild_id) {
            g.members.remove(&user.id);
        }
    }

    fn guild_member_update(&self, _ctx: Context, new: model::event::GuildMemberUpdateEvent) {
        let guild_id = new.guild_id;
        let new = UserData::from(new);
        if let Some(m) = self
            .guilds
            .write()
            .get_mut(&guild_id)
            .and_then(|g| g.members.get_mut(&new.id))
        {
            if m.name != new.name {
                let _ = self.event_tx.send(Event::UserRenamed {
                    old: m.name.clone(),
                    new: new.name.clone(),
                });
            }
            *m = new;
        }
    }

    fn message(&self, _ctx: Context, new_message: Message) {
        let span = info_span!("message");
        let _enter = span.enter();
        info!("entered");
        if new_message.kind != MessageType::Regular
            || self
                .current_user
                .read()
                .as_ref()
                .map(|u| u.id == new_message.author.id)
                .unwrap_or(false)
        {
            return;
        }

        if let Some(ch) = self.channels.read().get_by_id(new_message.channel_id) {
            self.event_tx
                .send(Event::MessageCreated {
                    nickname: author_name(&self.guilds.read(), &new_message).to_owned(),
                    channel: format!("#{}", ch.name()),
                    content: new_message.content,
                    origin: None,
                })
                .unwrap();
        } else {
            info!("channel not found: {}", new_message.channel_id);
        }
    }
}
