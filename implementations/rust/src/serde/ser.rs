use alloc::{format, string::String, string::ToString, vec::Vec};
use core::fmt;

use serde::ser::{self, Serialize};

use crate::serde::{DATETIME_TOKEN, DECIMAL_TOKEN, UUID_TOKEN};

// ── Error ─────────────────────────────────────────────────────────────────────

/// Serialization error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerError(String);

impl fmt::Display for SerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for SerError {}

impl ser::Error for SerError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        SerError(format!("{}", msg))
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Encodes a `Serialize` value into Snowed binary bytes.
pub fn to_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, SerError> {
    let mut s = Serializer::new();
    value.serialize(&mut s)?;
    Ok(s.into_bytes())
}

// ── Serializer ────────────────────────────────────────────────────────────────

/// A serde `Serializer` that writes Snowed binary data.
pub struct Serializer {
    out: Vec<u8>,
    // For tuple-variant: we need to close an outer object after the inner array.
    // We use a stack of pending closers.
    closers: Vec<u8>,
}

impl Default for Serializer {
    fn default() -> Self { Self::new() }
}

impl Serializer {
    pub fn new() -> Self {
        Self { out: Vec::new(), closers: Vec::new() }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.out
    }

    fn push(&mut self, b: u8) { self.out.push(b); }

    fn push_le64(&mut self, v: u64) { self.out.extend_from_slice(&v.to_le_bytes()); }

    fn write_binary_payload(&mut self, bytes: &[u8]) {
        self.push_le64(bytes.len() as u64);
        self.out.extend_from_slice(bytes);
    }

    fn write_key(&mut self, key: &str) -> Result<(), SerError> {
        let kb = key.as_bytes();
        if kb.len() > 255 {
            return Err(SerError(format!("key {:?} exceeds 255 bytes", key)));
        }
        self.push(kb.len() as u8);
        self.out.extend_from_slice(kb);
        Ok(())
    }
}

// ── Serialize a value into a separate buffer ──────────────────────────────────

fn serialize_to_buf<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, SerError> {
    let mut s = Serializer::new();
    value.serialize(&mut s)?;
    Ok(s.out)
}

