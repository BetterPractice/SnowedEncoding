use alloc::{
    collections::BTreeMap,
    string::String,
    vec::Vec,
};

/// A type-erased value in the Snowed binary format.
///
/// Covers every type defined in the Snowed spec v0.1.0-draft. Unknown type
/// bytes are decoded to the nearest base type as required by §4.4.
///
/// ## Decimal storage
///
/// `Decimal` stores its value as a canonical UTF-8 [`String`] matching the
/// rules in §5.9. No arithmetic is performed; the string is the wire
/// representation. Enable the `rust_decimal` feature to obtain
/// `From<rust_decimal::Decimal>` and `TryFrom` conversions.
///
/// ## DateTime storage
///
/// `DateTime` stores milliseconds since the Unix epoch (UTC) as an `f64`,
/// matching the wire representation directly. Positive = after epoch,
/// negative = before epoch.
///
/// ## Object key ordering
///
/// Keys in `Object` are stored in a [`BTreeMap`] so that iteration order is
/// deterministic and round-trip encoding produces lexicographically-sorted
/// keys, consistent with other Snowed implementations.
#[derive(Clone, Debug, PartialEq)]
pub enum SnowedValue {
    /// Null (0x00)
    Null,
    /// Boolean (0x02 / 0x03)
    Bool(bool),
    /// Signed 64-bit integer (0x04)
    Int64(i64),
    /// Unsigned 64-bit integer (0x05)
    UInt64(u64),
    /// IEEE 754 double-precision float (0x08)
    Float64(f64),
    /// Milliseconds since Unix epoch UTC, stored as f64 (0x09)
    DateTime(f64),
    /// Raw binary payload (0x10)
    Binary(Vec<u8>),
    /// UTF-8 string (0x11)
    String(String),
    /// Arbitrary-precision decimal, stored as a canonical string (0x12)
    Decimal(String),
    /// UUID — exactly 16 bytes in RFC 4122 byte order (0x13)
    Uuid([u8; 16]),
    /// Ordered sequence of values (0x40)
    Array(Vec<SnowedValue>),
    /// Key-value map with UTF-8 string keys (0x80)
    Object(BTreeMap<String, SnowedValue>),
}

// ────────────────────────────────────────────────────────────────────────────
// PartialEq for Float64 / DateTime
//
// The derived PartialEq uses f64's own PartialEq, which means NaN != NaN.
// That is intentional and correct here — we are comparing wire values, not
// semantic equality. Two NaN floats are not equal just as IEEE 754 specifies.
// ────────────────────────────────────────────────────────────────────────────

// ────────────────────────────────────────────────────────────────────────────
// rust_decimal conversions (optional feature)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "rust_decimal")]
mod rust_decimal_conversions {
    use super::SnowedValue;
    use alloc::string::ToString;
    use rust_decimal::Decimal;

    impl From<Decimal> for SnowedValue {
        fn from(d: Decimal) -> Self {
            SnowedValue::Decimal(canonical_decimal_string(d))
        }
    }

    /// Attempts to parse the canonical decimal string back into a
    /// `rust_decimal::Decimal`.
    impl TryFrom<SnowedValue> for Decimal {
        type Error = rust_decimal::Error;

        fn try_from(v: SnowedValue) -> Result<Self, Self::Error> {
            match v {
                SnowedValue::Decimal(s) => s.parse::<Decimal>(),
                _ => Err(rust_decimal::Error::ConversionTo("non-Decimal SnowedValue".into())),
            }
        }
    }

    /// Produces a canonical Snowed decimal string from a `rust_decimal::Decimal`.
    ///
    /// Rules (§5.9):
    /// - Plain decimal notation, no exponent.
    /// - No trailing zeros after the decimal point.
    /// - No leading zeros before the decimal point (except `0.something`).
    /// - Zero and negative zero → `"0"`.
    pub(crate) fn canonical_decimal_string(d: Decimal) -> alloc::string::String {
        if d.is_zero() {
            return alloc::string::String::from("0");
        }
        // rust_decimal's `to_string` produces plain notation without exponents.
        // We still need to strip trailing zeros.
        let mut s = d.normalize().to_string();
        if s.contains('.') {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
        s
    }
}
