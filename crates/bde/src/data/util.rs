use std::error::Error as StdError;
use std::fmt;

use serde::{
    ser::{self, Impossible},
    serde_if_integer128, Serialize, Serializer,
};

pub struct FmtSerializer<'a, 'b>(pub &'a mut fmt::Formatter<'b>);

impl Serializer for &mut FmtSerializer<'_, '_> {
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

impl FmtSerializer<'_, '_> {
    pub fn write<T>(&mut self, value: &T) -> fmt::Result
    where
        T: Serialize,
    {
        Serialize::serialize(value, self).map_err(|e| e.into_inner())
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

#[cfg(test)]
mod test {
    use super::*;

    struct DisplayAdapter<T>(T);

    impl<T> fmt::Display for DisplayAdapter<T>
    where
        T: Serialize,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            FmtSerializer(f).write(&self.0)
        }
    }

    macro_rules! test_should_panic {
        ($($name:ident : $value:expr,)*) => {
            $(
                #[test]
                #[should_panic]
                fn $name() {
                    format!("{}", DisplayAdapter($value));
                }
            )*
        }
    }

    #[derive(Serialize)]
    struct UnitStruct;

    #[derive(Serialize)]
    struct NewTypeStruct<T>(T);

    test_should_panic! {
        serialize_bool: true,
        serialize_i8: 0i8,
        serialize_i16: 1i16,
        serialize_i32: 2i32,
        serialize_i64: 3i64,
        serialize_u8: 4u8,
        serialize_u16: 5u16,
        serialize_u32: 6u32,
        serialize_u64: 7u64,
        serialize_f32: 8f32,
        serialize_f64: 9f64,
        serialize_char: '가',
        serialize_bytes: b"\x00\x01\x02\x03",
        serialize_unit: (),
        serialize_unit_struct: UnitStruct,
        serialize_new_type_struct: NewTypeStruct("str"),
    }

    #[test]
    fn serialize_str() {
        assert_eq!(format!("{}", DisplayAdapter("str")), "str");
    }
}
