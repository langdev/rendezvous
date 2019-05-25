// async fn
#![feature(async_await, await_macro)]
// impl FnOnce for T
#![feature(arbitrary_self_types, fn_traits, unboxed_closures)]
// std::process::Termination
#![feature(termination_trait_lib)]

#![deny(rust_2018_idioms)]
#![deny(proc_macro_derive_resolution_fallback)]
#![recursion_limit = "128"]

#[macro_use]
mod macros;

mod bus;
mod config;
mod discord_client;
mod error;
mod irc_client;
mod message;
mod prelude;
mod util;

pub use crate::{
    bus::{Bus, BusId},
    config::{fetch_config, Config},
    error::Error,
    util::{AddrExt, GetBusId},
};

use crate::{
    message::{ChannelUpdated, MessageCreated},
    prelude::*,
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

        Arbiter::spawn_async(f.boxed());
    });
    std::process::exit(code);
}

async fn run() -> Result<(), failure::Error> {
    let _irc = irc_client::Irc::new()?.start();
    let _discord = discord_client::Discord::new()?.start();

    let inspector = Inspector {
        counter: 2,
        bus_id: Bus::new_id(),
    }
    .start();
    let _ = inspector.subscribe::<ChannelUpdated>().await;
    let _ = inspector.subscribe::<MessageCreated>().await;
    let _ = inspector.subscribe::<message::Terminate>().await;

    Ok(())
}

struct Inspector {
    counter: i32,
    bus_id: BusId,
}

impl actix::Actor for Inspector {
    type Context = actix::Context<Self>;
}

impl_get_bus_id!(Inspector);

impl actix::Handler<message::Terminate> for Inspector {
    type Result = ();

    fn handle(&mut self, _: message::Terminate, _: &mut Self::Context) -> Self::Result {
        self.counter -= 1;
        debug!("Inspector receives Terminate: counter = {}", self.counter);
        if self.counter <= 0 {
            System::current().stop();
        }
    }
}

impl actix::Handler<message::ChannelUpdated> for Inspector {
    type Result = ();

    fn handle(&mut self, msg: message::ChannelUpdated, _: &mut Self::Context) -> Self::Result {
        info!("discord channels: {:?}", msg.channels);
    }
}

impl actix::Handler<message::MessageCreated> for Inspector {
    type Result = ();

    fn handle(&mut self, msg: message::MessageCreated, _: &mut Self::Context) -> Self::Result {
        info!("{:#?}", msg);
    }
}
