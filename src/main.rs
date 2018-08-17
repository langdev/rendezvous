#![feature(async_await, await_macro, futures_api)]

// mod discord_client;
mod error;
mod irc_client;
mod message;

use std::default::Default;
use std::env;

use actix::prelude::*;
use failure::Fail;
use irc::client::data::Config;

pub use crate::error::Error;


fn main() -> Result<(), failure::Error> {
    env_logger::init();

    let discord_bot_token = env::var("DISCORD_BOT_TOKEN")?;

    let cfg = Config {
        nickname: Some(format!("Rendezvous")),
        server: Some(format!("irc.ozinger.org")),
        use_ssl: Some(false),
        port: Some(6667),
        channels: Some(vec![]),
        umodes: Some("+Bx".to_owned()),
        .. Default::default()
    };

    let code = System::run(move || {
        let irc = irc_client::Irc::from_config(cfg).unwrap().start();

        let discord = actix::SyncArbiter::start(3, move || {
            discord_client::Discord::new(&discord_bot_token).unwrap()
        });

        irc.send(message::Subscribe(discord.recipient()));
        discord.send(message::Subscribe(irc.recipient()));

        // std::thread::spawn(move || {
        //     let mut id_map = HashMap::new();
        //     id_map.insert(irc_bus_id, "IRC".to_owned());
        //     id_map.insert(discord_bus_id, "Discord".to_owned());
        //     inspect_bus(bus, id_map);
        // });
    });
    std::process::exit(code);
}

struct Inspector;

impl actix::Actor for Inspector {
    type Context = actix::Context<Self>;
}

impl actix::Handler<message::MessageCreated> for Inspector {
    type Result = ();

    fn handle(&mut self, msg: message::MessageCreated, ctx: &mut Self::Context) -> Self::Result {
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
