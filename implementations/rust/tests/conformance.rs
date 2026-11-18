//! Cross-implementation conformance runner.
//!
//! Reads every vector file in `tests/conformance/vectors/` (relative to the
//! repository root) and checks decoding, semantic round-trip, canonical
//! re-encoding, and error cases as described in `tests/conformance/README.md`.

use std::fs;
use std::path::PathBuf;

use serde_json::Value as Json;
use snowed::{decode, encode, SnowedValue};

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/vectors")
}

fn parse_hex(hex: &str) -> Result<Vec<u8>, String> {
    hex.split_whitespace()
        .map(|h| u8::from_str_radix(h, 16).map_err(|e| format!("bad hex byte {h:?}: {e}")))
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 character {:?}", c as char)),
        }
    }
    let bytes = s.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err(format!("base64 length {} is not a multiple of 4", bytes.len()));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (i, chunk) in bytes.chunks(4).enumerate() {
        let last = i == bytes.len() / 4 - 1;
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return Err("invalid base64 padding".into());
        }
        let mut n: u32 = 0;
        for &c in &chunk[..4 - pad] {
            n = (n << 6) | val(c)?;
        }
        n <<= 6 * pad as u32;
        let group = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&group[..3 - pad]);
    }
    Ok(out)
}

fn format_uuid(b: &[u8; 16]) -> String {
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

fn type_name(v: &SnowedValue) -> &'static str {
    match v {
        SnowedValue::Null => "null",
        SnowedValue::Bool(_) => "bool",
        SnowedValue::Int64(_) => "int64",
        SnowedValue::UInt64(_) => "uint64",
        SnowedValue::Float64(_) => "float64",
        SnowedValue::DateTime(_) => "datetime",
        SnowedValue::Binary(_) => "binary",
        SnowedValue::String(_) => "string",
        SnowedValue::Decimal(_) => "decimal",
        SnowedValue::Uuid(_) => "uuid",
        SnowedValue::Array(_) => "array",
        SnowedValue::Object(_) => "object",
    }
}

fn expect_i64(v: &Json) -> Result<i64, String> {
    match v {
        Json::String(s) => s.parse::<i64>().map_err(|e| format!("bad int64 string {s:?}: {e}")),
        Json::Number(n) => n.as_i64().ok_or_else(|| format!("int64 value {n} not an i64")),
        _ => Err(format!("int64 value must be number or string, got {v}")),
    }
}

fn expect_u64(v: &Json) -> Result<u64, String> {
    match v {
        Json::String(s) => s.parse::<u64>().map_err(|e| format!("bad uint64 string {s:?}: {e}")),
        Json::Number(n) => n.as_u64().ok_or_else(|| format!("uint64 value {n} not a u64")),
        _ => Err(format!("uint64 value must be number or string, got {v}")),
    }
}

/// Parsed expectation for a float: either a concrete number (bitwise
/// compared, so -0 vs 0 is distinguished) or "any NaN".
enum FloatExpect {
    Nan,
    Value(f64),
}

fn expect_f64(v: &Json) -> Result<FloatExpect, String> {
    match v {
        Json::String(s) => match s.as_str() {
            "NaN" => Ok(FloatExpect::Nan),
            "Infinity" => Ok(FloatExpect::Value(f64::INFINITY)),
            "-Infinity" => Ok(FloatExpect::Value(f64::NEG_INFINITY)),
            "-0" => Ok(FloatExpect::Value(-0.0)),
            _ => Err(format!("unknown float string {s:?}")),
        },
        Json::Number(n) => n
            .as_f64()
            .map(FloatExpect::Value)
            .ok_or_else(|| format!("float value {n} not representable")),
        _ => Err(format!("float value must be number or string, got {v}")),
    }
}

fn check_float(expected: &Json, actual: f64, path: &str) -> Result<(), String> {
    match expect_f64(expected)? {
        FloatExpect::Nan => {
            if actual.is_nan() {
                Ok(())
            } else {
                Err(format!("{path}: expected NaN, got {actual:?}"))
            }
        }
        FloatExpect::Value(e) => {
            // Bitwise compare: distinguishes 0 from -0 and is exact.
            if e.to_bits() == actual.to_bits() {
                Ok(())
            } else {
                Err(format!(
                    "{path}: expected {e:?} (bits {:#018x}), got {actual:?} (bits {:#018x})",
                    e.to_bits(),
                    actual.to_bits()
                ))
            }
        }
    }
}

