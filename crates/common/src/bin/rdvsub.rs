use futures::prelude::*;

use rendezvous_common::proto::{bouncer_service_client::BouncerServiceClient, ClientType, Header};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client = BouncerServiceClient::connect("http://[::1]:49252").await?;

    let req = Header {
        client_type: ClientType::Unknown.into(),
    };

    let mut resp = client.subscribe(req).await?;

    let stream = resp.get_mut();

    while let Some(event) = stream.try_next().await? {
        println!("{:?}", event);
    }

    Ok(())
}
