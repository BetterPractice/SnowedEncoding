#!/usr/bin/env python3
"""Generate the Snowed conformance vectors in tests/conformance/vectors/.

The bytes are built directly with `struct` from the wire format in spec/snowed-spec.md, so the
vectors don't depend on any implementation's encoder. Standard library only (Python 3.8+).

Usage:
    python3 tests/conformance/generate_vectors.py [output_dir]

output_dir defaults to the vectors/ directory next to this script. Edit the case lists below and
re-run to change the suite; commit the regenerated JSON alongside the script change.
"""
import base64, json, struct, sys, uuid
from pathlib import Path

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent / "vectors"

def i64(n): return bytes([0x04]) + struct.pack("<q", n)
def u64(n): return bytes([0x05]) + struct.pack("<Q", n)
def f64(x, ty=0x08): return bytes([ty]) + struct.pack("<d", x)
def blob(ty, b): return bytes([ty]) + struct.pack("<Q", len(b)) + b
def arr(*els, ty=0x40): return bytes([ty]) + b"".join(els) + b"\xff"
def key(k): kb = k.encode(); return bytes([len(kb)]) + kb
def obj(*pairs, ty=0x80): return bytes([ty]) + b"".join(key(k) + v for k, v in pairs) + b"\x00"

def hx(b): return " ".join(f"{x:02X}" for x in b)

def intval(n): return n if abs(n) <= 2**53 else str(n)

N = {"type": "null", "value": None}
def B(v): return {"type": "bool", "value": v}
def I(n): return {"type": "int64", "value": intval(n)}
def U(n): return {"type": "uint64", "value": intval(n)}
def F(x): return {"type": "float64", "value": x}
def D(ms): return {"type": "datetime", "value": ms}
def BIN(b): return {"type": "binary", "value": base64.b64encode(b).decode()}
def S(s): return {"type": "string", "value": s}
def DEC(s): return {"type": "decimal", "value": s}
def UU(s): return {"type": "uuid", "value": s}
def A(*v): return {"type": "array", "value": list(v)}
def O(**kv): return {"type": "object", "value": kv}
def Od(d): return {"type": "object", "value": d}

def case(name, desc, data, decoded=None, canonical=True, error=False):
    c = {"name": name, "description": desc, "hex": hx(data)}
    if error:
        c["error"] = True
    else:
        c["decoded"] = decoded
        if not canonical:
            c["canonical"] = False
    return c

suites = {}

suites["null"] = [case("null", "Null value (0x00)", b"\x00", N)]

suites["boolean"] = [
    case("false", "Boolean False (0x02)", b"\x02", B(False)),
    case("true", "Boolean True (0x03)", b"\x03", B(True)),
]

suites["int64"] = [
    case("zero", "Int64 0", i64(0), I(0)),
    case("positive", "Int64 42 (spec §5.3 example)", i64(42), I(42)),
    case("negative_one", "Int64 -1 (spec §5.3 example)", i64(-1), I(-1)),
    case("byte_order", "Int64 0x0102030405060708 checks little-endian layout", i64(0x0102030405060708), I(0x0102030405060708)),
    case("max_safe_double", "Int64 2^53", i64(2**53), I(2**53)),
    case("above_safe_double", "Int64 2^53 + 1 (not representable as a double)", i64(2**53 + 1), I(2**53 + 1)),
    case("max", "Int64 max", i64(2**63 - 1), I(2**63 - 1)),
    case("min", "Int64 min", i64(-2**63), I(-2**63)),
]

suites["uint64"] = [
    case("zero", "UInt64 0", u64(0), U(0)),
    case("positive", "UInt64 42", u64(42), U(42)),
    case("int64_max", "UInt64 equal to Int64 max", u64(2**63 - 1), U(2**63 - 1)),
    case("above_int64_max", "UInt64 2^63 (exceeds Int64 range)", u64(2**63), U(2**63)),
    case("max", "UInt64 max (spec §5.4 example)", u64(2**64 - 1), U(2**64 - 1)),
]

