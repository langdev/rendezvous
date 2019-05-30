use ::actix::prelude::*;
use ::log::*;


pub struct SubscriptionList<K, M>
where
    M: Message + Send,
    M::Result: Send,
{
    subscribers: Vec<(K, Recipient<M>)>,
}

impl<K, M> SubscriptionList<K, M>
where
    M: Message + Send,
    M::Result: Send,
{
    pub fn new() -> Self {
        SubscriptionList { subscribers: Vec::new() }
    }

    pub fn add(&mut self, receiver: K, recipient: Recipient<M>) {
        self.subscribers.push((receiver, recipient));
    }
}

impl<K, M> SubscriptionList<K, M>
where
    K: PartialEq,
    M: Message + Send + Clone,
    M::Result: Send,
{
    pub fn send(&mut self, sender: Option<K>, msg: M) {
        self.subscribers.retain(|(id, s)| {
            if sender.as_ref() == Some(id) {
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

impl<K, M> Default for SubscriptionList<K, M>
where
    M: Message + Send,
    M::Result: Send,
{
    fn default() -> Self { Self::new() }
}


#[cfg(test)]
mod test {
    use std::sync::Arc;

    use ::actix::prelude::*;
    use ::futures::compat::*;
    use ::parking_lot::Mutex;

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
    }

    impl Viliager {
        fn new(name: impl Into<String>) -> Self {
            Viliager {
                name: name.into(),
                goods: Arc::default(),
            }
        }
    }

    impl Actor for Viliager {
        type Context = Context<Self>;

        fn started(&mut self, _: &mut Self::Context) {
            println!("{}::started", self.name);
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
            let alice = Viliager::new("Alice");
            let goods = Arc::clone(&alice.goods);
            let alice = alice.start();

            sub.add(42, alice.clone().recipient());
            assert_eq!(sub.subscribers.len(), 1);

            sub.send(None, Parcel::from("Hi"));
            sleep_millis!(200).unwrap();
            assert_eq!(*goods.lock(), "Hi");
        }

        async fn stopped_actor_should_be_unsubscribed() {
            let mut sub = SubscriptionList::new();
            let bob = Viliager::new("Bob");
            let goods = Arc::clone(&bob.goods);
            let bob = bob.start();
            let key = 42;

            sub.add(key, bob.clone().recipient());
            assert_eq!(sub.subscribers.len(), 1);

            sub.send(None, Parcel::from("Clothes"));
            sleep_millis!(200).unwrap();
            assert_eq!(*goods.lock(), "Clothes");

            bob.send(Terminate).compat().await.unwrap();
            sub.send(None, Parcel::from("Drinks"));
            sleep_millis!(200).unwrap();

            assert_eq!(*goods.lock(), "Clothes");
            assert!(sub.subscribers.is_empty());
        }
    }
}
