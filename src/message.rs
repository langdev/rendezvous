use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};
use std::sync::mpsc::{TryRecvError, TrySendError};

use multiqueue;

type Sender = multiqueue::BroadcastSender<Payload>;
type Receiver = multiqueue::BroadcastReceiver<Payload>;

#[derive(Clone)]
pub struct Bus {
    pub id: BusId,
    sender: Sender,
    receiver: Receiver,
}

impl Bus {
    pub fn root() -> Bus {
        let (sender, receiver) = multiqueue::broadcast_queue(16);
        Bus {
            id: BusId::new(),
            sender,
            receiver,
        }
    }

    pub fn add(&self) -> Bus {
        Bus {
            id: BusId::new(),
            sender: self.sender.clone(),
            receiver: self.receiver.add_stream(),
        }
    }

    pub fn sender(&self) -> BusSender {
        BusSender {
            id: self.id,
            sender: self.sender.clone(),
        }
    }

    pub fn try_recv(&self) -> Result<Payload, TryRecvError> {
        loop {
            let payload = self.receiver.try_recv()?;
            if self.id != payload.sender {
                return Ok(payload);
            }
        }
    }
}

impl PartialEq for Bus {
    fn eq(&self, other: &Bus) -> bool {
        self.id == other.id
    }
}

impl IntoIterator for Bus {
    type Item = Payload;
    type IntoIter = BusIter;

    fn into_iter(self) -> Self::IntoIter {
        BusIter {
            bus_id: self.id,
            recv: self.receiver,
        }
    }
}

pub struct BusIter {
    bus_id: BusId,
    recv: multiqueue::BroadcastReceiver<Payload>,
}

impl Iterator for BusIter {
    type Item = Payload;

    #[inline(always)]
    fn next(&mut self) -> Option<Payload> {
        loop {
            return match self.recv.recv() {
                Ok(val) => {
                    if val.sender == self.bus_id {
                        continue;
                    }
                    Some(val)
                },
                Err(_) => None,
            };
        }
    }
}



pub struct BusSender {
    id: BusId,
    sender: Sender,
}

impl BusSender {
    pub fn try_send(&self, msg: Message) -> Result<(), TrySendError<Message>> {
        use self::TrySendError::*;
        let payload = Payload { sender: self.id, message: msg };
        self.sender.try_send(payload).map_err(|e| match e {
            Disconnected(Payload { message, .. }) => Disconnected(message),
            Full(Payload { message, .. }) => Full(message),
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BusId(usize);

static COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;

impl BusId {
    fn new() -> BusId {
        BusId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

#[derive(Clone)]
pub struct Payload {
    pub sender: BusId,
    pub message: Message,
}

#[derive(Clone)]
pub enum Message {
    MessageCreated(MessageCreated),
}

#[derive(Clone)]
pub struct MessageCreated {
    pub nickname: String,
    pub channel: String,
    pub content: String,
}


#[cfg(test)]
mod test {
    use std::collections::BTreeSet;
    use std::sync::mpsc::channel;
    use std::thread;
    use std::time::Duration;

    use rand::{self, Rng};

    use super::*;

    #[test]
    fn bus_id_uniqueness() {
        let n = 100;
        let mut rng = rand::thread_rng();
        let mut handle = vec![];
        let rx = {
            let (tx, rx) = channel();
            for _ in 0..n {
                let d = Duration::from_millis(rng.gen_range(0, 1000));
                let tx = tx.clone();
                let h = thread::spawn(move || {
                    thread::sleep(d);
                    let id = BusId::new();
                    tx.send(id).unwrap();
                });
                handle.push(h);
            }
            rx
        };
        for h in handle {
            h.join().unwrap();
        }
        let ids: Vec<_> = rx.into_iter().collect();
        assert_eq!(ids.len(), n);
        let set: BTreeSet<_> = ids.iter().map(|i| i.0).collect();
        assert_eq!(set.len(), ids.len());
    }

    #[test]
    fn bus_partial_eq() {
        let a = Bus::root();
        let b = a.clone();
        let c = a.add();
        let d = c.clone();
        let e = b.add();
        assert!(a == b);
        assert!(a != c);
        assert!(a != d);
        assert!(a != e);
        assert!(b != c);
        assert!(b != d);
        assert!(b != e);
        assert!(c == d);
        assert!(c != e);
        assert!(d != e);

        assert!(b == a);
        assert!(d == c);
        assert!(e != a);
    }
}
