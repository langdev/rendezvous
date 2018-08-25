use core::mem::PinMut;
use core::ops::FnOnce;

use actix::prelude::*;
use actix::dev::{MessageResponse, ResponseChannel, ToEnvelope};
use futures::{compat::*, prelude::*};
use pin_utils::unsafe_pinned;

use crate::bus::{self, Bus, BusId};

pub mod subscription;


#[derive(Clone, Message)]
#[rtype(result = "crate::bus::BusId")]
pub struct GetBusId;


impl<A: Actor, M: Message<Result = BusId>> MessageResponse<A, M> for BusId {
    fn handle<R: ResponseChannel<M>>(self, _: &mut A::Context, tx: Option<R>) {
        tx.map(|tx| tx.send(self));
    }
}


pub trait AddrExt {
    type Actor: Actor + Handler<GetBusId>;

    fn subscribe<M>(&self) -> WaitSubscribe<<Self as AddrExt>::Actor, M>
    where
        <Self as AddrExt>::Actor: Handler<M>,
        <<Self as AddrExt>::Actor as Actor>::Context: ToEnvelope<<Self as AddrExt>::Actor, GetBusId>,
        <<Self as AddrExt>::Actor as Actor>::Context: ToEnvelope<<Self as AddrExt>::Actor, M>,
        M: Message + Send + Clone + 'static,
        M::Result: Send,
    ;

    fn publish<M>(&self, message: M) -> WaitPublish<<Self as AddrExt>::Actor, M>
    where
        <<Self as AddrExt>::Actor as Actor>::Context: ToEnvelope<<Self as AddrExt>::Actor, GetBusId>,
        M: Message + Send + Clone + 'static,
        M::Result: Send,
    ;
}

impl<A> AddrExt for Addr<A>
where
    A: Actor,
    A: Handler<GetBusId>,
    A::Context: ToEnvelope<A, GetBusId>,
{
    type Actor = A;

    fn subscribe<M>(&self) -> WaitSubscribe<A, M>
    where
        A: Handler<M>,
        A::Context: ToEnvelope<A, GetBusId>,
        A::Context: ToEnvelope<A, M>,
        M: Message + Send + Clone + 'static,
        M::Result: Send,
    {
        WaitSubscribe::new(self)
    }

    fn publish<M>(&self, message: M) -> WaitPublish<A, M>
    where
        A::Context: ToEnvelope<A, GetBusId>,
        M: Message + Send + Clone + 'static,
        M::Result: Send,
    {
        WaitPublish::new(self, message)
    }
}


type WaitSubscribeInner<A, M> = future::AndThen<
    Compat<Request<A, GetBusId>, ()>,
    bus::WaitSubscribe<M>,
    ToSubscribe<M>,
>;

#[must_use = "futures do nothing unless polled"]
pub struct WaitSubscribe<A, M>
where
    A: Actor + Handler<GetBusId> + Handler<M>,
    A::Context: ToEnvelope<A, GetBusId> + ToEnvelope<A, M>,
    M: Message + Send + Clone + 'static,
    M::Result: Send,
{
    inner: WaitSubscribeInner<A, M>,
}

impl<A, M> WaitSubscribe<A, M>
where
    A: Actor + Handler<GetBusId> + Handler<M>,
    A::Context: ToEnvelope<A, GetBusId> + ToEnvelope<A, M>,
    M: Message + Send + Clone + 'static,
    M::Result: Send,
{
    unsafe_pinned!(inner: WaitSubscribeInner<A, M>);

    fn new(addr: &Addr<A>) -> Self {
        let inner = addr.send(GetBusId).compat()
            .and_then(ToSubscribe { recipient: addr.clone().recipient::<M>() });
        WaitSubscribe { inner }
    }
}

struct ToSubscribe<M> where M: Message + Send + 'static, M::Result: Send {
    recipient: Recipient<M>,
}

impl<M> FnOnce<(BusId,)> for ToSubscribe<M> where M: Message + Send + Clone + 'static, M::Result: Send
{
    type Output = bus::WaitSubscribe<M>;

    extern "rust-call" fn call_once(self, args: (BusId,)) -> Self::Output {
        Bus::subscribe(args.0, self.recipient)
    }
}

impl<A, M> Future for WaitSubscribe<A, M>
where
    A: Actor + Handler<GetBusId> + Handler<M>,
    A::Context: ToEnvelope<A, GetBusId> + ToEnvelope<A, M>,
    M: Message + Send + Clone + 'static,
    M::Result: Send,
{
    type Output = Result<(), MailboxError>;

    fn poll(mut self: PinMut<Self>, cx: &mut std::task::Context) -> Poll<Self::Output> {
        self.inner().poll(cx)
    }
}


type WaitPublishInner<A, M> = future::AndThen<
    Compat<Request<A, GetBusId>, ()>,
    bus::WaitPublish<M>,
    ToPublish<M>,
>;

#[must_use = "futures do nothing unless polled"]
pub struct WaitPublish<A, M>
where
    A: Actor + Handler<GetBusId>,
    A::Context: ToEnvelope<A, GetBusId>,
    M: Message + Send + Clone + 'static,
    M::Result: Send,
{
    inner: WaitPublishInner<A, M>,
}

impl<A, M> WaitPublish<A, M>
where
    A: Actor + Handler<GetBusId>,
    A::Context: ToEnvelope<A, GetBusId>,
    M: Message + Send + Clone + 'static,
    M::Result: Send,
{
    unsafe_pinned!(inner: WaitPublishInner<A, M>);

    fn new(addr: &Addr<A>, message: M) -> Self {
        let inner = addr.send(GetBusId).compat()
            .and_then(ToPublish { message });
        WaitPublish { inner }
    }
}

struct ToPublish<M> {
    message: M,
}

impl<M> FnOnce<(BusId,)> for ToPublish<M> where M: Message + Send + Clone + 'static, M::Result: Send
{
    type Output = bus::WaitPublish<M>;

    extern "rust-call" fn call_once(self, args: (BusId,)) -> Self::Output {
        Bus::publish(args.0, self.message)
    }
}

impl<A, M> Future for WaitPublish<A, M>
where
    A: Actor + Handler<GetBusId>,
    A::Context: ToEnvelope<A, GetBusId>,
    M: Message + Send + Clone + 'static,
    M::Result: Send,
{
    type Output = Result<(), MailboxError>;

    fn poll(mut self: PinMut<Self>, cx: &mut std::task::Context) -> Poll<Self::Output> {
        self.inner().poll(cx)
    }
}
