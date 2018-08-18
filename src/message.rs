use actix::{self, prelude::*};

pub struct ChannelUpdated {
    pub channels: Vec<String>,
}

impl actix::Message for ChannelUpdated {
    type Result = ();
}

#[derive(Clone)]
pub struct MessageCreated {
    pub nickname: String,
    pub channel: String,
    pub content: String,
}

impl actix::Message for MessageCreated {
    type Result = ();
}

pub struct Subscribe(pub Recipient<MessageCreated>);

impl actix::Message for Subscribe {
    type Result = ();
}
