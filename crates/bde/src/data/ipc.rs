use std::io;

use serde::de::{self, value::StrDeserializer, Deserialize, Error as _, IntoDeserializer};

use rendezvous_common::serde_cbor::{self, ser::IoWrite};

use super::{Event, EventName};

pub fn deserialize_event<'a>(data: &'a [u8]) -> Result<Event<'a>, serde_cbor::Error> {
    let pos = data
        .iter()
        .position(|b| *b == b'\n')
        .unwrap_or_else(|| data.len());
    let (header, body) = data.split_at(pos);
    let body = &body[1..];
    let de: StrDeserializer<serde_cbor::Error> = parse_header(header)
        .ok_or_else(|| serde_cbor::Error::invalid_value(de::Unexpected::Bytes(header), &"header"))?
        .into_deserializer();
    let name = EventName::deserialize(de)?;
    let mut de = serde_cbor::Deserializer::new(serde_cbor::de::SliceRead::new(body));
    Ok(Event::deserialize_inner(name, &mut de)?)
}

fn parse_header(header: &[u8]) -> Option<&str> {
    let mut parts = header.split(|b| *b == b'.');
    if parts.next()? != b"event" {
        return None;
    }
    let name = parts.next()?;
    std::str::from_utf8(name).ok()
}

pub fn serialize_event<W>(mut writer: W, event: &Event<'_>) -> Result<(), serde_cbor::Error>
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
