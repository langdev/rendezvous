use core::marker::Unpin;

pub use ::actix::prelude::*;
pub use ::futures::{compat::*, prelude::*};
pub use ::log::*;

pub trait TokioFutureCompatExt: TryFuture {
    fn tokio_compat(self) -> Compat<Self> where Self: Sized + Unpin {
        self.compat()
    }
}

impl<F> TokioFutureCompatExt for F where F: TryFuture { }

pub trait TokioStreamCompatExt: TryStream {
    fn tokio_compat(self) -> Compat<Self> where Self: Sized + Unpin {
        self.compat()
    }
}

impl<F> TokioStreamCompatExt for F where F: TryStream { }

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
        Self::spawn(fut.unit_error().tokio_compat());
    }
}
