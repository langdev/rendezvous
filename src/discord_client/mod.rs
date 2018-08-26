mod handler;
mod worker;

use std::collections::HashMap;
use std::sync::Arc;

use actix::{
    self,
    prelude::*,
};
use futures::{compat::*, prelude::*};
use log::*;
use serenity::model::{
    channel,
    prelude::*,
};

use crate::{
    Bus,
    BusId,
    Config,
    Error,
    fetch_config,
    message::{ChannelUpdated, MessageCreated, Terminate},
};

use self::handler::{ClientState, DiscordEvent, new_client};
use self::worker::{DiscordWorker, SendMessage};


pub struct Discord {
    config: Arc<Config>,
    bus_id: BusId,

    channels: HashMap<String, GuildChannel>,
    members: HashMap<(GuildId, UserId), Member>,

    client_state: Option<ClientState<SpawnHandle>>,
    current_user: Option<CurrentUser>,
    worker: Addr<DiscordWorker>,
}

impl Discord {
    pub fn new() -> Result<Discord, Error> {
        let config = fetch_config();
        let worker = SyncArbiter::start(8, || DiscordWorker::new());

        Ok(Discord {
            config,
            bus_id: Bus::new_id(),
            channels: HashMap::new(),
            members: HashMap::new(),
            client_state: None,
            current_user: None,
            worker,
        })
    }

    fn set_client_state(
        &mut self,
        ctx: &mut <Self as Actor>::Context,
        state: Option<ClientState<SpawnHandle>>,
    ) {
        if let Some(old_state) = self.client_state.take() {
            old_state.shard_manager.lock().shutdown_all();
            ctx.cancel_future(old_state.extra);
            drop(old_state);
        }
        self.client_state = state;
    }

    fn find_channel(&self, channel: &str) -> Option<&GuildChannel> {
        self.channels.get(channel)
    }

    fn find_channel_by_id(&self, id: ChannelId) -> Option<(&str, &GuildChannel)> {
        self.channels.iter()
            .find(|(_, channel)| channel.id == id)
            .map(|(name, channel)| (&name[..], channel))
    }
}

impl Actor for Discord {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let state = new_client(Arc::clone(&self.config), |rx| {
            ctx.add_stream(rx.map(|e| Ok(e)).compat(TokioDefaultSpawn))
        }).unwrap();
        self.set_client_state(ctx, Some(state));
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        self.set_client_state(ctx, None);
    }
}

impl_get_bus_id!(Discord);

impl Handler<Terminate> for Discord {
    type Result = ();
    fn handle(&mut self, _: Terminate, ctx: &mut Self::Context) -> Self::Result {
        ctx.terminate();
    }
}

impl StreamHandler<DiscordEvent, ()> for Discord {
    fn handle(&mut self, item: DiscordEvent, _: &mut Self::Context) {
        use self::DiscordEvent::*;
        match item {
            Ready { ready } => self.on_ready(ready),
            GuildCreate { guild, is_new } => self.on_guild_create(guild, is_new),
            GuildMemberAddition { guild_id, member } => self.on_guild_member_addition(guild_id, member),
            GuildMemberRemoval { guild_id, user } => self.on_guild_member_removal(guild_id, user),
            GuildMemberUpdate { member } => self.on_guild_member_update(member),
            Message { msg } => self.on_message(msg).unwrap(),
            _ => {
                debug!("Unknown event: {:#?}", item);
            }
        }
    }
}

impl Discord {
    fn on_ready(&mut self, Ready { user, .. }: Ready) {
        self.current_user = Some(user);
    }

    fn on_guild_create(&mut self, guild: Guild, _is_new: bool) {
        let mut new_channels = vec![];
        for channel in guild.channels.values() {
            let chan = channel.read();
            if chan.kind != ChannelType::Text {
                continue;
            }
            self.channels.entry(chan.name.clone()).or_insert_with(|| {
                new_channels.push(chan.name.clone());
                chan.clone()
            });
        }
        if !new_channels.is_empty() {
            Bus::publish(self.bus_id, ChannelUpdated {
                channels: new_channels,
            });
        }

        for (id, member) in &guild.members {
            self.members.insert((guild.id, *id), member.clone());
        }
    }

    fn on_guild_member_addition(&mut self, guild_id: GuildId, member: Member) {
        let user_id = member.user.read().id;
        self.members.insert((guild_id, user_id), member);
    }

    fn on_guild_member_removal(&mut self, guild_id: GuildId, user: User) {
        self.members.remove(&(guild_id, user.id));
    }

    fn on_guild_member_update(&mut self, member: Member) {
        self.on_guild_member_addition(member.guild_id, member)
    }

    fn on_message(
        &mut self,
        msg: channel::Message,
    ) -> Result<(), Error> {
        if let Some((name, channel)) = self.find_channel_by_id(msg.channel_id) {
            if let Some(u) = &self.current_user {
                if u.id == msg.author.id {
                    return Ok(());
                }
            }
            // let nickname = channel.guild_id.member(msg.author.id)?.nick.unwrap_or(msg.author.name);
            let nickname = self.members.get(&(channel.guild_id, msg.author.id))
                .and_then(|m| m.nick.as_ref())
                .unwrap_or(&msg.author.name);
            let m = MessageCreated::builder()
                .nickname(&nickname[..])
                .channel(format!("#{}", name))
                .content(msg.content)
                .build().unwrap();
            Bus::do_publish(self.bus_id, m);
        }
        Ok(())
    }
}

impl Handler<MessageCreated> for Discord {
    type Result = ();

    fn handle(&mut self, msg: MessageCreated, _: &mut Self::Context) {
        if !msg.channel.starts_with('#') {
            return;
        }
        if let Some(channel) = self.find_channel(&msg.channel[1..]) {
            let m = format!("<{}> {}", msg.nickname, msg.content);
            self.worker.do_send(SendMessage { channel: channel.id, content: m });
        }
    }
}
