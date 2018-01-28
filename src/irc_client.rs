use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};
use std::thread;

use futures::prelude::*;
use futures_cpupool::CpuPool;
use irc::client::prelude::*;
use slog;
use tokio_core::reactor::Handle;

use ::{Error, Result};
use message::{Bus, Message, MessageCreated, Payload};


pub struct Irc {
    inner: Arc<IrcInner>,
    pool: CpuPool,
}

struct IrcInner {
    client: IrcServer,
    channels: RwLock<BTreeSet<String>>,
}

impl Irc {
    pub fn from_config<'a, L>(handle: Handle, logger: L, bus: Bus, cfg: &'a Config)
    -> Result<impl Future<Item=Irc, Error=Error> + 'a>
        where L: Into<Option<slog::Logger>> + 'a
    {
        let logger = logger.into().unwrap_or_else(|| slog::Logger::root(slog::Discard, o!()));
        let client = IrcServer::new_future(handle.clone(), cfg)?;
        Ok(async_block! {
            let client = await!(client)?;
            client.identify()?;
            let inner = Arc::new(IrcInner {
                client,
                channels: RwLock::new(BTreeSet::new()),
            });
            let pool = CpuPool::new(8);
            handle.spawn(listen(logger.new(o!()), pool.clone(), inner.clone(), bus.clone())
                .map_err(|_| ()));
            spawn_actor(logger, inner.clone(), bus);
            Ok(Irc { inner, pool })
        })
    }
}

#[async]
fn listen(logger: slog::Logger, pool: CpuPool, irc: Arc<IrcInner>, bus: Bus) -> Result<()> {
    let sender = bus.sender();
    let mut pong_received = false;
    #[async]
    for message in irc.client.stream() {
        debug!(logger, "{}", message);
        let nickname = message.source_nickname()
            .map(String::from)
            .unwrap_or_else(String::new);
        match message.command {
            Command::Response(resp, _, _) => {
                match resp {
                    Response::RPL_WELCOME => {
                        if let Err(e) = irc.sync_channel_joining(Vec::new()) {
                            error!(logger, "failed to join a channel: {}", e);
                        }
                    },
                    _ => { }
                }
            }
            Command::PRIVMSG(target, content) => {
                let mut m = Message::MessageCreated(MessageCreated {
                    nickname,
                    channel: target,
                    content,
                });
                let s = sender.clone();
                await!(pool.spawn_fn(move || -> Result<()> {
                    while let Err(e) = s.try_send(m) {
                        use std::sync::mpsc::TrySendError::*;
                        m = match e {
                            Full(p) => p,
                            Disconnected(_) => { panic!("bus closed"); }
                        };
                        thread::yield_now();
                    }
                    Ok(())
                }))?;
            }
            Command::PONG(_, _) => {
                if !pong_received {
                    pong_received = true;
                    debug!(logger, "woah");
                }
            }
            _ => { }
        }
    }
    Ok(())
}

fn spawn_actor(logger: slog::Logger, irc: Arc<IrcInner>, bus: Bus) {
    use message::Message::*;
    thread::spawn(move || {
        for Payload { message, .. } in bus {
            match message {
                ChannelUpdated { channels } => {
                    if let Err(e) = irc.sync_channel_joining(channels) {
                        error!(logger, "failed to join a channel: {}", e);
                    }
                }
                MessageCreated(msg) => {
                    let m = format!("<{}> {}", msg.nickname, msg.content);
                    if let Err(e) = irc.client.send(Command::PRIVMSG(msg.channel, m)) {
                        error!(logger, "failed to send a message: {}", e);
                    }
                }
                _ => { }
            }
        }
    });
}

impl IrcInner {
    fn sync_channel_joining(&self, new_channels: Vec<String>) -> Result<()> {
        let mut ch = self.channels.write().unwrap();
        ch.extend(new_channels);
        if ch.len() <= 0 { return Ok(()); }
        let chanlist = ch.iter()
            .map(|n| format!("#{}", n))
            .collect::<Vec<_>>()
            .join(",");
        self.client.send_join(&chanlist)?;
        Ok(())
    }
}