suites["float64"] = [
    case("zero", "Float64 0.0", f64(0.0), F(0.0)),
    case("negative_zero", "Float64 -0.0 (sign bit set)", f64(-0.0), F("-0")),
    case("pi_approx", "Float64 3.14 (spec §5.5 example)", f64(3.14), F(3.14)),
    case("negative", "Float64 -1.5", f64(-1.5), F(-1.5)),
    case("max", "Largest finite Float64", f64(1.7976931348623157e308), F(1.7976931348623157e308)),
    case("min_subnormal", "Smallest positive subnormal Float64", f64(5e-324), F(5e-324)),
    case("infinity", "Positive infinity", f64(float("inf")), F("Infinity")),
    case("negative_infinity", "Negative infinity", f64(float("-inf")), F("-Infinity")),
    case("nan", "Quiet NaN (0x7FF8000000000000)", bytes([0x08]) + struct.pack("<Q", 0x7FF8000000000000), F("NaN")),
]

suites["datetime"] = [
    case("epoch", "Unix epoch", f64(0.0, 0x09), D(0)),
    case("modern", "2023-11-14T22:13:20Z", f64(1700000000000.0, 0x09), D(1700000000000)),
    case("with_millis", "2001-09-09T01:46:40.123Z", f64(1000000000123.0, 0x09), D(1000000000123)),
    case("before_epoch", "1969-12-31T00:00:00Z (negative)", f64(-86400000.0, 0x09), D(-86400000)),
]

suites["binary"] = [
    case("empty", "Empty binary (spec §5.7 example)", blob(0x10, b""), BIN(b"")),
    case("deadbeef", "Bytes DE AD BE EF (spec §5.7 example)", blob(0x10, bytes.fromhex("DEADBEEF")), BIN(bytes.fromhex("DEADBEEF"))),
    case("all_byte_values", "Bytes 00 through FF", blob(0x10, bytes(range(256))), BIN(bytes(range(256)))),
]

uni = "Snowed ❄ 雪 café \U0001F600"
suites["string"] = [
    case("empty", "Empty string", blob(0x11, b""), S("")),
    case("hello", "ASCII string (spec §5.8 example)", blob(0x11, b"hello"), S("hello")),
    case("unicode", "Multi-byte UTF-8: 2-, 3- and 4-byte sequences; length is in bytes", blob(0x11, uni.encode()), S(uni)),
    case("embedded_nul", "String containing U+0000", blob(0x11, b"a\x00b"), S("a\u0000b")),
]

decs = [
    ("zero", "0", "Zero"),
    ("one", "1", "Integer value"),
    ("one_point_five", "1.5", "Simple fraction"),
    ("negative", "-3.14159", "Negative fraction"),
    ("small", "0.001", "Leading zero before the decimal point"),
    ("large", "1000000", "Integer with trailing zeros (no exponent)"),
    ("high_precision", "1234567890123456789.012345678", "28 significant digits"),
]
suites["decimal"] = [case(n, d, blob(0x12, s.encode()), DEC(s)) for n, s, d in decs]

uuids = [
    ("spec_example", "550e8400-e29b-41d4-a716-446655440000", "Spec §5.10 example: RFC 4122 byte order"),
    ("nil", "00000000-0000-0000-0000-000000000000", "Nil UUID"),
    ("max", "ffffffff-ffff-ffff-ffff-ffffffffffff", "Max UUID"),
    ("byte_order", "00112233-4455-6677-8899-aabbccddeeff", "Distinct bytes; catches mixed-endian (.NET Guid) layouts"),
]
suites["uuid"] = [case(n, d, blob(0x13, uuid.UUID(s).bytes), UU(s)) for n, s, d in uuids]

