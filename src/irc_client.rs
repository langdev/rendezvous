use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};
use std::thread;

use futures::prelude::*;
use futures_cpupool::CpuPool;
use irc::client::prelude::*;
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
    pub fn from_config<'a>(handle: Handle, bus: Bus, cfg: &'a Config)
        -> Result<impl Future<Item=Irc, Error=Error> + 'a>
    {
        let client = IrcServer::new_future(handle.clone(), cfg)?;
        Ok(async_block! {
            let client = await!(client)?;
            client.identify()?;
            let inner = Arc::new(IrcInner {
                client,
                channels: RwLock::new(BTreeSet::new()),
            });
            let pool = CpuPool::new(8);
            handle.spawn(listen(pool.clone(), inner.clone(), bus.clone())
                .map_err(|_| ()));
            spawn_actor(inner.clone(), bus);
            Ok(Irc { inner, pool })
        })
    }
}

#[async]
fn listen(pool: CpuPool, irc: Arc<IrcInner>, bus: Bus) -> Result<()> {
    let sender = bus.sender();
    let mut pong_received = false;
    #[async]
    for message in irc.client.stream() {
        debug!("{}", message);
        let nickname = message.source_nickname()
            .map(String::from)
            .unwrap_or_else(String::new);
        match message.command {
            Command::Response(resp, _, _) => {
                match resp {
                    Response::RPL_WELCOME => {
                        if let Err(e) = irc.sync_channel_joining(Vec::new()) {
                            error!("failed to join a channel: {}", e);
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
                    debug!("woah");
                }
            }
            _ => { }
        }
    }
    Ok(())
}

fn spawn_actor(irc: Arc<IrcInner>, bus: Bus) {
    use message::Message::*;
    thread::spawn(move || {
        for Payload { message, .. } in bus {
            match message {
                ChannelUpdated { channels } => {
                    if let Err(e) = irc.sync_channel_joining(channels) {
                        error!("failed to join a channel: {}", e);
                    }
                }
                MessageCreated(msg) => {
                    let m = format!("<{}> {}", msg.nickname, msg.content);
                    if let Err(e) = irc.client.send(Command::PRIVMSG(msg.channel, m)) {
                        error!("failed to send a message: {}", e);
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
