use alloc::{format, string::String, string::ToString};
use core::fmt;

use serde::de::{self, DeserializeOwned, IntoDeserializer, Visitor};

use crate::serde::{DATETIME_TOKEN, DECIMAL_TOKEN, UUID_TOKEN};

// ── Error ─────────────────────────────────────────────────────────────────────

/// Deserialization error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeError(String);

impl fmt::Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for DeError {}

impl de::Error for DeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        DeError(msg.to_string())
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Decodes Snowed binary bytes into a `Deserialize` value.
pub fn from_bytes<T: DeserializeOwned>(input: &[u8]) -> Result<T, DeError> {
    let mut de = Deserializer::new(input);
    let v = T::deserialize(&mut de)?;
    if !de.buf.is_empty() {
        return Err(DeError("trailing bytes".to_string()));
    }
    Ok(v)
}

// ── Cursor ────────────────────────────────────────────────────────────────────

/// A serde `Deserializer` that reads Snowed binary data.
pub struct Deserializer<'de> {
    pub(crate) buf: &'de [u8],
}

impl<'de> Deserializer<'de> {
    pub fn new(buf: &'de [u8]) -> Self {
        Self { buf }
    }

    fn take(&mut self, n: usize) -> Result<&'de [u8], DeError> {
        if self.buf.len() < n {
            return Err(DeError("unexpected EOF".to_string()));
        }
        let (head, tail) = self.buf.split_at(n);
        self.buf = tail;
        Ok(head)
    }

    fn take_u8(&mut self) -> Result<u8, DeError> {
        Ok(self.take(1)?[0])
    }

    fn take_le64(&mut self) -> Result<u64, DeError> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn take_bytes(&mut self) -> Result<&'de [u8], DeError> {
        let len = self.take_le64()?;
        let len = usize::try_from(len)
            .map_err(|_| DeError("length overflow".to_string()))?;
        self.take(len)
    }

    fn take_str(&mut self) -> Result<&'de str, DeError> {
        let bytes = self.take_bytes()?;
        core::str::from_utf8(bytes).map_err(|_| DeError("invalid UTF-8".to_string()))
    }

    fn peek(&self) -> Result<u8, DeError> {
        self.buf.first().copied().ok_or_else(|| DeError("unexpected EOF".to_string()))
    }

    /// Compute the base type for a given type byte (forward-compat rule).
    fn base_type(b: u8) -> u8 {
        match b {
            0x00        => 0x00,
            0x01        => 0x00, // reserved → Null
            0x02..=0x07 => 0x04, // Int64 subtypes
            0x08..=0x0F => 0x08, // Float64 subtypes
            0x10..=0x3F => 0x10, // Binary subtypes
            0x40..=0x7F => 0x40, // Array subtypes
            0x80..=0xFE => 0x80, // Object subtypes
            0xFF        => 0xFF,
        }
    }

    /// Read and discard one complete value (used in deserialize_ignored_any).
    fn skip_value(&mut self) -> Result<(), DeError> {
        let b = self.take_u8()?;
        match Self::base_type(b) {
            0x00 => Ok(()),
            0x04 => { self.take(8)?; Ok(()) }
            0x08 => { self.take(8)?; Ok(()) }
            0x10 => { let len = self.take_le64()? as usize; self.take(len)?; Ok(()) }
            0x40 => loop {
                if self.peek()? == 0xFF { self.take_u8()?; return Ok(()); }
                self.skip_value()?;
            },
            0x80 => loop {
                let klen = self.take_u8()? as usize;
                if klen == 0 { return Ok(()); }
                self.take(klen)?;
                self.skip_value()?;
            },
            _ => Err(DeError(format!("unexpected byte 0x{:02X} while skipping", b))),
        }
    }
}

// ── Deserializer trait impl ───────────────────────────────────────────────────

