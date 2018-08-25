use actix::prelude::*;
use derive_builder::Builder;

#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]
pub struct ChannelUpdated {
    pub channels: Vec<String>,
}

#[derive(Builder, Clone, Debug, Message)]
#[rtype(result = "()")]
#[builder(setter(into))]
pub struct MessageCreated {
    pub nickname: String,
    pub channel: String,
    pub content: String,
    #[builder(default)]
    pub origin: Option<String>,
}

impl MessageCreated {
    pub fn builder() -> MessageCreatedBuilder { Default::default() }
}

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct Terminate;
