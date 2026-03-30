/// Serde round-trip tests for the Snowed serde feature.
///
/// Run with: cargo test --features serde

use serde::{Deserialize, Serialize};
use snowed::serde::{from_bytes, to_bytes, DATETIME_TOKEN, DECIMAL_TOKEN, UUID_TOKEN};

// ── Primitives ─────────────────────────────────────────────────────────────────

#[test]
fn bool_false() {
    let bytes = to_bytes(&false).unwrap();
    assert_eq!(bytes, [0x02]);
    let v: bool = from_bytes(&bytes).unwrap();
    assert!(!v);
}

#[test]
fn bool_true() {
    let bytes = to_bytes(&true).unwrap();
    assert_eq!(bytes, [0x03]);
    let v: bool = from_bytes(&bytes).unwrap();
    assert!(v);
}

#[test]
fn i64_roundtrip() {
    let bytes = to_bytes(&-42i64).unwrap();
    assert_eq!(bytes[0], 0x04);
    let v: i64 = from_bytes(&bytes).unwrap();
    assert_eq!(v, -42);
}

#[test]
fn u64_roundtrip() {
    let bytes = to_bytes(&9_999_999u64).unwrap();
    assert_eq!(bytes[0], 0x05);
    let v: u64 = from_bytes(&bytes).unwrap();
    assert_eq!(v, 9_999_999);
}

#[test]
fn f64_roundtrip() {
    let bytes = to_bytes(&3.14f64).unwrap();
    assert_eq!(bytes[0], 0x08);
    let v: f64 = from_bytes(&bytes).unwrap();
    assert_eq!(v, 3.14);
}

#[test]
fn str_roundtrip() {
    let bytes = to_bytes("hello").unwrap();
    assert_eq!(bytes[0], 0x11);
    let v: String = from_bytes(&bytes).unwrap();
    assert_eq!(v, "hello");
}

#[test]
fn bytes_roundtrip() {
    let input: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF];
    let bytes = to_bytes(serde_bytes::Bytes::new(input)).unwrap();
    assert_eq!(bytes[0], 0x10);
    let v: serde_bytes::ByteBuf = from_bytes(&bytes).unwrap();
    assert_eq!(v.as_slice(), input);
}

#[test]
fn null_from_none() {
    let v: Option<i64> = None;
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes, [0x00]);
    let back: Option<i64> = from_bytes(&bytes).unwrap();
    assert!(back.is_none());
}

#[test]
fn some_roundtrip() {
    let v: Option<i64> = Some(42);
    let bytes = to_bytes(&v).unwrap();
    let back: Option<i64> = from_bytes(&bytes).unwrap();
    assert_eq!(back, Some(42));
}

// ── Struct / Object ────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Point {
    x: i64,
    y: i64,
}

#[test]
fn struct_roundtrip() {
    let p = Point { x: 10, y: -5 };
    let bytes = to_bytes(&p).unwrap();
    assert_eq!(bytes[0], 0x80); // Object
    let back: Point = from_bytes(&bytes).unwrap();
    assert_eq!(back, p);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Outer {
    name: String,
    inner: Point,
    count: u64,
}

#[test]
fn nested_struct_roundtrip() {
    let v = Outer {
        name: "test".to_string(),
        inner: Point { x: 1, y: 2 },
        count: 99,
    };
    let bytes = to_bytes(&v).unwrap();
    let back: Outer = from_bytes(&bytes).unwrap();
    assert_eq!(back, v);
}

// ── Option field ───────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct MaybeVal {
    val: Option<i64>,
}

#[test]
fn option_none_field() {
    let v = MaybeVal { val: None };
    let bytes = to_bytes(&v).unwrap();
    let back: MaybeVal = from_bytes(&bytes).unwrap();
    assert_eq!(back, v);
}

#[test]
fn option_some_field() {
    let v = MaybeVal { val: Some(7) };
    let bytes = to_bytes(&v).unwrap();
    let back: MaybeVal = from_bytes(&bytes).unwrap();
    assert_eq!(back, v);
}

