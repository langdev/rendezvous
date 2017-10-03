#[macro_use] extern crate error_chain;
#[macro_use] extern crate slog;

extern crate irc;
extern crate multiqueue;
extern crate serenity;
extern crate slog_async;
extern crate slog_term;
extern crate typemap;

#[cfg(test)] extern crate rand;

mod discord_client;
mod irc_client;
mod message;

use std::default::Default;
use std::env;
use std::process;
use std::thread;
use std::time::Duration;

use irc::client::data::Config;
use slog::*;


error_chain! {
    links {
        Irc(irc::error::Error, irc::error::ErrorKind);
    }
    foreign_links {
        Discord(serenity::Error);
        EnvironmentVariable(std::env::VarError);
    }
    errors {
        UnexpectedlyPosioned {

        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Fatal error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let deco = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(deco).build().fuse();
    let log = slog::Logger::root(
        slog_async::Async::new(drain).build().fuse(),
        o!()
    );

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
    irc_client::Irc::from_config(log.new(o!()), irc_bus, cfg)?;

    let discord_bot_token = env::var("DISCORD_BOT_TOKEN")?;

    let discord_bus = bus.add();
    let discord_bus_id = discord_bus.id;
    let discord = discord_client::Discord::new(log.new(o!()), discord_bus, &discord_bot_token)?;

    for payload in bus {
        use message::Message::*;
        match payload.message {
            ChannelUpdated { channels } => {
                info!(log, "discord channels: {:?}", channels);
            }
            MessageCreated(msg) => {
                if payload.sender == discord_bus_id {
                    info!(log, "from Discord {} {}: {}", msg.channel, msg.nickname, msg.content);
                } else if payload.sender == irc_bus_id {
                    info!(log, "from IRC {} {}: {}", msg.channel, msg.nickname, msg.content);
                }
            },
            _ => { }
        }
    }

    Ok(())
}
