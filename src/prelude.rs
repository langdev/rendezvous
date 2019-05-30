use core::marker::Unpin;

pub use ::actix::prelude::*;
pub use ::futures::{compat::*, prelude::*};
pub use ::log::*;


pub trait AsyncContextExt<A>: AsyncContext<A> where A: Actor<Context = Self> {
    fn add_message_stream_03<S>(&mut self, fut: S)
    where
        S: Stream + Unpin + 'static,
        S::Item: Message,
        A: Handler<S::Item>,
    {
        self.add_message_stream(fut.map(|e| Ok(e)).compat())
    }
}

impl<A, C> AsyncContextExt<A> for C where A: Actor<Context = C>, C: AsyncContext<A> { }

pub trait ArbiterExt {
    fn spawn_async<F>(fut: F) where F: Future<Output = ()> + Unpin + 'static;
}

impl ArbiterExt for Arbiter {
    fn spawn_async<F>(fut: F) where F: Future<Output = ()> + Unpin + 'static {
        Self::spawn(fut.unit_error().compat());
    }
}