suites["array"] = [
    case("empty", "Empty array", arr(), A()),
    case("spec_example", "[true, 42] (spec §5.11 example)", arr(b"\x03", i64(42)), A(B(True), I(42))),
    case("mixed", "Mixed element types", arr(b"\x00", blob(0x11, b"a"), f64(1.5), blob(0x10, b"\x01")),
         A(N, S("a"), F(1.5), BIN(b"\x01"))),
    case("nested", "Nested arrays [[], [null]]", arr(arr(), arr(b"\x00")), A(A(), A(N))),
]

k255 = "k" * 255
suites["object"] = [
    case("empty", "Empty object", obj(), O()),
    case("spec_example", '{"x": 1} (spec §5.12 example)', obj(("x", i64(1))), O(x=I(1))),
    case("sentinel_terminated", "Object ended by Sentinel in value position; the dangling key is discarded",
         b"\x80" + key("x") + i64(1) + key("dangling") + b"\xff", O(x=I(1)), canonical=False),
    case("sentinel_terminated_empty", "Object with only a dangling key before the Sentinel is empty",
         b"\x80" + key("dangling") + b"\xff", O(), canonical=False),
    case("max_key_length", "255-byte key: key length 0xFF is a length, not a Sentinel",
         obj((k255, b"\x03")), Od({k255: B(True)})),
    case("unicode_key", "Key with multi-byte UTF-8", obj(("café", b"\x00")), Od({"café": N})),
    case("multiple_keys", "Several keys with different value types",
         obj(("a", i64(1)), ("b", blob(0x11, b"two")), ("c", b"\x02")),
         O(a=I(1), b=S("two"), c=B(False)), canonical=False),
]

suites["nested"] = [
    case("object_in_array", 'Array containing an object: [{"id": 7}]', arr(obj(("id", i64(7)))), A(O(id=I(7)))),
    case("array_in_object", '{"tags": ["a", "b"]}', obj(("tags", arr(blob(0x11, b"a"), blob(0x11, b"b")))),
         O(tags=A(S("a"), S("b")))),
    case("deep", "Four levels: {\"a\": [{\"b\": [null]}]}", obj(("a", arr(obj(("b", arr(b"\x00")))))),
         O(a=A(O(b=A(N))))),
    case("record", "A realistic record mixing every type",
         obj(("id", blob(0x13, uuid.UUID("550e8400-e29b-41d4-a716-446655440000").bytes)),
             ("name", blob(0x11, b"Ada")),
             ("active", b"\x03"),
             ("count", i64(3)),
             ("big", u64(2**64 - 1)),
             ("ratio", f64(0.25)),
             ("price", blob(0x12, b"19.99")),
             ("created", f64(1700000000000.0, 0x09)),
             ("avatar", blob(0x10, b"\x89PNG")),
             ("parent", b"\x00"),
             ("children", arr(obj(("name", blob(0x11, b"Bo")))))),
         O(id=UU("550e8400-e29b-41d4-a716-446655440000"), name=S("Ada"), active=B(True), count=I(3),
           big=U(2**64 - 1), ratio=F(0.25), price=DEC("19.99"), created=D(1700000000000),
           avatar=BIN(b"\x89PNG"), parent=N, children=A(O(name=S("Bo")))),
         canonical=False),
]

