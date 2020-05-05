pub mod ipc;
mod util;

use std::borrow::Cow;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{
    de::{self, MapAccess, Visitor},
    ser, Deserialize, Serialize,
};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug)]
pub enum Payload<'a> {
    Event { seq: i64, event: Event<'a> },
    Reconnect,
    Hello(Hello),
    HeartbeatAck,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, PartialEq, Serialize_repr)]
#[repr(u8)]
pub enum OpCode {
    Dispatch = 0,
    Heartbeat = 1,
    Identify = 2,
    PresenceUpdate = 3,
    VoiceStateUpdate = 4,
    Resume = 6,
    Reconnect = 7,
    RequestGuildMembers = 8,
    InvalidSession = 9,
    Hello = 10,
    HeartbeatAck = 11,
}

macro_rules! event {
    ($($borrowed:ident,)* / $($owned:ident,)*) => {
        #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum EventName {
            $($borrowed,)*
            $($owned,)*
        }

        #[derive(Debug, Deserialize, Serialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum Event<'a> {
            $(
                #[serde(borrow)]
                $borrowed($borrowed<'a>),
            )*
            $(
                $owned($owned),
            )*
        }

        impl<'a> Event<'a> {
            pub fn name(&self) -> EventName {
                match self {
                    $(
                        Event::$borrowed(_) => EventName::$borrowed,
                    )*
                    $(
                        Event::$owned(_) => EventName::$owned,
                    )*
                }
            }

            pub fn deserialize_inner<D>(name: EventName, deserializer: D) -> Result<Self, D::Error>
            where
                D: de::Deserializer<'a>
            {
                match name {
                    $(
                        EventName::$borrowed => Ok(Event::$borrowed(Deserialize::deserialize(deserializer)?)),
                    )*
                    $(
                        EventName::$owned => Ok(Event::$owned(Deserialize::deserialize(deserializer)?)),
                    )*
                }
            }

            pub fn serialize_inner<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ser::Serializer
            {
                match self {
                    $(
                        Event::$borrowed(e) => Serialize::serialize(e, serializer),
                    )*
                    $(
                        Event::$owned(e) => Serialize::serialize(e, serializer),
                    )*
                }
            }
        }

        fn deserialize_event<'de, V>(event: &EventName, map: &mut V) -> Result<Event<'de>, V::Error>
        where
            V: MapAccess<'de>,
        {
            #[allow(unreachable_patterns)]
            match event {
                $(
                    EventName::$borrowed => Ok(Event::$borrowed(map.next_value()?)),
                )*
                $(
                    EventName::$owned => Ok(Event::$owned(map.next_value()?)),
                )*
                _ => Err(de::Error::custom("oops")),
            }
        }
    }
}

event! {
    Ready,
    GuildCreate,
    GuildMemberAdd,
    GuildMemberRemove,
    GuildMemberUpdate,
    MessageCreate,
    /
    WebhooksUpdate,
}

impl std::fmt::Display for EventName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self::util::FmtSerializer(f).write(self)
    }
}

#[derive(Debug, Deserialize)]
pub struct Hello {
    #[serde(with = "interval")]
    pub heartbeat_interval: Duration,
}

