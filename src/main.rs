#![feature(conservative_impl_trait, generators, proc_macro)]

#[macro_use] extern crate error_chain;
#[macro_use] extern crate log;

extern crate env_logger;
extern crate futures_await as futures;
extern crate futures_cpupool;
extern crate irc;
extern crate multiqueue;
extern crate parking_lot;
extern crate serenity;
extern crate tokio_core;
extern crate typemap;

#[cfg(test)] extern crate rand;

mod discord_client;
mod irc_client;
mod message;

use std::collections::HashMap;
use std::default::Default;
use std::env;
use std::process;

use futures::prelude::*;
use irc::client::data::Config;


error_chain! {
    links {
        Irc(irc::error::Error, irc::error::ErrorKind);
    }
    foreign_links {
        Discord(serenity::Error);
        EnvironmentVariable(std::env::VarError);
        Io(std::io::Error);
    }
    errors {
        UnexpectedlyPosioned {

        }
    }
}

fn main() {
    env_logger::init();

    if let Err(e) = run() {
        eprintln!("Fatal error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut core = tokio_core::reactor::Core::new()?;

    let bus = message::Bus::root();

    let cfg = Config {
        nickname: Some(format!("Rendezvous")),
        server: Some(format!("irc.ozinger.org")),
        use_ssl: Some(false),
        port: Some(6667),
        channels: Some(vec![]),
        umodes: Some("+Bx".to_owned()),
        .. Default::default()
    };
    let irc_bus = bus.add();
    let irc_bus_id = irc_bus.id;
    let irc_future = irc_client::Irc::from_config(core.handle(), irc_bus, &cfg)?;

    let discord_bot_token = env::var("DISCORD_BOT_TOKEN")?;

    let discord_bus = bus.add();
    let discord_bus_id = discord_bus.id;
    let discord = discord_client::Discord::new(discord_bus, &discord_bot_token)?;

    std::thread::spawn(move || {
        let mut id_map = HashMap::new();
        id_map.insert(irc_bus_id, "IRC".to_owned());
        id_map.insert(discord_bus_id, "Discord".to_owned());
        inspect_bus(bus, id_map);
    });

    core.run(irc_future.and_then(|_| futures::future::empty::<(), Error>()))?;

    Ok(())
}

fn inspect_bus(bus: message::Bus, id_map: HashMap<message::BusId, String>) {
    for payload in bus {
        use message::Message::*;
        match payload.message {
            ChannelUpdated { channels } => {
                info!("discord channels: {:?}", channels);
            }
            MessageCreated(msg) => {
                if let Some(name) = id_map.get(&payload.sender) {
                    info!("from {} {} {}: {}", name, msg.channel, msg.nickname, msg.content);
                }
            },
            _ => { }
        }
    }
}
