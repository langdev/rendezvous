use std::marker::Unpin;

use actix::prelude::*;
use futures::{
    channel::mpsc,
    compat::*,
    prelude::*,
};
use log::*;


pub type Sender<M> = mpsc::Sender<M>;
pub type Receiver<M> = mpsc::Receiver<M>;

pub struct SubscriptionList<K, M> where M: Send {
    subscribers: Vec<(K, Sender<M>)>,
}

impl<K, M> SubscriptionList<K, M> where M: Send {
    pub fn new() -> Self {
        SubscriptionList { subscribers: Vec::new() }
    }

    pub fn subscribe(&mut self, id: K) -> Receiver<M> {
        let (tx, rx) = mpsc::channel(8);
        self.subscribers.push((id, tx));
        rx
    }
}

impl<K, M> SubscriptionList<K, M>
where
    M: Message + Send + 'static,
    M::Result: Send + 'static,
{
    #[allow(dead_code)]
    pub fn add<A, C>(&mut self, id: K, ctx: &mut C)
    where
        A: Actor<Context = C> + Handler<M>,
        C: AsyncContext<A>,
    {
        let rx = self.subscribe(id);
        ctx.add_message_stream(rx.map(Result::Ok).compat(TokioDefaultSpawn));
    }
}

impl<K, M> SubscriptionList<K, M> where K: PartialEq, M: Send + Clone
{
    // pub fn send<'a>(&'a mut self, sender_id: Option<K>, msg: M) -> impl Future<Output = ()> + 'a {
    //     let this = self;
    //     pin_mut!(this);
    //     async move {
    //         this.do_send(sender_id, msg);
    //     }
    // }

    pub fn do_send(&mut self, sender_id: Option<K>, msg: M) {
        let mut i = 0;
        while i != self.subscribers.len() {
            let (ref id, ref mut s) = self.subscribers[i];
            if sender_id.as_ref() != Some(id) {
                if let Err(e) = s.try_send(msg.clone()) {
                    if e.is_full() {
                        warn!("mailbox is full");
                    } else if e.is_disconnected() {
                        self.subscribers.remove(i);
                        continue;
                    }
                }
            }
            i += 1;
        }
    }
}

impl<K, M: Send> Default for SubscriptionList<K, M> {
    fn default() -> Self { Self::new() }
}

impl<K, M: Send> Unpin for SubscriptionList<K, M> {}


#[cfg(test)]
mod test {
    use std::sync::Arc;

    use actix::prelude::*;
    use futures::compat::*;
    use parking_lot::Mutex;

    use crate::message::Terminate;

    use super::*;

    #[derive(Clone, Message)]
    #[rtype(result = "()")]
    struct Parcel(String);

    impl From<&str> for Parcel {
        fn from(s: &str) -> Self {
            Parcel(String::from(s))
        }
    }

    struct Viliager {
        name: String,
        goods: Arc<Mutex<String>>,
        id: i32,
        sub: Option<Arc<Mutex<SubscriptionList<i32, Parcel>>>>,
    }

    impl Viliager {
        fn new(id: i32, name: impl Into<String>) -> Self {
            Viliager {
                name: name.into(),
                goods: Arc::default(),
                id,
                sub: None,
            }
        }
    }

    impl Actor for Viliager {
        type Context = Context<Self>;

        fn started(&mut self, ctx: &mut Self::Context) {
            println!("{}::started", self.name);
            if let Some(s) = &self.sub {
                let mut s = s.lock();
                s.add(self.id, ctx);
            }
        }

        fn stopping(&mut self, _: &mut Self::Context) -> Running {
            println!("{}::stopping", self.name);
            Running::Stop
        }

        fn stopped(&mut self, _: &mut Self::Context) {
            println!("{}::stopped", self.name);
        }
    }

    impl Handler<Parcel> for Viliager {
        type Result = ();

        fn handle(&mut self, msg: Parcel, _: &mut Self::Context) -> Self::Result {
            *self.goods.lock() = msg.0;
        }
    }

    impl Handler<Terminate> for Viliager {
        type Result = ();

        fn handle(&mut self, _: Terminate, ctx: &mut Self::Context) -> Self::Result {
            ctx.terminate();
        }
    }

    actix_test_cases! {
        async fn basic() {
            let mut sub = SubscriptionList::<i32, Parcel>::new();

            let mut rx = sub.subscribe(42);
            assert_eq!(sub.subscribers.len(), 1);

            sub.do_send(None, Parcel::from("Hi"));
            let i = await!(rx.next());
            assert_eq!(i.unwrap().0, "Hi");
        }

        async fn stopped_actor_should_be_unsubscribed() {
            let sub = Arc::new(Mutex::new(SubscriptionList::new()));
            let key = 192;
            let mut bob = Viliager::new(key, "Bob");
            bob.sub = Some(Arc::clone(&sub));
            let goods = Arc::clone(&bob.goods);
            let bob = bob.start();

            sleep_millis!(500).unwrap();
            assert_eq!(sub.lock().subscribers.len(), 1);

            sub.lock().do_send(None, Parcel::from("Clothes"));
            sleep_millis!(200).unwrap();
            assert_eq!(*goods.lock(), "Clothes");

            await!(bob.send(Terminate).compat()).unwrap();

            sub.lock().do_send(None, Parcel::from("Drinks"));
            sleep_millis!(200).unwrap();

            assert_eq!(*goods.lock(), "Clothes");
            assert!(sub.lock().subscribers.is_empty());
        }
    }
}
