use serenity::model::{
    channel::Channel as SerenityChannel,
    prelude::*,
};

#[derive(Clone, Debug)]
pub(super) enum Channel {
    Guild(String, GuildChannel),
    Private(PrivateChannel),
}

#[allow(dead_code)]
impl Channel {
    pub(super) fn from_discord(channel: SerenityChannel) -> Option<Self> {
        match channel {
            SerenityChannel::Guild(ch) => {
                let ch = ch.read();
                if ch.kind != ChannelType::Text {
                    return None;
                }
                Some(Channel::Guild(ch.name.clone(), ch.clone()))
            }
            SerenityChannel::Private(ch) => {
                let ch = ch.read();
                Some(Channel::Private(ch.clone()))
            }
            _ => None,
        }
    }

    pub(super) fn kind(&self) -> ChannelType {
        match self {
            Channel::Guild(_, ch) => ch.kind,
            Channel::Private(ch) => ch.kind,
        }
    }

    pub(super) fn into_guild(self) -> Option<(String, GuildChannel)> {
        if let Channel::Guild(name, ch) = self { Some((name, ch)) } else { None }
    }

    pub(super) fn into_private(self) -> Option<PrivateChannel> {
        if let Channel::Private(ch) = self { Some(ch) } else { None }
    }

    pub(super) fn as_guild(&self) -> Option<(&str, &GuildChannel)> {
        if let Channel::Guild(name, ch) = self { Some((&name[..], ch)) } else { None }
    }

    pub(super) fn as_private(&self) -> Option<&PrivateChannel> {
        if let Channel::Private(ch) = self { Some(ch) } else { None }
    }

    pub(super) fn as_ref(&self) -> ChannelRef<'_> {
        match self {
            Channel::Guild(name, ch) => ChannelRef::Guild(&name[..], ch),
            Channel::Private(ch) => ChannelRef::Private(ch),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ChannelRef<'a> {
    Guild(&'a str, &'a GuildChannel),
    Private(&'a PrivateChannel),
}
