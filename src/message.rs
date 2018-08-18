use actix::prelude::*;

#[derive(Clone, Debug)]
pub struct ChannelUpdated {
    pub channels: Vec<String>,
}

impl Message for ChannelUpdated {
    type Result = ();
}

#[derive(Clone, Debug)]
pub struct MessageCreated {
    pub nickname: String,
    pub channel: String,
    pub content: String,
}

impl Message for MessageCreated {
    type Result = ();
}