// ── Main Serializer trait impl ────────────────────────────────────────────────

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = SerError;

    // Use Self for seq/map/struct — the serializer is its own compound accumulator.
    type SerializeSeq           = Self;
    type SerializeTuple         = Self;
    type SerializeTupleStruct   = Self;
    type SerializeTupleVariant  = Self;
    type SerializeMap           = MapState<'a>;
    type SerializeStruct        = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<(), SerError> {
        self.push(if v { 0x03 } else { 0x02 });
        Ok(())
    }

    fn serialize_i8(self, v: i8)   -> Result<(), SerError> { self.serialize_i64(v as i64) }
    fn serialize_i16(self, v: i16) -> Result<(), SerError> { self.serialize_i64(v as i64) }
    fn serialize_i32(self, v: i32) -> Result<(), SerError> { self.serialize_i64(v as i64) }
    fn serialize_i64(self, v: i64) -> Result<(), SerError> {
        self.push(0x04);
        self.push_le64(v as u64);
        Ok(())
    }

    fn serialize_u8(self, v: u8)   -> Result<(), SerError> { self.serialize_u64(v as u64) }
    fn serialize_u16(self, v: u16) -> Result<(), SerError> { self.serialize_u64(v as u64) }
    fn serialize_u32(self, v: u32) -> Result<(), SerError> { self.serialize_u64(v as u64) }
    fn serialize_u64(self, v: u64) -> Result<(), SerError> {
        self.push(0x05);
        self.push_le64(v);
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), SerError> { self.serialize_f64(v as f64) }
    fn serialize_f64(self, v: f64) -> Result<(), SerError> {
        self.push(0x08);
        self.push_le64(v.to_bits());
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<(), SerError> {
        let mut buf = [0u8; 4];
        let s = v.encode_utf8(&mut buf);
        self.serialize_str(s)
    }

    fn serialize_str(self, v: &str) -> Result<(), SerError> {
        self.push(0x11);
        self.write_binary_payload(v.as_bytes());
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<(), SerError> {
        self.push(0x10);
        self.write_binary_payload(v);
        Ok(())
    }

    fn serialize_none(self) -> Result<(), SerError> { self.push(0x00); Ok(()) }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), SerError> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), SerError> { self.push(0x00); Ok(()) }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), SerError> {
        self.push(0x00);
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<(), SerError> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<(), SerError> {
        match name {
            UUID_TOKEN => {
                let buf = serialize_to_buf(value)?;
                // Expected: 0x10 <8-byte-LE-len=16> <16 bytes>
                if buf.len() == 25 && buf[0] == 0x10 {
                    self.push(0x13);
                    self.push_le64(16);
                    self.out.extend_from_slice(&buf[9..]);
                    return Ok(());
                }
                Err(SerError("UUID_TOKEN: inner value must serialize as 16-byte &[u8]".to_string()))
            }
            DATETIME_TOKEN => {
                let buf = serialize_to_buf(value)?;
                // Expected: 0x08 <8 bytes>
                if buf.len() == 9 && buf[0] == 0x08 {
                    self.push(0x09);
                    self.out.extend_from_slice(&buf[1..]);
                    return Ok(());
                }
                Err(SerError("DATETIME_TOKEN: inner value must serialize as f64".to_string()))
            }
            DECIMAL_TOKEN => {
                let buf = serialize_to_buf(value)?;
                // Expected: 0x11 <8-byte-len> <utf8>
                if buf.len() >= 9 && buf[0] == 0x11 {
                    self.push(0x12);
                    self.out.extend_from_slice(&buf[1..]);
                    return Ok(());
                }
                Err(SerError("DECIMAL_TOKEN: inner value must serialize as &str".to_string()))
            }
            _ => value.serialize(self),
        }
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), SerError> {
        self.push(0x80);
        self.write_key(variant)?;
        value.serialize(&mut *self)?;
        self.push(0x00);
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self, SerError> {
        self.push(0x40);
        Ok(self)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self, SerError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<Self, SerError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self, SerError> {
        // Encode as { variant: [ ... ] }
        self.push(0x80);
        self.write_key(variant)?;
        self.push(0x40); // inner array
        // After the array ends we need to close the outer object.
        self.closers.push(0x00);
        Ok(self)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<MapState<'a>, SerError> {
        self.push(0x80);
        Ok(MapState { ser: self, pending_key: None })
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self, SerError> {
        self.push(0x80);
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self, SerError> {
        // Encode as { variant: { ... } }
        self.push(0x80);
        self.write_key(variant)?;
        self.push(0x80); // inner struct object
        self.closers.push(0x00); // outer object closer
        Ok(self)
    }

    fn collect_str<T: fmt::Display + ?Sized>(self, value: &T) -> Result<(), SerError> {
        let s = format!("{}", value);
        self.serialize_str(&s)
    }
}

// ── SerializeSeq / Tuple / TupleStruct (all use `Self`) ───────────────────────

impl ser::SerializeSeq for &mut Serializer {
    type Ok = ();
    type Error = SerError;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), SerError> {
        self.push(0xFF); // sentinel
        // Apply any pending closers (e.g. outer object for tuple_variant).
        while let Some(closer) = self.closers.pop() {
            self.push(closer);
        }
        Ok(())
    }
}

impl ser::SerializeTuple for &mut Serializer {
    type Ok = ();
    type Error = SerError;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<(), SerError> { ser::SerializeSeq::end(self) }
}

impl ser::SerializeTupleStruct for &mut Serializer {
    type Ok = ();
    type Error = SerError;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<(), SerError> { ser::SerializeSeq::end(self) }
}

impl ser::SerializeTupleVariant for &mut Serializer {
    type Ok = ();
    type Error = SerError;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<(), SerError> { ser::SerializeSeq::end(self) }
}

// ── SerializeStruct / StructVariant (use `Self`) ──────────────────────────────

impl ser::SerializeStruct for &mut Serializer {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerError> {
        self.write_key(key)?;
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), SerError> {
        self.push(0x00);
        // Apply any pending closers (outer object for struct_variant).
        while let Some(closer) = self.closers.pop() {
            self.push(closer);
        }
        Ok(())
    }
}

impl ser::SerializeStructVariant for &mut Serializer {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerError> {
        ser::SerializeStruct::serialize_field(self, key, value)
    }

    fn end(self) -> Result<(), SerError> {
        ser::SerializeStruct::end(self)
    }
}

// ── Map serializer ────────────────────────────────────────────────────────────

/// State for serializing maps (key-value pairs where keys come from serde).
pub struct MapState<'a> {
    ser: &'a mut Serializer,
    pending_key: Option<String>,
}

impl ser::SerializeMap for MapState<'_> {
    type Ok = ();
    type Error = SerError;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), SerError> {
        let buf = serialize_to_buf(key)?;
        // Expect: 0x11 <len8> <utf8>
        if buf.len() >= 9 && buf[0] == 0x11 {
            let s = core::str::from_utf8(&buf[9..])
                .map_err(|_| SerError("map key not valid UTF-8".to_string()))?;
            if s.len() > 255 {
                return Err(SerError(format!("map key exceeds 255 bytes: {:?}", s)));
            }
            self.pending_key = Some(s.to_string());
            Ok(())
        } else {
            Err(SerError("map key must serialize as a string".to_string()))
        }
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        let key = self.pending_key.take()
            .ok_or_else(|| SerError("serialize_value called without serialize_key".to_string()))?;
        self.ser.write_key(&key)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), SerError> {
        self.ser.push(0x00);
        Ok(())
    }
}
