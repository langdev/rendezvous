#![deny(rust_2018_idioms)]
#![deny(proc_macro_derive_resolution_fallback)]

use irc::client::{prelude::*, ClientStream};

use rendezvous_common::{
    anyhow,
    futures::prelude::*,
    proto::{
        bouncer_service_client::BouncerServiceClient, event, ClientType, Event, Header,
        MessageCreated,
    },
    // ipc,
    tokio,
    tonic::{transport::Channel, Response, Streaming},
    tracing::{self, info, instrument},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing::init()?;

    let mut irc_client = Client::new("config.toml").await?;
    irc_client.identify()?;
    info!("connected");

    let mut client = BouncerServiceClient::connect("http://[::1]:49252").await?;

    let resp = client
        .subscribe(Header {
            client_type: ClientType::Irc.into(),
        })
        .await?;

    tokio::try_join!(
        handle_irc_stream(irc_client.stream()?, client),
        handle_rpc_stream(resp, irc_client.sender()),
    )?;
    Ok(())
}

#[instrument]
async fn handle_irc_stream(
    mut irc_stream: ClientStream,
    mut client: BouncerServiceClient<Channel>,
) -> anyhow::Result<()> {
    while let Some(irc_msg) = irc_stream.try_next().await? {
        let nickname = irc_msg.source_nickname().unwrap_or("").into();
        match irc_msg.command {
            Command::PRIVMSG(channel, content) => {
                info!("privmsg");
                client
                    .post(Event {
                        header: Some(Header {
                            client_type: ClientType::Irc.into(),
                        }),
                        body: Some(event::Body::MessageCreated(MessageCreated {
                            nickname,
                            channel,
                            content,
                            origin: "".to_owned(),
                        })),
                    })
                    .await?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[instrument]
async fn handle_rpc_stream(
    mut resp: Response<Streaming<Event>>,
    sender: Sender,
) -> anyhow::Result<()> {
    let stream = resp.get_mut();
    while let Some(e) = stream.try_next().await? {
        match e.body {
            Some(event::Body::MessageCreated(MessageCreated {
                nickname,
                channel,
                content,
                ..
            })) => {
                sender.send_privmsg(&channel, &format!("<{}> {}", nickname, content))?;
            }
            _ => {}
        }
    }
    Ok(())
}
