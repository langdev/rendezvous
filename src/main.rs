// async fn
#![feature(async_await, await_macro, futures_api, pin)]
// impl FnOnce for T
#![feature(arbitrary_self_types, fn_traits, unboxed_closures)]
// std::process::Termination
#![feature(termination_trait_lib)]

#![deny(rust_2018_idioms)]
#![deny(proc_macro_derive_resolution_fallback)]

#[macro_use]
mod macros;

mod bus;
mod config;
// mod discord_client;
mod error;
mod irc_client;
mod message;
mod util;

use actix::prelude::*;
use failure::Fail;
use futures::prelude::*;
use log::*;

pub use crate::{
    bus::{Bus, BusId},
    config::{Config, fetch_config},
    error::Error,
    util::{AddrExt, GetBusId},
};

use crate::{
    message::{ChannelUpdated, MessageCreated},
    util::task,
};


fn main() -> Result<(), failure::Error> {
    env_logger::init();

    let cfg = config::Config::from_path("dev.toml")?;
    config::update(cfg);

    let code = System::run(move || {
        let f = run().then(|res| {
            std::process::Termination::report(res);
            future::ready(())
        });

        task::spawn(f.boxed());

        // let discord = actix::SyncArbiter::start(3, move || {
        //     discord_client::Discord::new(&discord_bot_token).unwrap()
        // });

        // std::thread::spawn(move || {
        //     let mut id_map = HashMap::new();
        //     id_map.insert(irc_bus_id, "IRC".to_owned());
        //     id_map.insert(discord_bus_id, "Discord".to_owned());
        //     inspect_bus(bus, id_map);
        // });
    });
    std::process::exit(code);
}

async fn run() -> Result<(), failure::Error> {
    let _irc = irc_client::Irc::new()?.start();
    let _discord = discord_client::Discord::new()?.start();

    let inspector = Inspector { bus_id: Bus::new_id() }.start();
    let _ = await!(inspector.subscribe::<ChannelUpdated>());
    let _ = await!(inspector.subscribe::<MessageCreated>());

    Ok(())
}


struct Inspector {
    bus_id: BusId
}

impl actix::Actor for Inspector {
    type Context = actix::Context<Self>;
}

impl_get_bus_id!(Inspector);

impl actix::Handler<message::MessageCreated> for Inspector {
    type Result = ();

    fn handle(&mut self, msg: message::MessageCreated, _: &mut Self::Context) -> Self::Result {
        info!("{:#?}", msg);
    }
}

// fn inspect_bus(bus: message::Bus, id_map: HashMap<message::BusId, String>) {
//     for payload in bus {
//         use message::Message::*;
//         match payload.message {
//             ChannelUpdated { channels } => {
//                 info!("discord channels: {:?}", channels);
//             }
//             MessageCreated(msg) => {
//                 if let Some(name) = id_map.get(&payload.sender) {
//                     info!("from {} {} {}: {}", name, msg.channel, msg.nickname, msg.content);
//                 }
//             },
//             _ => { }
//         }
//     }
// }
