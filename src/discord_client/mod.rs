mod handler;
mod worker;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use actix::{
    self,
    actors::signal,
    fut,
    prelude::*,
};
use futures::{
    channel::mpsc,
    compat::*,
    prelude::*,
};
use log::*;
use serenity::model::{
    channel,
    prelude::*,
};

use crate::{
    AddrExt,
    Bus,
    BusId,
    Config,
    Error,
    fetch_config,
    message::{ChannelUpdated, IrcReady, MessageCreated, Terminate},
    task,
};

use self::handler::{ClientState, DiscordEvent, new_client};
use self::worker::{DiscordWorker, SendMessage};


pub struct Discord {
    config: Arc<Config>,
    bus_id: BusId,

    channels: HashMap<String, GuildChannel>,
    members: HashMap<(GuildId, UserId), Member>,

    client_state: Option<ClientState>,
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

    fn set_client_state(&mut self, state: ClientState) {
        if let Some(ClientState::Running { shard_manager, .. }) = self.client_state.take() {
            shard_manager.lock().shutdown_all();
        }
        self.client_state = Some(state);
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
        signal::ProcessSignals::from_registry()
            .do_send(signal::Subscribe(ctx.address().recipient()));

        let (tx, rx) = mpsc::channel(128);
        match new_client(&self.config, tx) {
            Ok(state) => {
                ctx.add_message_stream(rx.map(|e| Ok(e)).compat(TokioDefaultSpawner));
                self.set_client_state(state);
            }
            Err(e) => {
                error!("Connection failure: {}", e);
                ctx.notify(Terminate);
                return;
            }
        }

        let addr = ctx.address();
        async fn subscribe(addr: &Addr<Discord>) -> Result<(), MailboxError> {
            // await!(addr.subscribe::<ChannelUpdated>())?;
            await!(addr.subscribe::<IrcReady>())?;
            await!(addr.subscribe::<MessageCreated>())?;
            Ok(())
        }
        task::spawn(async move {
            if let Err(err) = await!(subscribe(&addr)) {
                error!("Failed to subscribe: {}", err);
                addr.do_send(Terminate);
            }
        }.boxed());
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        let old_state = self.client_state.take();
        let ret = match old_state {
            Some(ClientState::Running { term_rx, shard_manager, .. }) => {
                shard_manager.lock().shutdown_all();
                ctx.run_later(Duration::from_secs(2), |_, ctx| {
                    ctx.spawn(fut::wrap_future(term_rx.compat(TokioDefaultSpawner))
                        .then(|res, _, _| {
                            debug!("Discord client thread terminated: {:?}", res);
                            fut::ok(())
                        })
                        .timeout(Duration::from_secs(5), ())
                        .then(|_, actor: &mut Self, ctx: &mut Self::Context| {
                            actor.client_state = Some(ClientState::Stopped);
                            ctx.stop();
                            fut::ok(())
                        })
                    );
                });
                self.client_state = Some(ClientState::Stopping);
                return Running::Continue;
            }
            Some(ClientState::Stopping { .. }) => Running::Continue,
            _ => Running::Stop,
        };
        self.client_state = old_state;
        ret
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        Bus::do_publish(self.bus_id, Terminate);
    }
}

impl_get_bus_id!(Discord);

impl Handler<Terminate> for Discord {
    type Result = ();
    fn handle(&mut self, _: Terminate, ctx: &mut Self::Context) -> Self::Result {
        ctx.terminate();
    }
}

impl Handler<signal::Signal> for Discord {
    type Result = ();

    fn handle(&mut self, msg: signal::Signal, ctx: &mut Self::Context) {
        use self::signal::SignalType::*;
        match msg.0 {
            Int | Term | Quit => { ctx.stop(); }
            _ => { }
        }
    }
}

impl Handler<DiscordEvent> for Discord {
    type Result = ();

    fn handle(&mut self, msg: DiscordEvent, _: &mut Self::Context) {
        debug!("Discord receives DiscordEvent: {:?}", msg);
        use self::DiscordEvent::*;
        match msg {
            Ready { ready } => self.on_ready(ready),
            GuildCreate { guild, is_new } => self.on_guild_create(guild, is_new),
            GuildMemberAddition { guild_id, member } => self.on_guild_member_addition(guild_id, member),
            GuildMemberRemoval { guild_id, user } => self.on_guild_member_removal(guild_id, user),
            GuildMemberUpdate { member } => self.on_guild_member_update(member),
            Message { msg } => self.on_message(msg).unwrap(),
            _ => {
                info!("Unknown event: {:?}", msg);
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

impl Handler<IrcReady> for Discord {
    type Result = ();

    fn handle(&mut self, _: IrcReady, _: &mut Self::Context) {
        let channels = self.channels.keys().map(Clone::clone).collect();
        Bus::do_publish(self.bus_id, ChannelUpdated { channels });
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
