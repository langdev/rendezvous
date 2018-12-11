use std::thread;
use std::time::Duration;

use ::actix::prelude::*;
use serenity::model::prelude::*;
use serenity::builder::ExecuteWebhook;
use serenity::utils::vecmap_to_json_map;
use serenity::http::raw::execute_webhook;

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

        let send = || if let Some(nick) = &msg.nickname {
            // 닉네임이 있음; Webhook을 사용해 전송함

            // 닉네임을 [2, 32] 글자 범위로 바꿔야만 함
            //
            // References:
            //   https://discordapp.com/developers/docs/resources/user#usernames-and-nicknames
            //   https://github.com/reactiflux/discord-irc/pull/454
            let truncated_nick = &format!("{:_<2}", nick)[..32];

            // 웹훅 발송
            let data = ExecuteWebhook::default().content(&msg.content).username(truncated_nick);
            let map = vecmap_to_json_map(data.0);
            execute_webhook(id, token, false, &map).map(|_| ())
        } else {
            // 닉네임이 없음; Bot API를 사용해 전송함
            msg.channel.send_message(|m| m.content(&msg.content)).map(|_| ())
        };

        // 메세지 전송
        let result = send();

        // 실패한경우 유한번 재시도함
        if let Err(mut error) = result {
            for _ in 0..1000 {
                match error {
                    // 아래 세 유형의 에러일경우 재시도
                    Http(_) | Hyper(_) | WebSocket(_) => (),
                    // 그 외에는 포기하고 즉시 에러 반환
                    _ => break
                }

                thread::sleep(Duration::from_millis(100));

                match send() {
                    Ok(_) => return Ok(()),
                    Err(e) => error = e,
                }
            }

            return Err(From::from(error))
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