impl<'de> de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        let b = self.take_u8()?;
        match Deserializer::base_type(b) {
            0x00 => visitor.visit_unit(),
            0x04 => match b {
                0x02 => visitor.visit_bool(false),
                0x03 => visitor.visit_bool(true),
                _    => visitor.visit_i64(self.take_le64()? as i64),
            },
            0x08 => {
                let f = f64::from_bits(self.take_le64()?);
                visitor.visit_f64(f)
            }
            0x10 => match b {
                0x11 => { let s = self.take_str()?; visitor.visit_borrowed_str(s) }
                0x12 => { let s = self.take_str()?; visitor.visit_borrowed_str(s) }
                0x13 => { let bs = self.take_bytes()?; visitor.visit_borrowed_bytes(bs) }
                _    => { let bs = self.take_bytes()?; visitor.visit_borrowed_bytes(bs) }
            },
            0x40 => visitor.visit_seq(SeqAccess { de: self, done: false }),
            0x80 => visitor.visit_map(MapAccess { de: self, done: false }),
            _    => Err(DeError(format!("unexpected type byte 0x{:02X}", b))),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.take_u8()? {
            0x02 => visitor.visit_bool(false),
            0x03 => visitor.visit_bool(true),
            b    => Err(DeError(format!("expected Bool, got 0x{:02X}", b))),
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_i8(read_int64(self)? as i8)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_i16(read_int64(self)? as i16)
    }
    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_i32(read_int64(self)? as i32)
    }
    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_i64(read_int64(self)?)
    }
    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_u8(read_uint64(self)? as u8)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_u16(read_uint64(self)? as u16)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_u32(read_uint64(self)? as u32)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_u64(read_uint64(self)?)
    }
    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_f32(read_float64(self)? as f32)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_f64(read_float64(self)?)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        let s = read_str(self)?;
        let mut chars = s.chars();
        let c = chars.next().ok_or_else(|| DeError("empty string for char".to_string()))?;
        if chars.next().is_some() {
            return Err(DeError("multi-char string for char".to_string()));
        }
        visitor.visit_char(c)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_borrowed_str(read_str(self)?)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        let b = self.take_u8()?;
        if Deserializer::base_type(b) != 0x10 {
            return Err(DeError(format!("expected Binary-family, got 0x{:02X}", b)));
        }
        visitor.visit_borrowed_bytes(self.take_bytes()?)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        if self.peek()? == 0x00 {
            self.take_u8()?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        let b = self.take_u8()?;
        if b != 0x00 {
            return Err(DeError(format!("expected Null (0x00), got 0x{:02X}", b)));
        }
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        match name {
            UUID_TOKEN => {
                let b = self.take_u8()?;
                if b != 0x13 {
                    return Err(DeError(format!("expected UUID (0x13), got 0x{:02X}", b)));
                }
                let bytes = self.take_bytes()?;
                if bytes.len() != 16 {
                    return Err(DeError(format!("UUID must be 16 bytes, got {}", bytes.len())));
                }
                visitor.visit_borrowed_bytes(bytes)
            }
            DATETIME_TOKEN => {
                let b = self.take_u8()?;
                if b != 0x09 {
                    return Err(DeError(format!("expected DateTime (0x09), got 0x{:02X}", b)));
                }
                visitor.visit_f64(f64::from_bits(self.take_le64()?))
            }
            DECIMAL_TOKEN => {
                let b = self.take_u8()?;
                if b != 0x12 {
                    return Err(DeError(format!("expected Decimal (0x12), got 0x{:02X}", b)));
                }
                visitor.visit_borrowed_str(self.take_str()?)
            }
            _ => visitor.visit_newtype_struct(self),
        }
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        let b = self.take_u8()?;
        if Deserializer::base_type(b) != 0x40 {
            return Err(DeError(format!("expected Array, got 0x{:02X}", b)));
        }
        visitor.visit_seq(SeqAccess { de: self, done: false })
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        let b = self.take_u8()?;
        if Deserializer::base_type(b) != 0x80 {
            return Err(DeError(format!("expected Object, got 0x{:02X}", b)));
        }
        visitor.visit_map(MapAccess { de: self, done: false })
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        let peeked = self.peek()?;
        if peeked == 0x11 {
            self.take_u8()?;
            let s = self.take_str()?;
            visitor.visit_enum(s.into_deserializer())
        } else if Deserializer::base_type(peeked) == 0x80 {
            self.take_u8()?;
            visitor.visit_enum(EnumAccess { de: self })
        } else {
            Err(DeError(format!("expected String or Object for enum, got 0x{:02X}", peeked)))
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.skip_value()?;
        visitor.visit_unit()
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

fn read_int64<'de>(de: &mut Deserializer<'de>) -> Result<i64, DeError> {
    let b = de.take_u8()?;
    match b {
        0x02 => Ok(0),
        0x03 => Ok(1),
        b if Deserializer::base_type(b) == 0x04 => Ok(de.take_le64()? as i64),
        _ => Err(DeError(format!("expected Int64/Bool, got 0x{:02X}", b))),
    }
}

fn read_uint64<'de>(de: &mut Deserializer<'de>) -> Result<u64, DeError> {
    let b = de.take_u8()?;
    match Deserializer::base_type(b) {
        0x04 => Ok(de.take_le64()?),
        _ => Err(DeError(format!("expected UInt64/Int64, got 0x{:02X}", b))),
    }
}

