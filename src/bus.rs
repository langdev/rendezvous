use core::marker::{PhantomData, Unpin};
use core::mem::PinMut;
use core::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};

use actix::{
    dev::ToEnvelope,
    fut,
    prelude::*,
};
use futures::{compat::*, prelude::*};
use log::*;
use tokio::prelude::{
    Async as Async01,
    Poll as Poll01,
};
use typemap::{Key, SendMap};
use pin_utils::unsafe_pinned;

use crate::util::subscription::SubscriptionList;


pub struct Bus {
    map: SendMap,
}

impl Bus {
    pub fn new_id() -> BusId { BusId::new() }

    pub fn subscribe<A, M>(id: BusId) -> WaitSubscribe<A, M>
    where
        A: Actor + Handler<M>,
        A::Context: AsyncContext<A> + ToEnvelope<A, M>,
        M: Message + Send + Clone + 'static,
        M::Result: Send,
    {
        let bus = Bus::from_registry();
        WaitSubscribe::new(&bus, id)
    }

    pub fn publish<M>(id: BusId, message: M) -> WaitPublish<M>
    where
        M: Message + Send + Clone + 'static,
        M::Result: Send,
    {
        let bus = Bus::from_registry();
        WaitPublish::new(&bus, id, message)
    }

    pub fn do_publish<M>(id: BusId, message: M)
    where
        M: Message + Send + Clone + 'static,
        M::Result: Send,
    {
        let bus = Bus::from_registry();
        bus.do_send(Publish { sender: Some(id), message, });
    }
}

impl Default for Bus {
    fn default() -> Self {
        Bus { map: SendMap::custom() }
    }
}

impl Actor for Bus {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        debug!("Bus::started");
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        debug!("Bus::stopping");
        Running::Stop
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        debug!("Bus::stopped");
    }
}

impl Supervised for Bus {
    fn restarting(&mut self, _: &mut Self::Context) {
        debug!("Bus::restarting");
    }
}

impl SystemService for Bus {
    fn service_started(&mut self, _: &mut Self::Context) {
        debug!("Bus::service_started");
    }
}


static LAST_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BusId(NonZeroUsize);

impl BusId {
    fn new() -> Self {
        let id = LAST_ID.fetch_add(1, Ordering::SeqCst);
        assert_ne!(id, 0);
        unsafe { BusId(NonZeroUsize::new_unchecked(id)) }
    }
}

struct Bucket<M>(PhantomData<M>);
impl<M: Send + 'static> Key for Bucket<M> {
    type Value = SubscriptionList<BusId, M>;
}

#[derive(Message)]
#[rtype(result = "crate::util::subscription::Receiver<M>")]
pub struct Subscribe<M: 'static> {
    receiver: BusId,
    _msg: PhantomData<M>,
}

impl<M> Subscribe<M> {
    pub fn new(receiver: BusId) -> Self {
        Subscribe { receiver, _msg: PhantomData }
    }
}

impl<M: Send + Sized> Handler<Subscribe<M>> for Bus {
    type Result = MessageResult<Subscribe<M>>;

    fn handle(&mut self, msg: Subscribe<M>, _: &mut Self::Context) -> Self::Result {
        debug!("Bus received Subscribe<M>");
        let list = self.map.entry::<Bucket<M>>().or_insert_with(Default::default);
        let rx = list.subscribe(msg.receiver);
        MessageResult(rx)
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Publish<M> {
    sender: Option<BusId>,
    pub message: M,
}

impl<M: Message + Send + Clone + 'static> Handler<Publish<M>> for Bus where M::Result: Send {
    type Result = ();

    fn handle(&mut self, msg: Publish<M>, _: &mut Self::Context) -> Self::Result {
        debug!("Bus received Publish<M>");
        let list = self.map.entry::<Bucket<M>>().or_insert_with(Default::default);
        list.do_send(msg.sender, msg.message);
    }
}


pub struct WaitSubscribe<A, M>
where
    A: Actor,
    M: Send + 'static,
{
    future: fut::FutureWrap<Request<Bus, Subscribe<M>>, A>,
}

impl<A, M> WaitSubscribe<A, M>
where
    A: Actor + Handler<M>,
    A::Context: AsyncContext<A> + ToEnvelope<A, M>,
    M: Message + Send + 'static,
    M::Result: Send,
{
    fn new(addr: &Addr<Bus>, id: BusId) -> Self {
        WaitSubscribe {
            future: fut::wrap_future(addr.send(Subscribe::<M>::new(id)))
        }
    }
}

impl<A, M> Unpin for WaitSubscribe<A, M>
where
    A: Actor + Unpin,
    M: Message + Send + 'static,
    M::Result: Send,
{}

impl<A, M> ActorFuture for WaitSubscribe<A, M>
where
    A: Actor + Handler<M>,
    A::Context: AsyncContext<A> + ToEnvelope<A, M>,
    M: Message + Send + Unpin + 'static,
    M::Result: Send + Unpin,
{
    type Item = ();
    type Error = MailboxError;
    type Actor = A;

    fn poll(&mut self, srv: &mut Self::Actor, ctx: &mut <Self::Actor as Actor>::Context) -> Poll01<Self::Item, Self::Error> {
        debug!("WaitSubscribe::poll");
        let rx = match self.future.poll(srv, ctx)? {
            Async01::Ready(t) => t,
            Async01::NotReady => { return Ok(Async01::NotReady); }
        };
        ctx.add_message_stream(rx.map(Result::Ok).compat(TokioDefaultSpawn));
        Ok(Async01::Ready(()))
    }
}

pub struct WaitPublish<M> where M: Message + Send + Clone + 'static, M::Result: Send {
    future: Compat<Request<Bus, Publish<M>>, ()>,
}

impl<M> WaitPublish<M> where M: Message + Send + Clone + 'static, M::Result: Send {
    unsafe_pinned!(future: Compat<Request<Bus, Publish<M>>, ()>);

    fn new(addr: &Addr<Bus>, id: BusId, message: M) -> Self {
        WaitPublish { future: addr.send(Publish { sender: Some(id), message, }).compat() }
    }
}

impl<M> Unpin for WaitPublish<M> where M: Message + Send + Clone + 'static, M::Result: Send {}

impl<M> Future for WaitPublish<M> where M: Message + Send + Clone + 'static, M::Result: Send {
    type Output = Result<(), MailboxError>;

    fn poll(mut self: PinMut<'_, Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        debug!("WaitPublish::poll");
        self.future().poll(cx)
    }
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
