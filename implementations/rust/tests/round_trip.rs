extern crate alloc;

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use snowed::{decode, encode, DecodeError, SnowedValue};

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

fn round_trip(value: &SnowedValue) -> SnowedValue {
    let encoded = encode(value);
    decode(&encoded).expect("round-trip decode failed")
}

fn assert_bytes(value: &SnowedValue, expected_hex: &str) {
    let encoded = encode(value);
    let expected: Vec<u8> = expected_hex
        .split_whitespace()
        .map(|h| u8::from_str_radix(h, 16).unwrap())
        .collect();
    assert_eq!(
        encoded, expected,
        "encoded bytes mismatch for {:?}",
        value
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Round-trip tests
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn null_round_trip() {
    assert_eq!(round_trip(&SnowedValue::Null), SnowedValue::Null);
}

#[test]
fn bool_false_round_trip() {
    assert_eq!(round_trip(&SnowedValue::Bool(false)), SnowedValue::Bool(false));
}

#[test]
fn bool_true_round_trip() {
    assert_eq!(round_trip(&SnowedValue::Bool(true)), SnowedValue::Bool(true));
}

#[test]
fn int64_zero() {
    assert_eq!(round_trip(&SnowedValue::Int64(0)), SnowedValue::Int64(0));
}

#[test]
fn int64_positive() {
    assert_eq!(round_trip(&SnowedValue::Int64(42)), SnowedValue::Int64(42));
}

#[test]
fn int64_negative() {
    assert_eq!(round_trip(&SnowedValue::Int64(-1)), SnowedValue::Int64(-1));
}

#[test]
fn int64_min() {
    assert_eq!(round_trip(&SnowedValue::Int64(i64::MIN)), SnowedValue::Int64(i64::MIN));
}

#[test]
fn int64_max() {
    assert_eq!(round_trip(&SnowedValue::Int64(i64::MAX)), SnowedValue::Int64(i64::MAX));
}

#[test]
fn uint64_zero() {
    assert_eq!(round_trip(&SnowedValue::UInt64(0)), SnowedValue::UInt64(0));
}

#[test]
fn uint64_max() {
    assert_eq!(round_trip(&SnowedValue::UInt64(u64::MAX)), SnowedValue::UInt64(u64::MAX));
}

#[test]
fn uint64_above_i64_max() {
    let v = i64::MAX as u64 + 1;
    assert_eq!(round_trip(&SnowedValue::UInt64(v)), SnowedValue::UInt64(v));
}

#[test]
fn float64_pi() {
    assert_eq!(round_trip(&SnowedValue::Float64(3.14)), SnowedValue::Float64(3.14));
}

#[test]
fn float64_negative_zero() {
    let rt = round_trip(&SnowedValue::Float64(-0.0_f64));
    match rt {
        SnowedValue::Float64(v) => assert!(v.is_sign_negative(), "expected -0.0"),
        _ => panic!("expected Float64"),
    }
}

#[test]
fn float64_infinity() {
    assert_eq!(round_trip(&SnowedValue::Float64(f64::INFINITY)), SnowedValue::Float64(f64::INFINITY));
    assert_eq!(round_trip(&SnowedValue::Float64(f64::NEG_INFINITY)), SnowedValue::Float64(f64::NEG_INFINITY));
}

#[test]
fn float64_nan() {
    let rt = round_trip(&SnowedValue::Float64(f64::NAN));
    match rt {
        SnowedValue::Float64(v) => assert!(v.is_nan()),
        _ => panic!("expected Float64"),
    }
}

#[test]
fn datetime_epoch() {
    assert_eq!(round_trip(&SnowedValue::DateTime(0.0)), SnowedValue::DateTime(0.0));
}

#[test]
fn datetime_positive() {
    let ms = 1_700_000_000_000.0_f64;
    assert_eq!(round_trip(&SnowedValue::DateTime(ms)), SnowedValue::DateTime(ms));
}

#[test]
fn datetime_negative() {
    let ms = -1_000_000.0_f64;
    assert_eq!(round_trip(&SnowedValue::DateTime(ms)), SnowedValue::DateTime(ms));
}

#[test]
fn binary_empty() {
    assert_eq!(round_trip(&SnowedValue::Binary(vec![])), SnowedValue::Binary(vec![]));
}

#[test]
fn binary_payload() {
    let bytes = vec![0xDE, 0xAD, 0xBE, 0xEF];
    assert_eq!(round_trip(&SnowedValue::Binary(bytes.clone())), SnowedValue::Binary(bytes));
}

#[test]
fn string_empty() {
    assert_eq!(
        round_trip(&SnowedValue::String(String::new())),
        SnowedValue::String(String::new())
    );
}

#[test]
fn string_hello() {
    let s = "hello".to_string();
    assert_eq!(round_trip(&SnowedValue::String(s.clone())), SnowedValue::String(s));
}

#[test]
fn string_unicode() {
    let s = "こんにちは🌨️".to_string();
    assert_eq!(round_trip(&SnowedValue::String(s.clone())), SnowedValue::String(s));
}

#[test]
fn decimal_integer() {
    let d = SnowedValue::Decimal("42".to_string());
    assert_eq!(round_trip(&d), d);
}

#[test]
fn decimal_fractional() {
    let d = SnowedValue::Decimal("3.14159".to_string());
    assert_eq!(round_trip(&d), d);
}

#[test]
fn decimal_negative() {
    let d = SnowedValue::Decimal("-0.001".to_string());
    assert_eq!(round_trip(&d), d);
}

#[test]
fn decimal_zero() {
    let d = SnowedValue::Decimal("0".to_string());
    assert_eq!(round_trip(&d), d);
}

#[test]
fn uuid_round_trip() {
    // 550e8400-e29b-41d4-a716-446655440000
    let bytes: [u8; 16] = [
        0x55, 0x0e, 0x84, 0x00,
        0xe2, 0x9b,
        0x41, 0xd4,
        0xa7, 0x16,
        0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
    ];
    assert_eq!(round_trip(&SnowedValue::Uuid(bytes)), SnowedValue::Uuid(bytes));
}

#[test]
fn array_empty() {
    assert_eq!(round_trip(&SnowedValue::Array(vec![])), SnowedValue::Array(vec![]));
}

#[test]
fn array_mixed() {
    let arr = SnowedValue::Array(vec![
        SnowedValue::Bool(true),
        SnowedValue::Int64(42),
        SnowedValue::String("hi".to_string()),
    ]);
    assert_eq!(round_trip(&arr), arr);
}

#[test]
fn array_nested() {
    let inner = SnowedValue::Array(vec![SnowedValue::Int64(1), SnowedValue::Int64(2)]);
    let outer = SnowedValue::Array(vec![inner, SnowedValue::Null]);
    assert_eq!(round_trip(&outer), outer);
}

#[test]
fn object_empty() {
    assert_eq!(
        round_trip(&SnowedValue::Object(BTreeMap::new())),
        SnowedValue::Object(BTreeMap::new())
    );
}

#[test]
fn object_single_key() {
    let mut map = BTreeMap::new();
    map.insert("x".to_string(), SnowedValue::Int64(1));
    let obj = SnowedValue::Object(map);
    assert_eq!(round_trip(&obj), obj);
}

#[test]
fn object_nested() {
    let mut inner = BTreeMap::new();
    inner.insert("a".to_string(), SnowedValue::Bool(true));
    let mut outer = BTreeMap::new();
    outer.insert("inner".to_string(), SnowedValue::Object(inner));
    outer.insert("n".to_string(), SnowedValue::Null);
    let obj = SnowedValue::Object(outer);
    assert_eq!(round_trip(&obj), obj);
}

// ────────────────────────────────────────────────────────────────────────────
// Wire-byte tests (spec §5 examples)
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn wire_null() {
    assert_bytes(&SnowedValue::Null, "00");
}

#[test]
fn wire_bool_false() {
    assert_bytes(&SnowedValue::Bool(false), "02");
}

#[test]
fn wire_bool_true() {
    assert_bytes(&SnowedValue::Bool(true), "03");
}

#[test]
fn wire_int64_42() {
    assert_bytes(&SnowedValue::Int64(42), "04 2A 00 00 00 00 00 00 00");
}

#[test]
fn wire_int64_minus1() {
    assert_bytes(&SnowedValue::Int64(-1), "04 FF FF FF FF FF FF FF FF");
}

#[test]
fn wire_uint64_max() {
    assert_bytes(&SnowedValue::UInt64(u64::MAX), "05 FF FF FF FF FF FF FF FF");
}

#[test]
fn wire_float64_314() {
    // 3.14 in IEEE 754 LE
    assert_bytes(&SnowedValue::Float64(3.14), "08 1F 85 EB 51 B8 1E 09 40");
}

#[test]
fn wire_binary_deadbeef() {
    assert_bytes(
        &SnowedValue::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        "10 04 00 00 00 00 00 00 00 DE AD BE EF",
    );
}

#[test]
fn wire_string_hello() {
    assert_bytes(
        &SnowedValue::String("hello".to_string()),
        "11 05 00 00 00 00 00 00 00 68 65 6C 6C 6F",
    );
}

#[test]
fn wire_uuid() {
    // 550e8400-e29b-41d4-a716-446655440000
    let bytes: [u8; 16] = [
        0x55, 0x0e, 0x84, 0x00,
        0xe2, 0x9b,
        0x41, 0xd4,
        0xa7, 0x16,
        0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
    ];
    assert_bytes(
        &SnowedValue::Uuid(bytes),
        "13 10 00 00 00 00 00 00 00 55 0E 84 00 E2 9B 41 D4 A7 16 44 66 55 44 00 00",
    );
}

#[test]
fn wire_empty_array() {
    assert_bytes(&SnowedValue::Array(vec![]), "40 FF");
}

#[test]
fn wire_array_true_42() {
    assert_bytes(
        &SnowedValue::Array(vec![SnowedValue::Bool(true), SnowedValue::Int64(42)]),
        "40 03 04 2A 00 00 00 00 00 00 00 FF",
    );
}

#[test]
fn wire_empty_object() {
    assert_bytes(&SnowedValue::Object(BTreeMap::new()), "80 00");
}

#[test]
fn wire_object_x_1() {
    let mut map = BTreeMap::new();
    map.insert("x".to_string(), SnowedValue::Int64(1));
    assert_bytes(
        &SnowedValue::Object(map),
        "80 01 78 04 01 00 00 00 00 00 00 00 00",
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Forward-compatibility tests (§4.4)
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn forward_compat_unknown_null_subtype_0x01() {
    // 0x01 → Null family → decode as Null
    let bytes = [0x01u8];
    assert_eq!(decode(&bytes).unwrap(), SnowedValue::Null);
}

#[test]
fn forward_compat_unknown_int_subtype_0x06() {
    // 0x06 → Int family → decode as Int64
    let mut bytes = vec![0x06u8];
    bytes.extend_from_slice(&42i64.to_le_bytes());
    assert_eq!(decode(&bytes).unwrap(), SnowedValue::Int64(42));
}

#[test]
fn forward_compat_unknown_float_subtype_0x0a() {
    // 0x0A → Float family → decode as Float64
    let mut bytes = vec![0x0Au8];
    bytes.extend_from_slice(&3.14f64.to_bits().to_le_bytes());
    assert_eq!(decode(&bytes).unwrap(), SnowedValue::Float64(3.14));
}

#[test]
fn forward_compat_unknown_binary_subtype_0x14() {
    // 0x14 → Binary family → decode as Binary
    let payload = b"test";
    let mut bytes = vec![0x14u8];
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(payload);
    assert_eq!(decode(&bytes).unwrap(), SnowedValue::Binary(payload.to_vec()));
}

#[test]
fn forward_compat_unknown_array_subtype_0x41() {
    // 0x41 → Array family → decode as Array
    let bytes = [0x41u8, 0x03u8, 0xFFu8]; // array [true] with unknown array type byte
    let result = decode(&bytes).unwrap();
    assert_eq!(result, SnowedValue::Array(vec![SnowedValue::Bool(true)]));
}

#[test]
fn forward_compat_unknown_object_subtype_0xc0() {
    // 0xC0 → Object family → decode as Object
    let mut bytes = vec![0xC0u8];
    // key "x"
    bytes.push(0x01);
    bytes.push(b'x');
    // value Int64(1)
    bytes.push(0x04);
    bytes.extend_from_slice(&1i64.to_le_bytes());
    // terminate
    bytes.push(0x00);
    let mut expected = BTreeMap::new();
    expected.insert("x".to_string(), SnowedValue::Int64(1));
    assert_eq!(decode(&bytes).unwrap(), SnowedValue::Object(expected));
}

// ────────────────────────────────────────────────────────────────────────────
// Error tests
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn error_unexpected_eof() {
    // Int64 type byte with only 4 bytes of payload (need 8)
    let bytes = [0x04u8, 0x00, 0x00, 0x00, 0x00];
    assert_eq!(decode(&bytes), Err(DecodeError::UnexpectedEof));
}

#[test]
fn error_unexpected_sentinel_as_root() {
    assert_eq!(decode(&[0xFF]), Err(DecodeError::UnexpectedSentinel));
}

#[test]
fn error_invalid_utf8_string() {
    let mut bytes = vec![0x11u8];
    let payload = [0xFF, 0xFE]; // invalid UTF-8
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&payload);
    assert_eq!(decode(&bytes), Err(DecodeError::InvalidUtf8));
}

#[test]
fn error_uuid_wrong_length() {
    let mut bytes = vec![0x13u8];
    let len: u64 = 15;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(&[0u8; 15]);
    assert_eq!(decode(&bytes), Err(DecodeError::InvalidUuidLength(15)));
}

#[test]
fn error_trailing_bytes() {
    let mut bytes = encode(&SnowedValue::Null);
    bytes.push(0x00); // extra byte
    assert_eq!(decode(&bytes), Err(DecodeError::TrailingBytes));
}

#[test]
fn error_invalid_decimal_string() {
    // 0x12 payload with invalid UTF-8
    let mut bytes = vec![0x12u8];
    let payload = [0xFF, 0xFE];
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&payload);
    assert_eq!(decode(&bytes), Err(DecodeError::InvalidDecimalString));
}

#[test]
fn error_invalid_utf8_object_key() {
    // Object with a 2-byte key that is invalid UTF-8
    let mut bytes = vec![0x80u8, 0x02, 0xFF, 0xFE];
    // Need a value byte after the key; error happens at key validation
    bytes.push(0x00); // would be value type, but decode fails on the key first
    assert_eq!(decode(&bytes), Err(DecodeError::InvalidUtf8));
}

#[test]
fn object_sentinel_in_value_position() {
    // 0x80 + key "x" (len=1) + 0xFF in value position → terminates the object
    let bytes = [0x80u8, 0x01, b'x', 0xFF];
    assert_eq!(decode(&bytes).unwrap(), SnowedValue::Object(BTreeMap::new()));
}