fn read_float64<'de>(de: &mut Deserializer<'de>) -> Result<f64, DeError> {
    let b = de.take_u8()?;
    if Deserializer::base_type(b) != 0x08 {
        return Err(DeError(format!("expected Float64, got 0x{:02X}", b)));
    }
    Ok(f64::from_bits(de.take_le64()?))
}

fn read_str<'de>(de: &mut Deserializer<'de>) -> Result<&'de str, DeError> {
    let b = de.take_u8()?;
    if Deserializer::base_type(b) != 0x10 {
        return Err(DeError(format!("expected String/Binary, got 0x{:02X}", b)));
    }
    de.take_str()
}

// ── Sequence access ───────────────────────────────────────────────────────────

struct SeqAccess<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    done: bool,
}

impl<'de, 'a> de::SeqAccess<'de> for SeqAccess<'a, 'de> {
    type Error = DeError;

    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, DeError> {
        if self.done { return Ok(None); }
        if self.de.peek()? == 0xFF {
            self.de.take_u8()?;
            self.done = true;
            return Ok(None);
        }
        seed.deserialize(&mut *self.de).map(Some)
    }
}

impl Drop for SeqAccess<'_, '_> {
    /// When serde deserializes a fixed-length sequence (tuple, tuple struct)
    /// it stops calling `next_element_seed` after reading N elements without
    /// asking for the end-of-sequence `None`. The `0xFF` sentinel is therefore
    /// never consumed via the normal path. Consume it here so the buffer stays
    /// clean for the caller.
    fn drop(&mut self) {
        if !self.done && self.de.peek().ok() == Some(0xFF) {
            let _ = self.de.take_u8();
            self.done = true;
        }
    }
}

// ── Map access ────────────────────────────────────────────────────────────────

struct MapAccess<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    done: bool,
}

impl<'de, 'a> de::MapAccess<'de> for MapAccess<'a, 'de> {
    type Error = DeError;

    fn next_key_seed<K: de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, DeError> {
        if self.done { return Ok(None); }
        let klen = self.de.peek()? as usize;
        if klen == 0 {
            self.de.take_u8()?;
            self.done = true;
            return Ok(None);
        }
        self.de.take_u8()?; // consume length byte
        let key_bytes = self.de.take(klen)?;
        let key_str = core::str::from_utf8(key_bytes)
            .map_err(|_| DeError("object key is not valid UTF-8".to_string()))?;
        let k = seed.deserialize(key_str.into_deserializer())?;
        Ok(Some(k))
    }

    fn next_value_seed<V: de::DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, DeError> {
        seed.deserialize(&mut *self.de)
    }
}

// ── Enum access ───────────────────────────────────────────────────────────────

struct EnumAccess<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'de, 'a> de::EnumAccess<'de> for EnumAccess<'a, 'de> {
    type Error = DeError;
    type Variant = VariantAccess<'a, 'de>;

    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, VariantAccess<'a, 'de>), DeError> {
        let klen = self.de.take_u8()? as usize;
        if klen == 0 {
            return Err(DeError("empty key in enum object".to_string()));
        }
        let key_bytes = self.de.take(klen)?;
        let key_str = core::str::from_utf8(key_bytes)
            .map_err(|_| DeError("enum variant key is not valid UTF-8".to_string()))?;
        let v = seed.deserialize(key_str.into_deserializer())?;
        Ok((v, VariantAccess { de: self.de }))
    }
}

struct VariantAccess<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'de, 'a> de::VariantAccess<'de> for VariantAccess<'a, 'de> {
    type Error = DeError;

    fn unit_variant(self) -> Result<(), DeError> {
        let b = self.de.take_u8()?;
        if b != 0x00 {
            return Err(DeError(format!("expected Null for unit variant, got 0x{:02X}", b)));
        }
        let term = self.de.take_u8()?;
        if term != 0x00 {
            return Err(DeError("expected object terminator after unit variant".to_string()));
        }
        Ok(())
    }

    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, DeError> {
        let v = seed.deserialize(&mut *self.de)?;
        let term = self.de.take_u8()?;
        if term != 0x00 {
            return Err(DeError("expected object terminator after newtype variant".to_string()));
        }
        Ok(v)
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        use de::Deserializer as _;
        let v = (&mut *self.de).deserialize_tuple(len, visitor)?;
        let term = self.de.take_u8()?;
        if term != 0x00 {
            return Err(DeError("expected object terminator after tuple variant".to_string()));
        }
        Ok(v)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        use de::Deserializer as _;
        let v = (&mut *self.de).deserialize_struct("", fields, visitor)?;
        let term = self.de.take_u8()?;
        if term != 0x00 {
            return Err(DeError("expected object terminator after struct variant".to_string()));
        }
        Ok(v)
    }
}
