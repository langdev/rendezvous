use std::thread;
use std::time::Duration;

use actix::prelude::*;
use serenity::{
    model::prelude::*,
};

use crate::Error;


pub struct DiscordWorker {
    _well: (),
}

impl DiscordWorker {
    pub(super) fn new() -> Self {
        DiscordWorker {
            _well: (),
        }
    }
}

impl Actor for DiscordWorker {
    type Context = SyncContext<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<(), crate::Error>")]
pub(super) struct SendMessage {
    pub(super) channel: ChannelId,
    pub(super) content: String,
}

impl Handler<SendMessage> for DiscordWorker {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: SendMessage, _: &mut Self::Context) -> Self::Result {
        let SendMessage { channel, content, .. } = msg;
        while let Err(e) = channel.send_message(|msg| msg.content(content.clone())) {
            use serenity::Error::*;
            match e {
                Http(..) | Hyper(..) | WebSocket(..) => {
                    thread::sleep(Duration::from_millis(100));
                }
                _ => {
                    return Err(From::from(e));
                }
            }
        }
        Ok(())
    }
}