suites["forward_compat"] = [
    case("unknown_null_range", "0x01 rounds down to Null", b"\x01", N, canonical=False),
    case("unknown_int64_range", "0x06 rounds down to Int64", bytes([0x06]) + struct.pack("<q", 42), I(42), canonical=False),
    case("unknown_int64_range_top", "0x07 rounds down to Int64", bytes([0x07]) + struct.pack("<q", -5), I(-5), canonical=False),
    case("unknown_float64_range", "0x0A rounds down to Float64, not DateTime", f64(1.5, 0x0A), F(1.5), canonical=False),
    case("unknown_float64_range_top", "0x0F rounds down to Float64", f64(-2.0, 0x0F), F(-2.0), canonical=False),
    case("unknown_binary_range", "0x14 rounds down to Binary, not UUID", blob(0x14, b"\x01\x02"), BIN(b"\x01\x02"), canonical=False),
    case("unknown_binary_range_top", "0x3F rounds down to Binary", blob(0x3F, b"xyz"), BIN(b"xyz"), canonical=False),
    case("unknown_array_range", "0x41 rounds down to Array", arr(b"\x03", ty=0x41), A(B(True)), canonical=False),
    case("unknown_array_range_top", "0x7F rounds down to Array", arr(ty=0x7F), A(), canonical=False),
    case("unknown_object_range", "0xC0 rounds down to Object", obj(("x", i64(1)), ty=0xC0), O(x=I(1)), canonical=False),
    case("unknown_object_range_top", "0xFE rounds down to Object", obj(ty=0xFE), O(), canonical=False),
    case("unknown_nested", "Unknown type bytes inside a container are rounded down too",
         arr(f64(2.5, 0x0B), obj(("k", blob(0x20, b"\xAA"))), b"\x01"),
         A(F(2.5), O(k=BIN(b"\xAA")), N), canonical=False),
]

suites["invalid"] = [
    case("empty_input", "No bytes at all", b"", error=True),
    case("sentinel_root", "Sentinel is not a value (§5.13)", b"\xff", error=True),
    case("unterminated_array", "Array with an element and no Sentinel", b"\x40\x03", error=True),
    case("truncated_int64", "Int64 with only 4 payload bytes", b"\x04\x01\x02\x03\x04", error=True),
    case("truncated_float64", "Float64 with only 7 payload bytes", b"\x08" + b"\x00" * 7, error=True),
    case("truncated_length", "Binary with a 3-byte length field", b"\x10\x01\x00\x00", error=True),
    case("truncated_payload", "Binary claiming 4 bytes with only 2 present", b"\x10" + struct.pack("<Q", 4) + b"\xDE\xAD", error=True),
    case("huge_length", "Binary claiming 2^63 bytes", b"\x10" + struct.pack("<Q", 2**63), error=True),
    case("invalid_utf8_string", "String with an invalid UTF-8 byte (0xFF)", blob(0x11, b"a\xffb"), error=True),
    case("invalid_utf8_overlong", "String with an overlong encoding of '/' (C0 AF)", blob(0x11, b"\xc0\xaf"), error=True),
    case("invalid_utf8_surrogate", "String with an encoded UTF-16 surrogate (ED A0 80)", blob(0x11, b"\xed\xa0\x80"), error=True),
    case("invalid_utf8_key", "Object key with invalid UTF-8", b"\x80\x01\xff\x00\x00", error=True),
    case("uuid_short", "UUID with length 15", blob(0x13, b"\x00" * 15), error=True),
    case("uuid_long", "UUID with length 17", blob(0x13, b"\x00" * 17), error=True),
    case("truncated_object_key", "Object key length 5 with only 2 key bytes", b"\x80\x05ab", error=True),
    case("unterminated_object", "Object with a key-value pair and no terminator", b"\x80" + key("x") + b"\x00", error=True),
    case("truncated_nested", "Array ending inside a nested object", b"\x40\x80" + key("x"), error=True),
]

OUT.mkdir(parents=True, exist_ok=True)
names = {
    "null": "Null", "boolean": "Boolean", "int64": "Int64", "uint64": "UInt64", "float64": "Float64",
    "datetime": "DateTime", "binary": "Binary", "string": "String", "decimal": "Decimal", "uuid": "UUID",
    "array": "Array", "object": "Object", "nested": "Nested", "forward_compat": "Forward Compatibility",
    "invalid": "Invalid Input",
}
for fname, cases in suites.items():
    names_seen = [c["name"] for c in cases]
    assert len(names_seen) == len(set(names_seen)), fname
    doc = {"suite": names[fname], "cases": cases}
    (OUT / f"{fname}.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
print(sum(len(c) for c in suites.values()), "cases in", len(suites), "files")