// ── Array / Vec ────────────────────────────────────────────────────────────────

#[test]
fn vec_roundtrip() {
    let v = vec![1i64, 2, 3, -99];
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes[0], 0x40); // Array
    let back: Vec<i64> = from_bytes(&bytes).unwrap();
    assert_eq!(back, v);
}

#[test]
fn nested_vec_roundtrip() {
    let v: Vec<Vec<i64>> = vec![vec![1, 2], vec![3, 4]];
    let bytes = to_bytes(&v).unwrap();
    let back: Vec<Vec<i64>> = from_bytes(&bytes).unwrap();
    assert_eq!(back, v);
}

// ── Enum variants ──────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Color {
    Red,
    Green,
    Blue,
}

#[test]
fn unit_variant_roundtrip() {
    for c in [Color::Red, Color::Green, Color::Blue] {
        let bytes = to_bytes(&c).unwrap();
        assert_eq!(bytes[0], 0x11); // String
        let back: Color = from_bytes(&bytes).unwrap();
        assert_eq!(back, c);
    }
}

// ── Special Snowed types via newtypes ──────────────────────────────────────────

// --- DateTime ---

struct SnowedDateTime(f64);

impl Serialize for SnowedDateTime {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(DATETIME_TOKEN, &self.0)
    }
}

impl<'de> Deserialize<'de> for SnowedDateTime {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = SnowedDateTime;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("DateTime f64")
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(SnowedDateTime(v))
            }
        }
        d.deserialize_newtype_struct(DATETIME_TOKEN, V)
    }
}

#[test]
fn datetime_roundtrip() {
    let dt = SnowedDateTime(1_700_000_000_000.0);
    let bytes = to_bytes(&dt).unwrap();
    assert_eq!(bytes[0], 0x09); // DateTime
    let back: SnowedDateTime = from_bytes(&bytes).unwrap();
    assert_eq!(back.0, dt.0);
}

// --- Decimal ---

struct SnowedDecimal(String);

impl Serialize for SnowedDecimal {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(DECIMAL_TOKEN, self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for SnowedDecimal {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = SnowedDecimal;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("Decimal str")
            }
            fn visit_borrowed_str<E: serde::de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
                Ok(SnowedDecimal(v.to_owned()))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(SnowedDecimal(v.to_owned()))
            }
        }
        d.deserialize_newtype_struct(DECIMAL_TOKEN, V)
    }
}

#[test]
fn decimal_roundtrip() {
    let d = SnowedDecimal("3.14159".to_owned());
    let bytes = to_bytes(&d).unwrap();
    assert_eq!(bytes[0], 0x12); // Decimal
    let back: SnowedDecimal = from_bytes(&bytes).unwrap();
    assert_eq!(back.0, "3.14159");
}

// --- UUID ---

struct SnowedUuid([u8; 16]);

impl Serialize for SnowedUuid {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(UUID_TOKEN, serde_bytes::Bytes::new(&self.0))
    }
}

impl<'de> Deserialize<'de> for SnowedUuid {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = SnowedUuid;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("16-byte UUID")
            }
            fn visit_borrowed_bytes<E: serde::de::Error>(self, v: &'de [u8]) -> Result<Self::Value, E> {
                v.try_into().map(SnowedUuid).map_err(|_| E::custom("expected 16 bytes"))
            }
            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                v.try_into().map(SnowedUuid).map_err(|_| E::custom("expected 16 bytes"))
            }
        }
        d.deserialize_newtype_struct(UUID_TOKEN, V)
    }
}

#[test]
fn uuid_roundtrip() {
    let uuid_bytes: [u8; 16] = [
        0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1,
        0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
    ];
    let u = SnowedUuid(uuid_bytes);
    let bytes = to_bytes(&u).unwrap();
    assert_eq!(bytes[0], 0x13); // UUID
    let back: SnowedUuid = from_bytes(&bytes).unwrap();
    assert_eq!(back.0, uuid_bytes);
}

