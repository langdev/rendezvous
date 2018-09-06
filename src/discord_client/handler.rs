use std::sync::Arc;
use std::thread;

use actix::Message;
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


pub(super) fn new_client(
    config: &Config,
    tx: mpsc::Sender<DiscordEvent>,
) -> Result<ClientState, Error> {
    let (term_tx, term_rx) = oneshot::channel();
    let handler = DiscordHandler { tx };
    let mut client = serenity::Client::new(&config.discord.bot_token, handler)?;
    client.data.lock().insert::<Pool>(Mutex::new(client.threadpool.clone()));
    let shard_manager = client.shard_manager.clone();
    thread::spawn(move || {
        let result = client.start();
        client.threadpool.join();
        let _ = term_tx.send(result);
    });
    Ok(ClientState::Running {
        term_rx,
        shard_manager,
    })
}

pub(super) enum ClientState {
    Running {
        term_rx: oneshot::Receiver<Result<(), serenity::Error>>,
        shard_manager: Arc<Mutex<ShardManager>>,
    },
    Stopping,
    Stopped,
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

    fn send(&self, context: &Context, event: DiscordEvent) {
        if let Err(e) = self.sender().try_send(event) {
            if e.is_disconnected() {
                context.quit();
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
                fn $handler_name(&self, context: Context, $($arg_name: $arg_type),*) {
                    let event = args! { DiscordEvent::$variant_name [] $($arg_name,)* };
                    // debug!(concat!(stringify!($handler_name), ": {:?}"), &event);
                    self.send(&context, event);
                }
            )*
        }
    };
}

macro_rules! impl_event {
    ( [ $( $vname:ident { $($arg_name:ident: $arg_type:ty,)* }, )* ] ) => {
        #[derive(Debug, Message)]
        #[rtype(result = "()")]
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
    guild_create(guild: Guild) => GuildCreate,
    guild_member_addition(guild_id: GuildId, member: Member) => GuildMemberAddition,
    guild_member_removal(guild_id: GuildId, user: User) => GuildMemberRemoval,
    guild_member_update(event: GuildMemberUpdateEvent) => GuildMemberUpdate,
    channel_create(channel: Arc<RwLock<GuildChannel>>) => ChannelCreate,
    channel_delete(channel: Arc<RwLock<GuildChannel>>) => ChannelDelete,
    message(msg: channel::Message) => Message,
}
