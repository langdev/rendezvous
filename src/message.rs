use actix::prelude::*;
use derive_builder::Builder;

#[derive(Clone, Debug)]
pub struct ChannelUpdated {
    pub channels: Vec<String>,
}

impl Message for ChannelUpdated {
    type Result = ();
}

#[derive(Builder, Clone, Debug)]
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

impl Message for MessageCreated {
    type Result = ();
}

pub struct Terminate;

impl Message for Terminate {
    type Result = ();
}