mod interval {
    use std::time::Duration;

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let millis = deserializer.deserialize_u64(super::U64Visitor { allow_str: false })?;
        Ok(Duration::from_millis(millis))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Ready<'a> {
    pub guilds: Vec<UnavailableGuild>,
    pub session_id: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GuildCreate<'a> {
    pub id: Snowflake,
    pub name: Cow<'a, str>,
    #[serde(borrow)]
    pub members: Vec<GuildMember<'a>>,
    #[serde(borrow)]
    pub channels: Vec<Channel<'a>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GuildMemberAdd<'a> {
    pub guild_id: Snowflake,
    #[serde(borrow, flatten)]
    pub member: GuildMember<'a>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GuildMemberRemove<'a> {
    pub guild_id: Snowflake,
    #[serde(borrow)]
    pub user: User<'a>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GuildMemberUpdate<'a> {
    pub guild_id: Snowflake,
    pub roles: Vec<Snowflake>,
    #[serde(borrow)]
    pub user: User<'a>,
    pub nick: Option<Cow<'a, str>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GuildMember<'a> {
    #[serde(borrow)]
    pub user: Option<User<'a>>,
    #[serde(borrow, flatten)]
    pub member: Member<'a>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct User<'a> {
    pub id: Snowflake,
    pub username: Cow<'a, str>,
    pub discriminator: &'a str,
    pub avatar: Option<&'a str>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Member<'a> {
    pub nick: Option<Cow<'a, str>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Channel<'a> {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub type_: ChannelType,
    pub name: Cow<'a, str>,
    pub parent_id: Option<Snowflake>,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, PartialEq, Serialize_repr)]
#[repr(u8)]
pub enum ChannelType {
    GulidText = 0,
    Dm = 1,
    GuildVoice = 2,
    GroupDm = 3,
    GuildCategory = 4,
    GuildNews = 5,
    GuildStore = 6,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UnavailableGuild {
    pub id: Snowflake,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageCreate<'a> {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    #[serde(borrow)]
    pub author: User<'a>,
    pub member: Option<Member<'a>>,
    pub content: Cow<'a, str>,
    pub timestamp: DateTime<Utc>,
    #[serde(borrow)]
    pub mentions: Vec<UserMention<'a>>,
    #[serde(borrow, default)]
    pub mention_channels: Vec<ChannelMention<'a>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserMention<'a> {
    #[serde(borrow, flatten)]
    pub user: User<'a>,
    #[serde(borrow)]
    pub member: Option<Member<'a>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChannelMention<'a> {
    pub id: Snowflake,
    pub guild_id: Snowflake,
    #[serde(rename = "type")]
    pub type_: ChannelType,
    pub name: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhooksUpdate {
    pub guild_id: Snowflake,
    pub channel_id: Snowflake,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snowflake(u64);

impl From<u64> for Snowflake {
    fn from(id: u64) -> Self {
        Snowflake(id)
    }
}

impl<'d> Deserialize<'d> for Snowflake {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'d>,
    {
        deserializer
            .deserialize_any(U64Visitor { allow_str: true })
            .map(Snowflake)
    }
}

impl<'d> Deserialize<'d> for Payload<'d> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'d>,
    {
        struct PayloadVisitor;
        impl<'de> Visitor<'de> for PayloadVisitor {
            type Value = Payload<'de>;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("enum Payload")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut op = None;
                let mut data = None;
                let mut t = None;
                let mut s = None;
                while let Some(k) = map.next_key()? {
                    match (k, &t, op) {
                        ("t", None, _) => {
                            t = map.next_value()?;
                        }
                        ("t", Some(_), _) => {
                            return Err(de::Error::duplicate_field("t"));
                        }
                        ("op", _, None) => {
                            let i = map.next_value()?;
                            match i {
                                OpCode::Reconnect => {
                                    data = Some(Payload::Reconnect);
                                }
                                OpCode::HeartbeatAck => {
                                    data = Some(Payload::HeartbeatAck);
                                }
                                _ => {}
                            }
                            op = Some(i);
                        }
                        ("op", _, Some(_)) => {
                            return Err(de::Error::duplicate_field("op"));
                        }
                        ("d", _, Some(OpCode::Reconnect))
                        | ("d", _, Some(OpCode::HeartbeatAck)) => {
                            let _: () = map.next_value()?;
                        }
                        ("d", _, Some(OpCode::Hello)) => {
                            if data.is_some() {
                                return Err(de::Error::duplicate_field("d"));
                            }
                            data = Some(Payload::Hello(map.next_value()?));
                        }
                        ("d", Some(event), Some(OpCode::Dispatch)) => {
                            if data.is_some() {
                                return Err(de::Error::duplicate_field("d"));
                            }
                            let seq = match s {
                                Some(s) => s,
                                None => {
                                    return Err(de::Error::missing_field("s"));
                                }
                            };
                            let event = deserialize_event(event, &mut map)?;
                            data = Some(Payload::Event { event, seq });
                        }
                        ("d", _, _) => {
                            return Err(de::Error::custom("oh uh"));
                        }
                        ("s", _, _) => {
                            s = map.next_value()?;
                        }
                        _ => {
                            map.next_value()?;
                        }
                    }
                }
                data.ok_or(de::Error::missing_field("d"))
            }
        }
        deserializer.deserialize_struct("Payload", &["op", "d"], PayloadVisitor)
    }
}

struct U64Visitor {
    allow_str: bool,
}

impl<'de> Visitor<'de> for U64Visitor {
    type Value = u64;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad("integer")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if self.allow_str {
            value
                .parse()
                .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(value), &self))
        } else {
            Err(de::Error::invalid_type(de::Unexpected::Str(value), &self))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn payload_deserialize() {
        let data = r#"{"t":null,"s":null,"op":10,"d":{"heartbeat_interval":41250,"_trace":["[\"gateway-prd-main-z491\",{\"micros\":0.0}]"]}}"#;
        let payload = serde_json::from_str(data).unwrap();
        match payload {
            Payload::Hello(p) => {
                assert_eq!(p.heartbeat_interval, Duration::from_millis(41250));
            }
            _ => {
                panic!("expected Payload::Hello, got {:?}", payload);
            }
        }
    }

    #[test]
    fn payload_deserialize_ready_event() {
        let data = r#"{"t":"READY","s":1,"op":0,"d":{
            "v":6,"user_settings":{},
            "user":{"verified":true,"username":"Rendezvous (DEV)","mfa_enabled":true,"id":"363704614249562124","flags":0,"email":null,"discriminator":"4536","bot":true,"avatar":null},
            "session_id":"84174256b9cfa77bb18856c8ae53114e",
            "relationships":[],
            "private_channels":[],
            "presences":[],
            "guilds":[{"unavailable":true,"id":"363704885985804289"}],
            "application":{"id":"363704614249562124","flags":0},
            "_trace":["[\"gateway-prd-main-m7p2\",{\"micros\":164574,\"calls\":[\"discord-sessions-prd-1-45\",{\"micros\":155249,\"calls\":[\"start_session\",{\"micros\":151886,\"calls\":[\"api-prd-main-us-east1-kp47\",{\"micros\":145055,\"calls\":[\"get_user\",{\"micros\":10390},\"add_authorized_ip\",{\"micros\":11862},\"get_guilds\",{\"micros\":11708},\"coros_wait\",{\"micros\":1}]}]},\"guilds_connect\",{\"micros\":1,\"calls\":[]},\"presence_connect\",{\"micros\":1812,\"calls\":[]}]}]}]"]
        }}"#;
        let payload = serde_json::from_str(data).unwrap();
        let (e, seq) = match payload {
            Payload::Event {
                event: Event::Ready(e),
                seq,
            } => (e, seq),
            _ => {
                panic!("expected Payload::Event, got {:?}", payload);
            }
        };
        assert_eq!(seq, 1);
        assert_eq!(e.session_id, "84174256b9cfa77bb18856c8ae53114e");
    }

    #[test]
    fn showflake_serialize() {
        let id = Snowflake(363704885985804289);
        let s = serde_json::to_string(&id).unwrap();
        assert_eq!(s, "363704885985804289");
    }
}
