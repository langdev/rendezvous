use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Header {
    pub ty: ClientType,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Irc,
    Discord,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    MessageCreated {
        nickname: String,
        channel: String,
        content: String,
        origin: Option<String>,
    },
    UserRenamed {
        old: String,
        new: String,
    },
}
