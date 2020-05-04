use std::marker::Unpin;
use std::time::Duration;

use anyhow::anyhow;
use async_std::{io, prelude::*, stream::interval, writeln};
use async_tungstenite::tungstenite::{Error as WsError, Message};
use futures::{select, FutureExt as _, Sink, SinkExt as _, TryStreamExt as _};
use serde::Serialize;

use crate::data::{Event, Payload};

#[cfg(target_os = "linux")]
const OS: &str = "linux";
#[cfg(target_os = "macos")]
const OS: &str = "macos";
#[cfg(target_os = "windows")]
const OS: &str = "windows";

pub struct State<S> {
    stream: S,
    stdout: io::Stdout,
    timer: futures::stream::Fuse<async_std::stream::Interval>,
    session_id: Option<String>,
    last_sequence_number: Option<i64>,
    heartbeat_tries: u8,
}

impl<S> State<S>
where
    S: Stream<Item = Result<Message, WsError>> + Sink<Message, Error = WsError> + Unpin,
{
    pub async fn establish(mut stream: S, token: &str) -> anyhow::Result<Self> {
        let message = stream
            .try_next()
            .timeout(Duration::from_secs(4))
            .await??
            .ok_or_else(|| anyhow!("invalid"))?;
        let data = message.into_data();
        let payload = serde_json::from_slice(&data)?;
        let timer = match payload {
            Payload::Hello(p) => {
                let t = interval(p.heartbeat_interval);
                futures::StreamExt::fuse(t)
            }
            _ => {
                return Err(anyhow!("unexpected"));
            }
        };
        let payload = util::Command {
            op: util::OpCode::Identify,
            d: util::Identity {
                token,
                properties: util::Properties {
                    os: OS,
                    browser: "async-tungstenite",
                    device: "async-tungstenite",
                },
            },
        };
        let message = Message::binary(serde_json::to_vec(&payload)?);
        stream.send(message).await?;

        Ok(State {
            stream,
            stdout: io::stdout(),
            timer,
            session_id: None,
            last_sequence_number: None,
            heartbeat_tries: 0,
        })
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        loop {
            let message = select! {
                m = self.stream.next().fuse() => {
                    if let Some(it) = m { it? } else { break; }
                }
                _ = futures::StreamExt::select_next_some(&mut self.timer) => {
                    if self.heartbeat_tries > 3 {
                        break;
                    }
                    self.handle_heartbeat().await?;
                    continue;
                }
            };
            let data = message.into_data();
            self.handle_message(&data).await?;
        }
        Ok(())
    }

    async fn handle_message<'a>(&mut self, data: &'a [u8]) -> anyhow::Result<()> {
        let payload = match serde_json::from_slice(&data) {
            Ok(p) => p,
            Err(_) => {
                self.stdout.write_all(&data).await?;
                writeln!(self.stdout).await?;
                return Ok(());
            }
        };

        match payload {
            Payload::Event { event, seq } => {
                self.last_sequence_number = Some(seq);
                match event {
                    Event::Ready(e) => {
                        self.session_id = Some(e.session_id.to_owned());
                    }
                    Event::MessageCreate(_) => {
                        let value: serde_json::Value = serde_json::from_slice(&data)?;
                        let buf = serde_json::to_vec_pretty(&value)?;
                        self.stdout.write_all(&buf).await?;
                        writeln!(self.stdout).await?;
                    }
                    _ => {
                        let s = serde_cbor::ser::to_vec(&event)?;
                        self.stdout.write_all(&s).await?;
                        writeln!(self.stdout).await?;
                    }
                }
            }
            Payload::HeartbeatAck => {
                self.heartbeat_tries = 0;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_heartbeat(&mut self) -> anyhow::Result<()> {
        let payload = util::Command {
            op: util::OpCode::Heartbeat,
            d: self.last_sequence_number,
        };
        self.send(&payload).await?;
        self.heartbeat_tries += 1;
        Ok(())
    }

    async fn send<T: Serialize>(&mut self, value: &T) -> anyhow::Result<()> {
        let message = Message::binary(serde_json::to_vec(value)?);
        self.stream.send(message).await?;
        Ok(())
    }
}

mod util {
    use serde::Serialize;

    pub use crate::data::OpCode;

    #[derive(Serialize)]
    pub(super) struct Command<T> {
        pub op: OpCode,
        pub d: T,
    }

    #[derive(Serialize)]
    pub(super) struct Identity<'a> {
        pub token: &'a str,
        pub properties: Properties<'a>,
    }

    #[derive(Serialize)]
    pub(super) struct Properties<'a> {
        #[serde(rename = "$os")]
        pub os: &'a str,
        #[serde(rename = "$browser")]
        pub browser: &'a str,
        #[serde(rename = "$device")]
        pub device: &'a str,
    }
}
