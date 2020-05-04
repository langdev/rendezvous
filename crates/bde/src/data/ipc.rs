use std::io;

use serde::de::{value::StrDeserializer, Deserialize, IntoDeserializer};

use rendezvous_common::{
    anyhow,
    serde_cbor::{self, ser::IoWrite},
};

use super::{Event, EventName};

pub fn deserialize_event<'a>(data: &'a [u8]) -> anyhow::Result<Event<'a>> {
    let pos = data
        .iter()
        .position(|b| *b == b'\n')
        .unwrap_or_else(|| data.len());
    let (header, body) = data.split_at(pos);
    let body = &body[1..];
    let parts: Vec<_> = header.split(|b| *b == b'.').collect();
    // TODO: verify parts[0] == b"event"
    let de: StrDeserializer<serde_cbor::Error> = std::str::from_utf8(parts[1])?.into_deserializer();
    let name = EventName::deserialize(de)?;
    let mut de = serde_cbor::Deserializer::new(serde_cbor::de::SliceRead::new(body));
    Ok(Event::deserialize_inner(name, &mut de)?)
}

pub fn serialize_event<W>(mut writer: W, event: &Event<'_>) -> anyhow::Result<()>
where
    W: io::Write,
{
    writeln!(writer, "event.{}", event.name())?;
    let mut serializer = serde_cbor::Serializer::new(IoWrite::new(writer));
    event.serialize_inner(&mut serializer)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::data::Ready;

    use super::*;

    #[test]
    fn serialize_event_simple() {
        let event = Event::Ready(Ready {
            guilds: vec![],
            session_id: "84174256b9cfa77bb18856c8ae53114e",
        });
        let mut buf = Vec::new();
        serialize_event(&mut buf, &event).unwrap();
        let expected: &[u8] = b"event.READY\n\xa2\x66guilds\x80\x6asession_id\x78\x2084174256b9cfa77bb18856c8ae53114e";
        assert_eq!(buf, expected);
    }

    #[test]
    fn deserialize_event_simple() {
        let data: &[u8] = b"event.READY\n\xa2\x66guilds\x80\x6asession_id\x78\x2084174256b9cfa77bb18856c8ae53114e";
        let event: Event = deserialize_event(data).unwrap();
        match event {
            Event::Ready(e) => {
                assert_eq!(e.guilds.len(), 0);
                assert_eq!(e.session_id, "84174256b9cfa77bb18856c8ae53114e");
            }
            _ => {
                panic!();
            }
        }
    }
}
