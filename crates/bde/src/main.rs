#![warn(clippy::all)]

mod data;
mod gateway;

use async_std::println;
use async_tungstenite::async_std::connect_async_with_tls_connector;

use rendezvous_common::{anyhow, nng};

use crate::gateway::State;

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let token = std::env::var("DISCORD_BOT_TOKEN")?;

    let socket = nng::Socket::new(nng::Protocol::Pub0)?;
    socket.listen("ipc:///var/tmp/rendezvous.bnc.event.discord.pipe")?;

    let base_url = "wss://gateway.discord.gg";
    let (stream, _resp) =
        connect_async_with_tls_connector(format!("{}/?v=6&encoding=json", base_url), None).await?;
    println!("handshake completed").await;

    let state = State::establish(stream, &token, socket).await?;

    state.run().await?;
    Ok(())
}
