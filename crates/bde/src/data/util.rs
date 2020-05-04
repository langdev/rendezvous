use std::error::Error as StdError;
use std::fmt;

use serde::{
    ser::{self, Impossible},
    serde_if_integer128, Serialize, Serializer,
};

pub struct FmtSerializer<'a, 'b>(pub &'a mut fmt::Formatter<'b>);

impl Serializer for FmtSerializer<'_, '_> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.0.write_str(v)?;
        Ok(())
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        unimpl()
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        unimpl()
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }
    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        unimpl()
    }
    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        unimpl()
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        unimpl()
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        unimpl()
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        unimpl()
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        unimpl()
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        unimpl()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        unimpl()
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        unimpl()
    }

    serde_if_integer128! {
        fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
            unimpl()
        }
        fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
            unimpl()
        }
    }

    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: fmt::Display,
    {
        write!(self.0, "{}", value)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Error(fmt::Error);

impl Error {
    pub fn into_inner(self) -> fmt::Error {
        self.0
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.0)
    }
}

impl From<fmt::Error> for Error {
    fn from(e: fmt::Error) -> Self {
        Error(e)
    }
}

impl ser::Error for Error {
    fn custom<T>(_msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error(fmt::Error)
    }
}

fn unimpl<T>() -> Result<T, Error> {
    Err(Error(fmt::Error))
}
