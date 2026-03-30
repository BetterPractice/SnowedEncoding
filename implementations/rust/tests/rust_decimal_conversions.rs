/// Tests for the `rust_decimal` feature — `From` / `TryFrom` conversions and
/// canonical string formatting.
///
/// Run with: cargo test --features rust_decimal

use rust_decimal::Decimal;
use snowed::SnowedValue;
use std::str::FromStr;

// ── From<Decimal> for SnowedValue ─────────────────────────────────────────────

#[test]
fn from_decimal_zero() {
    assert_eq!(SnowedValue::from(Decimal::ZERO), SnowedValue::Decimal("0".into()));
}

#[test]
fn from_decimal_negative_zero() {
    // rust_decimal treats -0 as zero; canonical form is "0"
    let neg_zero = Decimal::from_str("-0").unwrap();
    assert_eq!(SnowedValue::from(neg_zero), SnowedValue::Decimal("0".into()));
}

#[test]
fn from_decimal_integer() {
    assert_eq!(SnowedValue::from(Decimal::from(42)), SnowedValue::Decimal("42".into()));
}

#[test]
fn from_decimal_negative_integer() {
    assert_eq!(SnowedValue::from(Decimal::from(-7)), SnowedValue::Decimal("-7".into()));
}

#[test]
fn from_decimal_fractional() {
    let d = Decimal::from_str("3.14").unwrap();
    assert_eq!(SnowedValue::from(d), SnowedValue::Decimal("3.14".into()));
}

#[test]
fn from_decimal_trailing_zeros_stripped() {
    // "3.10" must canonicalize to "3.1"
    let d = Decimal::from_str("3.10").unwrap();
    assert_eq!(SnowedValue::from(d), SnowedValue::Decimal("3.1".into()));
}

#[test]
fn from_decimal_all_fractional_zeros_stripped() {
    // "3.00" must canonicalize to "3"
    let d = Decimal::from_str("3.00").unwrap();
    assert_eq!(SnowedValue::from(d), SnowedValue::Decimal("3".into()));
}

#[test]
fn from_decimal_small_fractional() {
    let d = Decimal::from_str("0.001").unwrap();
    assert_eq!(SnowedValue::from(d), SnowedValue::Decimal("0.001".into()));
}

// ── TryFrom<SnowedValue> for Decimal ─────────────────────────────────────────

#[test]
fn try_from_snowed_decimal_succeeds() {
    let v = SnowedValue::Decimal("3.14".into());
    let d = Decimal::try_from(v).unwrap();
    assert_eq!(d, Decimal::from_str("3.14").unwrap());
}

#[test]
fn try_from_snowed_non_decimal_errors() {
    assert!(Decimal::try_from(SnowedValue::Int64(42)).is_err());
    assert!(Decimal::try_from(SnowedValue::Null).is_err());
    assert!(Decimal::try_from(SnowedValue::String("3.14".into())).is_err());
}

// ── Round-trip through encode / decode ───────────────────────────────────────

#[test]
fn round_trip_via_encode_decode() {
    use snowed::{decode, encode};

    let original = Decimal::from_str("2.71828").unwrap();
    let value = SnowedValue::from(original);
    let bytes = encode(&value);
    let decoded = decode(&bytes).unwrap();
    let back = Decimal::try_from(decoded).unwrap();
    assert_eq!(back, original);
}
