use std::borrow::Cow;
use std::iter::Extend;

use serenity::model::{channel::Channel as SerenityChannel, prelude::*};

#[derive(Clone, Debug)]
pub(crate) enum Channel {
    Guild(GuildChannel),
    Private(PrivateChannel),
}

#[allow(dead_code)]
impl Channel {
    pub(crate) fn from_discord(channel: SerenityChannel) -> Option<Self> {
        match channel {
            SerenityChannel::Guild(ch) => {
                if ch.kind != ChannelType::Text {
                    return None;
                }
                Some(Channel::Guild(ch.clone()))
            }
            SerenityChannel::Private(ch) => Some(Channel::Private(ch.clone())),
            _ => None,
        }
    }

    pub(crate) fn id(&self) -> ChannelId {
        match self {
            Self::Guild(ch) => ch.id,
            Self::Private(ch) => ch.id,
        }
    }

    pub(crate) fn name(&self) -> Cow<'_, str> {
        match self {
            Channel::Guild(ch) => Cow::Borrowed(&ch.name),
            Channel::Private(ch) => ch.name().into(),
        }
    }

    pub(crate) fn kind(&self) -> ChannelType {
        match self {
            Channel::Guild(ch) => ch.kind,
            Channel::Private(ch) => ch.kind,
        }
    }

    pub(crate) fn into_guild(self) -> Option<GuildChannel> {
        if let Channel::Guild(ch) = self {
            Some(ch)
        } else {
            None
        }
    }

    pub(crate) fn into_private(self) -> Option<PrivateChannel> {
        if let Channel::Private(ch) = self {
            Some(ch)
        } else {
            None
        }
    }

    pub(crate) fn as_guild(&self) -> Option<&GuildChannel> {
        if let Channel::Guild(ch) = self {
            Some(ch)
        } else {
            None
        }
    }

    pub(crate) fn as_private(&self) -> Option<&PrivateChannel> {
        if let Channel::Private(ch) = self {
            Some(ch)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ChannelList {
    items: Vec<Channel>,
}

impl ChannelList {
    pub(crate) fn get_by_id(&self, id: ChannelId) -> Option<&Channel> {
        self.items.iter().find(|ch| ch.id() == id)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&Channel> {
        self.items.iter().find(|ch| ch.name() == name)
    }

    pub(crate) fn get_or_insert_with(
        &mut self,
        id: ChannelId,
        f: impl FnOnce() -> Channel,
    ) -> &Channel {
        match self.items.iter().position(|ch| ch.id() == id) {
            Some(idx) => &self.items[idx],
            None => {
                self.items.push(f());
                self.items.last().expect("infallible")
            }
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Channel> {
        self.items.iter()
    }
}

impl Extend<Channel> for ChannelList {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = Channel>,
    {
        self.items.extend(iter);
    }
}
