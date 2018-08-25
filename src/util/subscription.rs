use actix::prelude::*;
use log::*;


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
