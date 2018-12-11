use std::thread;
use std::time::Duration;

use ::actix::prelude::*;
use serenity::model::prelude::*;
use serenity::http;

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
        use serenity::Error::{Http, Hyper, WebSocket};

        // TODO: 설정을 어딘가 괜찮은곳에 저장해야함
        let id = 245037420704169985;
        let token = "ig5AO-wdVWpCBtUUMxmgsWryqgsW3DChbKYOINftJ4DCrUbnkedoYZD0VOH1QLr-S3sV";
        let webhook = http::get_webhook_with_token(id, token).expect("valid webhook");

        loop {
            // 메세지 전송
            let result = if let Some(nick) = &msg.nickname {
                // 닉네임이 있음; Webhook을 사용해 전송함
                webhook.execute(true, |w| w.content(&msg.content).username(nick)).map(|_| ())
            } else {
                // 닉네임이 없음; Bot API를 사용해 전송함
                msg.channel.send_message(|m| m.content(&msg.content)).map(|_| ())
            };

            match result {
                // 성공할경우 결과 반환
                Ok(()) => {
                    return Ok(())
                }
                // 다음 에러중 하나일경우 일정시간 대기 후 재시도
                Err(Http(_)) | Err(Hyper(_)) | Err(WebSocket(_)) => {
                    thread::sleep(Duration::from_millis(100))
                }
                // 모르는 유형의 에러일경우 즉시 포기
                Err(e) => {
                    return Err(From::from(e))
                }
            }
        }
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
