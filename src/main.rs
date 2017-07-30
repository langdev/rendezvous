#[macro_use] extern crate slog;

extern crate discord;
extern crate irc;
extern crate multiqueue;
extern crate slog_async;
extern crate slog_term;

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

type Error = Box<::std::error::Error>;
type Result<T> = ::std::result::Result<T, Error>;


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

    let discord_bot_token = env::var("DISCORD_BOT_TOKEN")?;

    let discord_bus = bus.add();
    let discord_bus_id = discord_bus.id;
    let discord = discord_client::Discord::new(log.new(o!()), discord_bus, &discord_bot_token)?;
    thread::sleep(Duration::from_secs(5));
    let channels = discord.channels();

    let cfg = Config {
        nickname: Some(format!("Rendezvous")),
        server: Some(format!("irc.ozinger.org")),
        use_ssl: Some(true),
        port: Some(6697),
        channels: Some(channels.into_iter().map(|ch| format!("#{}", ch.name)).collect()),
        umodes: Some("+Bx".to_owned()),
        .. Default::default()
    };
    let irc_bus = bus.add();
    let irc_bus_id = irc_bus.id;
    let irc = irc_client::Irc::from_config(log.new(o!()), irc_bus, cfg)?;

    for (id, msg) in bus.receiver {
        if id == discord_bus_id {
            info!(log, "from Discord {} {}: {}", msg.channel, msg.nickname, msg.content);
            irc.send(msg)?;
        } else if id == irc_bus_id {
            info!(log, "from IRC {} {}: {}", msg.channel, msg.nickname, msg.content);
            discord.send(msg)?;
        }
    }

    Ok(())
}
