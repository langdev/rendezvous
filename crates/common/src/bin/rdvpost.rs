use rendezvous_common::proto::{
    bouncer_service_client::BouncerServiceClient, event, ClientType, Event, Header, MessageCreated,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client = BouncerServiceClient::connect("http://[::1]:49252").await?;

    let req = Event {
        header: Some(Header {
            client_type: ClientType::Unknown.into(),
        }),
        body: Some(event::Body::MessageCreated(MessageCreated {
            nickname: "Rendezvous^DEV".to_owned(),
            channel: "#langdev-temp".to_owned(),
            content: "Hello, world!".to_owned(),
            origin: "".to_owned(),
        })),
    };

    client.post(req).await?;

    Ok(())
}
