use std::sync::Arc;
use std::thread;

use log::*;
use futures::channel::{mpsc, oneshot};
use parking_lot::Mutex;
use serenity::{
    client::bridge::gateway::ShardManager,
    model::{
        channel,
        prelude::*,
    },
    prelude::*,
};
use threadpool::ThreadPool;
use typemap::Key;

use crate::{Config, Error};


pub(super) fn new_client<T>(
    config: Arc<Config>,
    f: impl FnOnce(mpsc::Receiver<DiscordEvent>) -> T,
) -> Result<ClientState<T>, Error> {
    let (term_tx, term_rx) = oneshot::channel();
    let (tx, rx) = mpsc::channel(128);
    let handler = DiscordHandler { tx };
    let mut client = serenity::Client::new(&config.discord.bot_token, handler)?;
    client.data.lock().insert::<Pool>(Mutex::new(client.threadpool.clone()));
    let shard_manager = client.shard_manager.clone();
    let thread_handle = thread::spawn(move || {
        let result = client.start();
        let _ = term_tx.send(result);
    });
    Ok(ClientState {
        thread_handle,
        term_rx,
        shard_manager,
        extra: f(rx),
    })
}

#[allow(dead_code)]
pub(super) struct ClientState<T> {
    pub(super) thread_handle: thread::JoinHandle<()>,
    pub(super) term_rx: oneshot::Receiver<Result<(), serenity::Error>>,
    pub(super) shard_manager: Arc<Mutex<ShardManager>>,
    pub(super) extra: T,
}


pub(super) struct Pool;
impl Key for Pool { type Value = Mutex<ThreadPool>; }


pub(super) struct DiscordHandler {
    tx: mpsc::Sender<DiscordEvent>,
}

impl DiscordHandler {
    fn sender(&self) -> mpsc::Sender<DiscordEvent> {
        self.tx.clone()
    }

    fn send(&self, event: DiscordEvent) {
        if let Err(e) = self.sender().try_send(event) {
            if e.is_disconnected() {

            }
        }
    }
}

macro_rules! impl_event_handler {
    (
        $(
            $handler_name:ident ( $($arg_name:tt: $arg_type:ty),* )
                => $variant_name:ident,
        )*
    ) => {
        impl_event! { []
            $(
                $variant_name($($arg_name: $arg_type;)*),
            )*
        }

        impl EventHandler for DiscordHandler {
            $(
                fn $handler_name(&self, _: Context, $($arg_name: $arg_type),*) {
                    let event = args! { DiscordEvent::$variant_name [] $($arg_name,)* };
                    debug!(concat!(stringify!($handler_name), ": {:?}"), &event);
                    self.send(event);
                }
            )*
        }
    };
}

macro_rules! impl_event {
    ( [ $( $vname:ident { $($arg_name:ident: $arg_type:ty,)* }, )* ] ) => {
        #[derive(Debug)]
        pub(super) enum DiscordEvent {
            $( $vname { $($arg_name: $arg_type),* }, )*
        }
    };

    (
        [ $($result:tt)* ]
        @ $vname:ident ( [ $($vresult:tt)* ] ),
        $($rest:tt)*
    ) => {
        impl_event! {
            [
                $($result)*
                $vname { $($vresult)* },
            ]
            $($rest)*
        }
    };

    (
        [ $($result:tt)* ]
        @ $vname:ident ( [ $($vresult:tt)* ] _: $type:ty; $($tokens:tt)* ),
        $($rest:tt)*
    ) => {
        impl_event! {
            [$($result)*]
            @ $vname([$($vresult)*] $($tokens)*),
            $($rest)*
        }
    };

    (
        [ $($result:tt)* ]
        @ $vname:ident ( [ $($vresult:tt)* ] $name:ident: $type:ty; $($tokens:tt)* ),
        $($rest:tt)*
    ) => {
        impl_event! {
            [$($result)*]
            @ $vname([$($vresult)* $name: $type,] $($tokens)*),
            $($rest)*
        }
    };

    (
        [ $($result:tt)* ]
        $vname:ident ( $($tokens:tt)* ),
        $($rest:tt)*
    ) => {
        impl_event! {
            [$($result)*]
            @ $vname([] $($tokens)*),
            $($rest)*
        }
    };
}

macro_rules! args {
    ( $($vname:ident)::* [ $($result:ident),* ] _, $($rest:tt)* ) => {
        args!($($vname)::* [$($result),*] $($rest)*)
    };
    ( $($vname:ident)::* [ $($result:ident),* ] $name:ident, $($rest:tt)* ) => {
        args!($($vname)::* [$($result,)* $name ] $($rest)*)
    };
    ( $($vname:ident)::* [ $($result:ident),* ] ) => {
        $($vname)::* { $($result),* }
    };
}

impl_event_handler! {
    ready(ready: Ready) => Ready,
    resume(_: ResumedEvent) => Resume,
    guild_create(guild: Guild, is_new: bool) => GuildCreate,
    guild_member_addition(guild_id: GuildId, member: Member) => GuildMemberAddition,
    guild_member_removal(guild_id: GuildId, user: User, _: Option<Member>) => GuildMemberRemoval,
    guild_member_update(_: Option<Member>, member: Member) => GuildMemberUpdate,
    channel_create(channel: Arc<RwLock<GuildChannel>>) => ChannelCreate,
    channel_delete(channel: Arc<RwLock<GuildChannel>>) => ChannelDelete,
    message(msg: channel::Message) => Message,
}

// impl EventHandler for DiscordHandler {
//     fn ready(&self, _: Context, ready: Ready) {
//         debug!("ready: {:?}", ready);
//         self.send(DiscordEvent::Ready(ready));
//         // if let Ok(chan) = channels(&lock) {
//         //     let s = self.subscribers.lock();
//         //     let chan = chan.into_iter().map(|ch| ch.name).collect();
//         //     s.send(Message::ChannelUpdated { channels: chan })
//         //         .unwrap_or_else(|e| warn!("error occured: {}", e));
//         // }
//     }

//     fn guild_update(
//         &self,
//         _: Context,
//         _: Option<Arc<RwLock<Guild>>>,
//         partial_guild: PartialGuild,
//     ) {
//         debug!("guild_update: {:?}", partial_guild);
//         // let mut lock = ctx.data.lock();
//         // let g = if let Some(g) = guild {
//         //     GuildStatus::OnlineGuild(g.read().clone())
//         // } else {
//         //     GuildStatus::OnlinePartialGuild(partial_guild)
//         // };
//         // lock.get_mut::<Guilds>().map(|m| m.insert(g.id(), g));
//         // if let Ok(chan) = channels(&lock) {
//         //     let s = self.subscribers.lock();
//         //     let chan = chan.into_iter().map(|ch| ch.name).collect();
//         //     s.send(Message::ChannelUpdated { channels: chan });
//         // }
//     }

//     fn message(&self, _: Context, msg: channel::Message) {
//         debug!("{:?}", msg);
//         self.send(DiscordEvent::Message(msg));
//     }
// }
