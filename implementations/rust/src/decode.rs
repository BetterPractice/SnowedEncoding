use alloc::{
    collections::BTreeMap,
    string::ToString,
    vec::Vec,
};

use crate::SnowedValue;

/// Errors that can occur while decoding Snowed binary data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Input ended before a complete value could be read.
    UnexpectedEof,
    /// A String or Object key contained invalid UTF-8.
    InvalidUtf8,
    /// A Decimal value's payload was not valid UTF-8.
    InvalidDecimalString,
    /// A UUID value's length field was not exactly 16.
    InvalidUuidLength(u64),
    /// A Sentinel byte (0xFF) appeared where a value was expected.
    UnexpectedSentinel,
    /// A value's length field exceeded the available input.
    LengthOverflow,
    /// Trailing bytes remain after the root value was fully read.
    TrailingBytes,
}

/// Decodes a single Snowed value from `input`.
///
/// Returns the decoded [`SnowedValue`]. Trailing bytes after the root value
/// are rejected with [`DecodeError::TrailingBytes`].
pub fn decode(input: &[u8]) -> Result<SnowedValue, DecodeError> {
    let mut reader = Reader::new(input);
    let value = reader.read_value()?;
    if reader.remaining() > 0 {
        return Err(DecodeError::TrailingBytes);
    }
    Ok(value)
}

// ────────────────────────────────────────────────────────────────────────────

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_byte(&mut self) -> Result<u8, DecodeError> {
        if self.pos >= self.data.len() {
            return Err(DecodeError::UnexpectedEof);
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_le64(&mut self) -> Result<u64, DecodeError> {
        if self.pos + 8 > self.data.len() {
            return Err(DecodeError::UnexpectedEof);
        }
        let bytes: [u8; 8] = self.data[self.pos..self.pos + 8]
            .try_into()
            .map_err(|_| DecodeError::UnexpectedEof)?;
        self.pos += 8;
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], DecodeError> {
        if self.pos + count > self.data.len() {
            return Err(DecodeError::UnexpectedEof);
        }
        let slice = &self.data[self.pos..self.pos + count];
        self.pos += count;
        Ok(slice)
    }

    // ── Length-prefixed payload (Binary family) ──────────────────────────────

    fn read_length_prefixed(&mut self) -> Result<&'a [u8], DecodeError> {
        let len = self.read_le64()?;
        let count = usize::try_from(len).map_err(|_| DecodeError::LengthOverflow)?;
        self.read_bytes(count)
    }

    // ── Core value reader ────────────────────────────────────────────────────

    fn read_value(&mut self) -> Result<SnowedValue, DecodeError> {
        let type_byte = self.read_byte()?;
        self.read_value_with_type(type_byte)
    }

    fn read_value_with_type(&mut self, type_byte: u8) -> Result<SnowedValue, DecodeError> {
        match type_byte {
            // ── Null family (0x00–0x03) ──────────────────────────────────────
            0x00 | 0x01 => Ok(SnowedValue::Null),
            0x02 => Ok(SnowedValue::Bool(false)),
            0x03 => Ok(SnowedValue::Bool(true)),

            // ── Int family (0x04–0x07) ───────────────────────────────────────
            0x04 => {
                let bits = self.read_le64()?;
                Ok(SnowedValue::Int64(bits as i64))
            }
            0x05 => {
                let v = self.read_le64()?;
                Ok(SnowedValue::UInt64(v))
            }
            0x06 | 0x07 => {
                // Unknown Int subtype → decode as Int64 (base type round-down, §4.4)
                let bits = self.read_le64()?;
                Ok(SnowedValue::Int64(bits as i64))
            }

            // ── Float family (0x08–0x0F) ─────────────────────────────────────
            0x08 => {
                let bits = self.read_le64()?;
                Ok(SnowedValue::Float64(f64::from_bits(bits)))
            }
            0x09 => {
                let bits = self.read_le64()?;
                Ok(SnowedValue::DateTime(f64::from_bits(bits)))
            }
            0x0A..=0x0F => {
                // Unknown Float subtype → decode as Float64 (base type round-down, §4.4)
                let bits = self.read_le64()?;
                Ok(SnowedValue::Float64(f64::from_bits(bits)))
            }

            // ── Binary family (0x10–0x3F) ────────────────────────────────────
            0x10 => {
                let bytes = self.read_length_prefixed()?;
                Ok(SnowedValue::Binary(bytes.to_vec()))
            }
            0x11 => {
                let bytes = self.read_length_prefixed()?;
                let s = core::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
                Ok(SnowedValue::String(s.to_string()))
            }
            0x12 => {
                let bytes = self.read_length_prefixed()?;
                let s = core::str::from_utf8(bytes)
                    .map_err(|_| DecodeError::InvalidDecimalString)?;
                Ok(SnowedValue::Decimal(s.to_string()))
            }
            0x13 => {
                let len = self.read_le64()?;
                if len != 16 {
                    return Err(DecodeError::InvalidUuidLength(len));
                }
                let bytes = self.read_bytes(16)?;
                let arr: [u8; 16] = bytes.try_into().map_err(|_| DecodeError::UnexpectedEof)?;
                Ok(SnowedValue::Uuid(arr))
            }
            0x14..=0x3F => {
                // Unknown Binary subtype → decode as Binary (base type round-down, §4.4)
                let bytes = self.read_length_prefixed()?;
                Ok(SnowedValue::Binary(bytes.to_vec()))
            }

            // ── Array family (0x40–0x7F) ─────────────────────────────────────
            0x40..=0x7F => {
                let mut elements = Vec::new();
                loop {
                    let next = self.read_byte()?;
                    if next == 0xFF {
                        break;
                    }
                    elements.push(self.read_value_with_type(next)?);
                }
                Ok(SnowedValue::Array(elements))
            }

            // ── Object family (0x80–0xFE) ────────────────────────────────────
            0x80..=0xFE => {
                let mut map = BTreeMap::new();
                loop {
                    let key_len = self.read_byte()?;
                    if key_len == 0x00 {
                        break; // zero-length key terminates (§5.12)
                    }
                    let key_bytes = self.read_bytes(key_len as usize)?;
                    let key = core::str::from_utf8(key_bytes)
                        .map_err(|_| DecodeError::InvalidUtf8)?
                        .to_string();
                    let value_type = self.read_byte()?;
                    if value_type == 0xFF {
                        break; // Sentinel in value position terminates (§5.12)
                    }
                    let value = self.read_value_with_type(value_type)?;
                    map.insert(key, value);
                }
                Ok(SnowedValue::Object(map))
            }

            // ── Sentinel (0xFF) ──────────────────────────────────────────────
            0xFF => Err(DecodeError::UnexpectedSentinel),
        }
    }
}