// ── UUID_TOKEN: wrong inner type gives error ───────────────────────────────────

struct BadUuid;

impl Serialize for BadUuid {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Pass an i64 instead of bytes — should produce an error.
        s.serialize_newtype_struct(UUID_TOKEN, &42i64)
    }
}

#[test]
fn uuid_wrong_inner_type_errors() {
    assert!(to_bytes(&BadUuid).is_err());
}

// ── Small integer and float types ─────────────────────────────────────────────

#[test]
fn i8_roundtrip() {
    let bytes = to_bytes(&-5i8).unwrap();
    assert_eq!(bytes[0], 0x04);
    let v: i8 = from_bytes(&bytes).unwrap();
    assert_eq!(v, -5);
}

#[test]
fn i16_roundtrip() {
    let bytes = to_bytes(&1000i16).unwrap();
    assert_eq!(bytes[0], 0x04);
    let v: i16 = from_bytes(&bytes).unwrap();
    assert_eq!(v, 1000);
}

#[test]
fn i32_roundtrip() {
    let bytes = to_bytes(&-100_000i32).unwrap();
    assert_eq!(bytes[0], 0x04);
    let v: i32 = from_bytes(&bytes).unwrap();
    assert_eq!(v, -100_000);
}

#[test]
fn u8_roundtrip() {
    let bytes = to_bytes(&255u8).unwrap();
    assert_eq!(bytes[0], 0x05);
    let v: u8 = from_bytes(&bytes).unwrap();
    assert_eq!(v, 255);
}

#[test]
fn u16_roundtrip() {
    let bytes = to_bytes(&60_000u16).unwrap();
    assert_eq!(bytes[0], 0x05);
    let v: u16 = from_bytes(&bytes).unwrap();
    assert_eq!(v, 60_000);
}

#[test]
fn u32_roundtrip() {
    let bytes = to_bytes(&3_000_000_000u32).unwrap();
    assert_eq!(bytes[0], 0x05);
    let v: u32 = from_bytes(&bytes).unwrap();
    assert_eq!(v, 3_000_000_000);
}

#[test]
fn f32_roundtrip() {
    // f32 is widened to f64 on the wire; round-trips exactly for values
    // representable in f32 (like 1.5).
    let bytes = to_bytes(&1.5f32).unwrap();
    assert_eq!(bytes[0], 0x08);
    let v: f32 = from_bytes(&bytes).unwrap();
    assert_eq!(v, 1.5f32);
}

// ── char ───────────────────────────────────────────────────────────────────────

#[test]
fn char_roundtrip() {
    let bytes = to_bytes(&'Z').unwrap();
    assert_eq!(bytes[0], 0x11); // String
    let v: char = from_bytes(&bytes).unwrap();
    assert_eq!(v, 'Z');
}

#[test]
fn char_unicode_roundtrip() {
    let bytes = to_bytes(&'🌨').unwrap();
    assert_eq!(bytes[0], 0x11);
    let v: char = from_bytes(&bytes).unwrap();
    assert_eq!(v, '🌨');
}

#[test]
fn error_char_empty_string() {
    let bytes = to_bytes("").unwrap();
    assert!(from_bytes::<char>(&bytes).is_err());
}

#[test]
fn error_char_multi_char_string() {
    let bytes = to_bytes("ab").unwrap();
    assert!(from_bytes::<char>(&bytes).is_err());
}

// ── unit / unit_struct ────────────────────────────────────────────────────────

