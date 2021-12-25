#![warn(clippy::all)]

mod channel;
mod guild;

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use rendezvous_common::{
    anyhow,
    futures::prelude::*,
    proto::{
        bouncer_service_client::BouncerServiceClient, event, ClientType, Event, Header,
        MessageCreated, UserRenamed,
    },
    tokio,
    tonic::transport,
    tracing::{self, debug, error, info, info_span, warn},
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing::init()?;

    let mut rpc_client = BouncerServiceClient::connect("http://[::1]:49252").await?;

    let token = std::env::var("RENDEZVOUS_DISCORD_BOT_TOKEN")?;

    let handler = Handler::new(rpc_client.clone());
    let channels = Arc::clone(&handler.channels);
    let mut discord_client = Client::builder(&token).event_handler(handler).await?;

    let http = Arc::clone(&discord_client.cache_and_http.http);

    let mut resp = rpc_client
        .subscribe(Header {
            client_type: ClientType::Discord.into(),
        })
        .await?;

    let handle_rpc_stream = async move {
        let stream = resp.get_mut();
        while let Some(m) = stream.try_next().await? {
            handle_ipc_event(&http, &channels, m).await?;
        }
        Ok::<_, anyhow::Error>(())
    };

    let handle_discord_events = discord_client
        .start_autosharded()
        .err_into::<anyhow::Error>();

    tokio::try_join!(handle_rpc_stream, handle_discord_events,)?;

    Ok(())
}

async fn handle_ipc_event(
    http: &Http,
    channels: &RwLock<ChannelList>,
    e: Event,
) -> anyhow::Result<()> {
    match e.body {
        Some(event::Body::MessageCreated(MessageCreated {
            nickname,
            channel,
            content,
            ..
        })) => {
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
                    .send_message(http, |m| m.content(format!("<{}> {}", nickname, content)))
                    .await?;
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
    rpc_client: BouncerServiceClient<transport::Channel>,
}

impl Handler {
    fn new(rpc_client: BouncerServiceClient<transport::Channel>) -> Self {
        Handler {
            guilds: Default::default(),
            channels: Default::default(),
            current_user: Default::default(),
            rpc_client,
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

#[async_trait]
impl EventHandler for Handler {
    async fn ready(
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

    async fn guild_create(&self, _ctx: Context, guild: Guild) {
        let mut new_channels = vec![];
        for chan in guild.channels.values() {
            if self.register_guild_channel(&chan) {
                new_channels.push(chan.name.clone());
            }
        }
        self.insert_guild(guild);
    }

    async fn guild_member_addition(&self, _ctx: Context, guild_id: GuildId, new_member: Member) {
        if let Some(g) = self.guilds.write().get_mut(&guild_id) {
            let id = new_member.user.id;
            g.members.insert(id, new_member.into());
        }
    }

    async fn guild_member_removal(
        &self,
        _ctx: Context,
        guild_id: GuildId,
        user: model::user::User,
    ) {
        if let Some(g) = self.guilds.write().get_mut(&guild_id) {
            g.members.remove(&user.id);
        }
    }

    async fn guild_member_update(&self, _ctx: Context, new: model::event::GuildMemberUpdateEvent) {
        let guild_id = new.guild_id;
        let new = UserData::from(new);
        let mut event = None;
        if let Some(m) = self
            .guilds
            .write()
            .get_mut(&guild_id)
            .and_then(|g| g.members.get_mut(&new.id))
        {
            if m.name != new.name {
                event = Some(Event {
                    header: Some(Header {
                        client_type: ClientType::Discord.into(),
                    }),
                    body: Some(event::Body::UserRenamed(UserRenamed {
                        old: m.name.clone(),
                        new: new.name.clone(),
                    })),
                });
            }
            *m = new;
        }
        if let Some(req) = event {
            if let Err(e) = self.rpc_client.clone().post(req).await {
                error!("failed to send event: {:?}", e);
            }
        }
    }

    async fn message(&self, _ctx: Context, new_message: Message) {
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

        let mut event = None;
        if let Some(ch) = self.channels.read().get_by_id(new_message.channel_id) {
            event = Some(Event {
                header: Some(Header {
                    client_type: ClientType::Discord.into(),
                }),
                body: Some(event::Body::MessageCreated(MessageCreated {
                    nickname: author_name(&self.guilds.read(), &new_message).to_owned(),
                    channel: format!("#{}", ch.name()),
                    content: new_message.content,
                    origin: "".to_owned(),
                })),
            });
        } else {
            info!("channel not found: {}", new_message.channel_id);
        }
        if let Some(req) = event {
            if let Err(e) = self.rpc_client.clone().post(req).await {
                error!("failed to send event: {:?}", e);
            }
        }
    }
}
