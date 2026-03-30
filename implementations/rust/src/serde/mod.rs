/// Marker name used to serialize a UUID via `serialize_newtype_struct`.
///
/// ```rust,ignore
/// serializer.serialize_newtype_struct(snowed::serde::UUID_TOKEN, &my_16_bytes)
/// ```
pub const UUID_TOKEN: &str = "$snowed::uuid";

/// Marker name used to serialize a DateTime (milliseconds since Unix epoch)
/// via `serialize_newtype_struct`.
pub const DATETIME_TOKEN: &str = "$snowed::datetime";

/// Marker name used to serialize a Decimal string
/// via `serialize_newtype_struct`.
pub const DECIMAL_TOKEN: &str = "$snowed::decimal";

mod ser;
mod de;

pub use ser::{to_bytes, Serializer, SerError};
pub use de::{from_bytes, Deserializer, DeError};