#[test]
fn unit_roundtrip() {
    let bytes = to_bytes(&()).unwrap();
    assert_eq!(bytes, [0x00]);
    let _: () = from_bytes(&bytes).unwrap();
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct UnitStruct;

#[test]
fn unit_struct_roundtrip() {
    let bytes = to_bytes(&UnitStruct).unwrap();
    assert_eq!(bytes, [0x00]);
    let back: UnitStruct = from_bytes(&bytes).unwrap();
    assert_eq!(back, UnitStruct);
}

// ── Tuple / tuple struct / tuple variant ──────────────────────────────────────

#[test]
fn tuple_roundtrip() {
    let t = (-1i64, true, "hi".to_string());
    let bytes = to_bytes(&t).unwrap();
    assert_eq!(bytes[0], 0x40); // Array
    let back: (i64, bool, String) = from_bytes(&bytes).unwrap();
    assert_eq!(back, t);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Pair(i64, bool);

#[test]
fn tuple_struct_roundtrip() {
    let p = Pair(7, false);
    let bytes = to_bytes(&p).unwrap();
    assert_eq!(bytes[0], 0x40); // Array
    let back: Pair = from_bytes(&bytes).unwrap();
    assert_eq!(back, p);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Tagged {
    Point(i64, i64),
}

#[test]
fn tuple_variant_roundtrip() {
    let v = Tagged::Point(3, 4);
    let bytes = to_bytes(&v).unwrap();
    assert_eq!(bytes[0], 0x80); // outer Object: { "Point": [3, 4] }
    let back: Tagged = from_bytes(&bytes).unwrap();
    assert_eq!(back, v);
}

// ── Newtype variant ───────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Wrapper {
    Int(i64),
    Text(String),
}

#[test]
fn newtype_variant_roundtrip() {
    for v in [Wrapper::Int(42), Wrapper::Text("hello".into())] {
        let bytes = to_bytes(&v).unwrap();
        assert_eq!(bytes[0], 0x80); // Object: { "Int": 42 } or { "Text": "hello" }
        let back: Wrapper = from_bytes(&bytes).unwrap();
        assert_eq!(back, v);
    }
}

// ── Struct variant ────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Shape {
    Circle { radius: f64 },
    Rect { width: f64, height: f64 },
}

#[test]
fn struct_variant_roundtrip() {
    for v in [
        Shape::Circle { radius: 2.5 },
        Shape::Rect { width: 10.0, height: 4.0 },
    ] {
        let bytes = to_bytes(&v).unwrap();
        assert_eq!(bytes[0], 0x80); // outer Object: { "Circle": { "radius": 2.5 } }
        let back: Shape = from_bytes(&bytes).unwrap();
        assert_eq!(back, v);
    }
}

// ── Map (serialize_map / deserialize_map) ─────────────────────────────────────

#[test]
fn btreemap_roundtrip() {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert("alpha".to_string(), 1i64);
    m.insert("beta".to_string(), -2i64);
    let bytes = to_bytes(&m).unwrap();
    assert_eq!(bytes[0], 0x80); // Object
    let back: BTreeMap<String, i64> = from_bytes(&bytes).unwrap();
    assert_eq!(back, m);
}

// ── Serializer error: key exceeds 255 bytes ───────────────────────────────────

#[test]
fn error_key_too_long() {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert("x".repeat(256), 0i64);
    assert!(to_bytes(&m).is_err());
}

// ── Ignored / unknown fields (exercises skip_value / deserialize_ignored_any) ─

#[derive(Serialize)]
struct Full {
    x: i64,
    y: i64,
    extra: String,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Partial {
    x: i64,
    y: i64,
}

#[test]
fn ignored_field_skipped() {
    let full = Full { x: 1, y: 2, extra: "ignore me".into() };
    let bytes = to_bytes(&full).unwrap();
    let back: Partial = from_bytes(&bytes).unwrap();
    assert_eq!(back, Partial { x: 1, y: 2 });
}

// ── Interop: serde output matches SnowedValue encode output ───────────────────

#[test]
fn serde_bytes_match_snowed_value_encode() {
    use snowed::{encode, SnowedValue};
    use std::collections::BTreeMap;

    let p = Point { x: 7, y: -3 };
    let serde_bytes = to_bytes(&p).unwrap();

    let mut map = BTreeMap::new();
    map.insert("x".to_string(), SnowedValue::Int64(7));
    map.insert("y".to_string(), SnowedValue::Int64(-3));
    let value_bytes = encode(&SnowedValue::Object(map));

    assert_eq!(serde_bytes, value_bytes);
}
