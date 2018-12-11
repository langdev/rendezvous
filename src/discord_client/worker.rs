use std::thread;
use std::time::Duration;

use ::actix::prelude::*;
use ::serenity::{
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
    /// 닉네임이 지정되지 않은 경우 Discord Bot API의 `send_message` 함수를 사용하고,
    /// 닉네임이 지정된 경우 Discord WebHook을 사용하여 주어진 닉네임과 함께 메세지를 전송한다.
    pub(super) nickname: Option<String>,
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

#[derive(Debug, Message)]
#[rtype(result = "Result<serenity::model::channel::Channel, crate::Error>")]
pub(super) struct GetChannel {
    pub(super) channel: ChannelId,
}

impl Handler<GetChannel> for DiscordWorker {
    type Result = Result<Channel, Error>;

    fn handle(&mut self, msg: GetChannel, _: &mut Self::Context) -> Self::Result {
        msg.channel.to_channel().map_err(Error::from)
    }
}

#[derive(Debug, Message)]
#[rtype(result = "Result<(), crate::Error>")]
pub(super) struct BroadcastTyping {
    pub(super) channel: ChannelId,
}

impl Handler<BroadcastTyping> for DiscordWorker {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: BroadcastTyping, _: &mut Self::Context) -> Self::Result {
        msg.channel.broadcast_typing().map_err(Error::from)
    }
}