/// Compare a decoded value against a `{"type": ..., "value": ...}` expectation.
fn check(expected: &Json, actual: &SnowedValue, path: &str) -> Result<(), String> {
    let ty = expected
        .get("type")
        .and_then(Json::as_str)
        .ok_or_else(|| format!("{path}: expectation missing \"type\""))?;
    let ev = expected.get("value").unwrap_or(&Json::Null);

    let mismatch = || {
        Err(format!(
            "{path}: expected type {ty}, got {} ({actual:?})",
            type_name(actual)
        ))
    };

    match (ty, actual) {
        ("null", SnowedValue::Null) => Ok(()),
        ("bool", SnowedValue::Bool(b)) => {
            let e = ev.as_bool().ok_or_else(|| format!("{path}: bool value not a boolean"))?;
            if e == *b { Ok(()) } else { Err(format!("{path}: expected {e}, got {b}")) }
        }
        ("int64", SnowedValue::Int64(n)) => {
            let e = expect_i64(ev).map_err(|m| format!("{path}: {m}"))?;
            if e == *n { Ok(()) } else { Err(format!("{path}: expected int64 {e}, got {n}")) }
        }
        ("uint64", SnowedValue::UInt64(n)) => {
            let e = expect_u64(ev).map_err(|m| format!("{path}: {m}"))?;
            if e == *n { Ok(()) } else { Err(format!("{path}: expected uint64 {e}, got {n}")) }
        }
        ("float64", SnowedValue::Float64(f)) => {
            check_float(ev, *f, path).map_err(|m| m.replacen(path, &format!("{path} (float64)"), 1))
        }
        ("datetime", SnowedValue::DateTime(f)) => {
            check_float(ev, *f, path).map_err(|m| m.replacen(path, &format!("{path} (datetime)"), 1))
        }
        ("binary", SnowedValue::Binary(b)) => {
            let s = ev.as_str().ok_or_else(|| format!("{path}: binary value not a string"))?;
            let e = base64_decode(s).map_err(|m| format!("{path}: {m}"))?;
            if e == *b {
                Ok(())
            } else {
                Err(format!("{path}: expected binary [{}], got [{}]", to_hex(&e), to_hex(b)))
            }
        }
        ("string", SnowedValue::String(s)) => {
            let e = ev.as_str().ok_or_else(|| format!("{path}: string value not a string"))?;
            if e == s { Ok(()) } else { Err(format!("{path}: expected string {e:?}, got {s:?}")) }
        }
        ("decimal", SnowedValue::Decimal(s)) => {
            let e = ev.as_str().ok_or_else(|| format!("{path}: decimal value not a string"))?;
            if e == s { Ok(()) } else { Err(format!("{path}: expected decimal {e:?}, got {s:?}")) }
        }
        ("uuid", SnowedValue::Uuid(u)) => {
            let e = ev.as_str().ok_or_else(|| format!("{path}: uuid value not a string"))?;
            let a = format_uuid(u);
            if e == a { Ok(()) } else { Err(format!("{path}: expected uuid {e}, got {a}")) }
        }
        ("array", SnowedValue::Array(items)) => {
            let e = ev.as_array().ok_or_else(|| format!("{path}: array value not an array"))?;
            if e.len() != items.len() {
                return Err(format!(
                    "{path}: expected array of {} elements, got {}",
                    e.len(),
                    items.len()
                ));
            }
            for (i, (ei, ai)) in e.iter().zip(items).enumerate() {
                check(ei, ai, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        ("object", SnowedValue::Object(map)) => {
            let e = ev.as_object().ok_or_else(|| format!("{path}: object value not an object"))?;
            if e.len() != map.len() {
                let ek: Vec<_> = e.keys().collect();
                let ak: Vec<_> = map.keys().collect();
                return Err(format!("{path}: expected keys {ek:?}, got {ak:?}"));
            }
            for (k, ekv) in e {
                let akv = map
                    .get(k)
                    .ok_or_else(|| format!("{path}: missing key {k:?}"))?;
                check(ekv, akv, &format!("{path}.{k}"))?;
            }
            Ok(())
        }
        (
            "null" | "bool" | "int64" | "uint64" | "float64" | "datetime" | "binary" | "string"
            | "decimal" | "uuid" | "array" | "object",
            _,
        ) => mismatch(),
        _ => Err(format!("{path}: unknown expected type {ty:?}")),
    }
}

fn run_case(case: &Json) -> Result<(), String> {
    let hex = case
        .get("hex")
        .and_then(Json::as_str)
        .ok_or("case missing \"hex\"")?;
    let bytes = parse_hex(hex)?;

    if case.get("error").and_then(Json::as_bool) == Some(true) {
        return match decode(&bytes) {
            Err(_) => Ok(()),
            Ok(v) => Err(format!("expected decode error, got Ok({v:?})")),
        };
    }

    let expected = case
        .get("decoded")
        .ok_or("case has neither \"decoded\" nor \"error\": true")?;
    let canonical = case.get("canonical").and_then(Json::as_bool).unwrap_or(true);

    // 1. Decode and compare.
    let value = decode(&bytes).map_err(|e| format!("decode failed: {e:?}"))?;
    check(expected, &value, "decoded")?;

    // 2. Semantic round-trip.
    let reencoded = encode(&value);
    let value2 = decode(&reencoded).map_err(|e| {
        format!("re-decode failed: {e:?} (re-encoded bytes: {})", to_hex(&reencoded))
    })?;
    check(expected, &value2, "round-trip")?;

    // 3. Canonical byte-exact re-encode.
    if canonical && reencoded != bytes {
        return Err(format!(
            "canonical re-encode mismatch: expected [{}], got [{}]",
            to_hex(&bytes),
            to_hex(&reencoded)
        ));
    }
    Ok(())
}

// ────────────────────────────────────────────────────────────────────────────
// Test
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn conformance_vectors() {
    let dir = vectors_dir();
    let entries = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("conformance vectors directory {} not readable: {e}", dir.display()));

    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map_or(false, |x| x == "json"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no .json conformance vectors found in {}",
        dir.display()
    );

    let mut failures: Vec<String> = Vec::new();
    let mut total = 0usize;

    for file in &files {
        let fname = file.file_name().unwrap().to_string_lossy().into_owned();
        let text = match fs::read_to_string(file) {
            Ok(t) => t,
            Err(e) => {
                failures.push(format!("{fname}: unreadable: {e}"));
                continue;
            }
        };
        let doc: Json = match serde_json::from_str(&text) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("{fname}: invalid JSON: {e}"));
                continue;
            }
        };
        let suite = doc
            .get("suite")
            .and_then(Json::as_str)
            .unwrap_or(&fname)
            .to_string();
        let Some(cases) = doc.get("cases").and_then(Json::as_array) else {
            failures.push(format!("{suite}: missing \"cases\" array"));
            continue;
        };
        if cases.is_empty() {
            failures.push(format!("{suite}: \"cases\" is empty"));
        }
        for (i, case) in cases.iter().enumerate() {
            total += 1;
            let name = case
                .get("name")
                .and_then(Json::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("#{i}"));
            if let Err(msg) = run_case(case) {
                failures.push(format!("{suite}/{name}: {msg}"));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} of {} conformance cases failed:\n  {}",
            failures.len(),
            total,
            failures.join("\n  ")
        );
    }
    eprintln!("conformance: {total} cases across {} files passed", files.len());
}

#[test]
fn base64_helper() {
    assert_eq!(base64_decode("").unwrap(), b"");
    assert_eq!(base64_decode("Zg==").unwrap(), b"f");
    assert_eq!(base64_decode("Zm8=").unwrap(), b"fo");
    assert_eq!(base64_decode("Zm9v").unwrap(), b"foo");
    assert_eq!(base64_decode("Zm9vYmFy").unwrap(), b"foobar");
    assert_eq!(base64_decode("AP8=").unwrap(), [0x00, 0xFF]);
    assert!(base64_decode("Zm9").is_err());
}
