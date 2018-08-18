use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};

use actix::prelude::*;
use futures::{compat::*, prelude::*};
use log::*;
use typemap::{Key, SendMap};


pub struct Bus {
    map: SendMap,
}

impl Bus {
    pub fn subscribe<A, M>(addr: Addr<A>) -> impl TryFuture<Ok = BusId, Error = MailboxError>
    where
        A: Actor + Handler<M>,
        A::Context: actix::dev::ToEnvelope<A, M>,
        M: Message + Send + Clone + 'static,
        M::Result: Send,
    {
        let bus = Bus::from_registry();
        bus.send(Subscribe::new(addr.recipient())).compat()
    }
}

impl Default for Bus {
    fn default() -> Self {
        Bus { map: SendMap::custom() }
    }
}

impl Actor for Bus {
    type Context = Context<Self>;
}

impl Supervised for Bus {}
impl SystemService for Bus {}


static last_id: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BusId(NonZeroUsize);

impl BusId {
    pub fn new() -> Self {
        let id = last_id.fetch_add(1, Ordering::SeqCst);
        assert_ne!(id, 0);
        BusId(NonZeroUsize::new_unchecked(id))
    }

    pub fn publish<M>(&self, message: M)
    where
        M: Message + Send + Clone + 'static,
        M::Result: Send,
    {
        let bus = Bus::from_registry();
        bus.do_send(Publish { message, sender: Some(*self) });
    }
}

pub struct Subscribe<M>
where
    M: Message + Send + 'static,
    M::Result: Send,
{
    pub recipient: Recipient<M>,
}

impl<M> Subscribe<M>
where
    M: Message + Send + 'static,
    M::Result: Send,
{
    pub fn new(recipient: Recipient<M>) -> Self {
        Subscribe { recipient }
    }
}

impl<M> Message for Subscribe<M>
where
    M: Message + Send + 'static,
    M::Result: Send,
{
    type Result = BusId;
}

impl<M> Handler<Subscribe<M>> for Bus
where
    M: Message + Send + 'static,
    M::Result: Send,
{
    type Result = MessageResult<Subscribe<M>>;

    fn handle(&mut self, msg: Subscribe<M>, _: &mut Self::Context) -> Self::Result {
        let list = self.map.entry::<SubscriptionList<M>>().or_insert_with(Default::default);
        let id = BusId::new();
        list.add(msg.recipient);
        MessageResult(id)
    }
}

pub struct Publish<M>
where
    M: Message + Send + 'static,
    M::Result: Send,
{
    pub message: M,
    sender: Option<BusId>,
}



impl<M> Message for Publish<M>
where
    M: Message + Send + 'static,
    M::Result: Send,
{
    type Result = ();
}

impl<M> Handler<Publish<M>> for Bus
where
    M: Message + Send + Clone + 'static,
    M::Result: Send,
{
    type Result = ();

    fn handle(&mut self, msg: Publish<M>, _: &mut Self::Context) -> Self::Result {
        let list = self.map.entry::<SubscriptionList<M>>().or_insert_with(Default::default);
        list.send(msg.message, msg.sender);
    }
}

pub struct SubscriptionList<M>
where
    M: actix::Message + Send,
    M::Result: Send,
{
    subscribers: Vec<(BusId, Recipient<M>)>,
}

impl<M> SubscriptionList<M>
where
    M: actix::Message + Send,
    M::Result: Send,
{
    pub fn new() -> Self {
        SubscriptionList { subscribers: Vec::new() }
    }

    pub fn add(&mut self, recipient: Recipient<M>) -> BusId {
        let id = BusId::new();
        self.subscribers.push((id, recipient));
        id
    }
}

impl<M> SubscriptionList<M>
where
    M: actix::Message + Send + Clone,
    M::Result: Send,
{
    pub fn send(&mut self, msg: M, sender: Option<BusId>) {
        self.subscribers.retain(|(id, s)| {
            if sender == Some(*id) {
                return true;
            }
            match s.do_send(msg.clone()) {
                Ok(_) => { true }
                Err(SendError::Full(_)) => {
                    warn!("mailbox is full");
                    true
                }
                Err(SendError::Closed(_)) => {
                    false
                }
            }
        });
    }
}

impl<M> Default for SubscriptionList<M>
where
    M: actix::Message + Send,
    M::Result: Send,
{
    fn default() -> Self { Self::new() }
}

impl<M> Key for SubscriptionList<M>
where
    M: Message + Send + 'static,
    M::Result: Send,
{
    type Value = Self;
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
}
