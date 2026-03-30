use alloc::vec::Vec;

use crate::SnowedValue;

/// Encodes a [`SnowedValue`] into Snowed binary format.
///
/// Returns the encoded bytes. Object keys are written in lexicographic order
/// for deterministic output.
pub fn encode(value: &SnowedValue) -> Vec<u8> {
    let mut out = Vec::new();
    append_value(value, &mut out);
    out
}

fn append_le64(v: u64, out: &mut Vec<u8>) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn append_value(value: &SnowedValue, out: &mut Vec<u8>) {
    match value {
        SnowedValue::Null => {
            out.push(0x00);
        }

        SnowedValue::Bool(false) => {
            out.push(0x02);
        }
        SnowedValue::Bool(true) => {
            out.push(0x03);
        }

        SnowedValue::Int64(i) => {
            out.push(0x04);
            append_le64((*i) as u64, out);
        }

        SnowedValue::UInt64(u) => {
            out.push(0x05);
            append_le64(*u, out);
        }

        SnowedValue::Float64(f) => {
            out.push(0x08);
            append_le64(f.to_bits(), out);
        }

        SnowedValue::DateTime(ms) => {
            out.push(0x09);
            append_le64(ms.to_bits(), out);
        }

        SnowedValue::Binary(bytes) => {
            out.push(0x10);
            append_le64(bytes.len() as u64, out);
            out.extend_from_slice(bytes);
        }

        SnowedValue::String(s) => {
            let utf8 = s.as_bytes();
            out.push(0x11);
            append_le64(utf8.len() as u64, out);
            out.extend_from_slice(utf8);
        }

        SnowedValue::Decimal(s) => {
            let utf8 = s.as_bytes();
            out.push(0x12);
            append_le64(utf8.len() as u64, out);
            out.extend_from_slice(utf8);
        }

        SnowedValue::Uuid(bytes) => {
            out.push(0x13);
            append_le64(16, out);
            out.extend_from_slice(bytes);
        }

        SnowedValue::Array(elements) => {
            out.push(0x40);
            for elem in elements {
                append_value(elem, out);
            }
            out.push(0xFF);
        }

        SnowedValue::Object(map) => {
            out.push(0x80);
            // BTreeMap iterates in key-sorted order, giving deterministic output.
            for (key, val) in map {
                let key_bytes = key.as_bytes();
                // Key length fits in u8 by construction; max key is 255 bytes (§6).
                out.push(key_bytes.len() as u8);
                out.extend_from_slice(key_bytes);
                append_value(val, out);
            }
            out.push(0x00); // zero-length key terminates object
        }
    }
}
