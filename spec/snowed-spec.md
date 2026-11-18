# Snowed Binary Encoding Specification

**Version:** 0.1.0-draft

---

## 1. Overview

Snowed is a self-describing binary data serialization format designed for encoding nested structures where values are frequently binary blobs. It provides a compact, streaming-friendly encoding with a type system logically similar to JSON, enriched with native binary and DateTime types.

---

## 2. Design Goals

- **Binary-first** — Optimized for use cases where binary blobs are common field values, without requiring base64 or string encoding.
- **Self-describing** — No external schema is required. Type information is embedded in the encoding.
- **Native DateTime** — A dedicated DateTime type eliminates the fragile convention of encoding dates as strings.
- **Streaming-friendly** — A complete value can be read from a stream without prior knowledge of total document length.
- **Forward-compatible** — Parsers encountering unknown type bytes degrade gracefully by treating them as their nearest known base type.
- **Zero dependencies** — Implementations require only the host language's standard library.

---

## 3. Conventions

### 3.1 Byte Order

All multi-byte integer and floating-point values are encoded in **little-endian** byte order.

### 3.2 Notation

Byte sequences in this document are written as space-separated hexadecimal values prefixed with `0x`, e.g.:

```
0x04 0x2A 0x00 0x00 0x00 0x00 0x00 0x00 0x00
```

---

## 4. Type System

### 4.1 Type Indicator Byte

Every Snowed value begins with a single **type indicator byte** that identifies the type and determines how the remaining bytes of the value are interpreted.

### 4.2 Defined Types

| Decimal | Hex  | Name          | Kind               |
|---------|------|---------------|--------------------|
| 0       | 0x00 | Null          | Base type          |
| 2       | 0x02 | Boolean False | Defined value      |
| 3       | 0x03 | Boolean True  | Defined value      |
| 4       | 0x04 | Int64         | Base type          |
| 5       | 0x05 | UInt64        | Subtype of Int64   |
| 8       | 0x08 | Float64       | Base type          |
| 9       | 0x09 | DateTime      | Subtype of Float64 |
| 16      | 0x10 | Binary        | Base type          |
| 17      | 0x11 | String        | Subtype of Binary  |
| 18      | 0x12 | Decimal       | Subtype of Binary  |
| 19      | 0x13 | UUID          | Subtype of Binary  |
| 64      | 0x40 | Array         | Base type          |
| 128     | 0x80 | Object        | Base type          |
| 255     | 0xFF | Sentinel      | Base type          |

### 4.3 Base Types and Subtype Ranges

A **base type** defines a **parsing layout** — how many bytes to read and how to interpret them. Every type indicator byte belongs to exactly one base type's range. Defined subtypes (such as DateTime and String) share their base type's binary layout; they extend or constrain its semantics.

| Base Type | Hex  | Range         | Layout                                     |
|-----------|------|---------------|--------------------------------------------|
| Null      | 0x00 | 0x00–0x03     | 1 byte (type indicator only)               |
| Int64     | 0x04 | 0x04–0x07     | 1-byte indicator + 8 bytes (integer)       |
| Float64   | 0x08 | 0x08–0x0F     | 1-byte indicator + 8 bytes (IEEE 754)      |
| Binary    | 0x10 | 0x10–0x3F     | 1-byte indicator + 8-byte length + N bytes |
| Array     | 0x40 | 0x40–0x7F     | 1-byte indicator + elements + Sentinel     |
| Object    | 0x80 | 0x80–0xFE     | 1-byte indicator + key-value pairs + terminator |
| Sentinel  | 0xFF | 0xFF          | 1 byte (type indicator only)               |

> **Note on Booleans:** Boolean False (0x02) and Boolean True (0x03) fall within the Null base range (0x00–0x03) but are fully specified single-byte values. A compliant parser handles them explicitly before applying the base-type round-down rule.

### 4.4 Forward Compatibility

When a parser encounters a type indicator byte that is **not** in the defined type table (§4.2), it **rounds down to the nearest base type** and parses the value using that base type's layout. The value is returned typed as the base type.

**Algorithm:** Given an unknown type byte X, the applicable base type B is the largest value in `{0x00, 0x04, 0x08, 0x10, 0x40, 0x80, 0xFF}` such that B ≤ X.

**Examples:**

| Unknown Byte | Nearest Base | Parsing Behavior |
|---|---|---|
| 0x01 | Null (0x00) | Single byte; return Null |
| 0x06 | Int64 (0x04) | Read 8 bytes; return as Int64 |
| 0x0A | Float64 (0x08) | Read 8 bytes; return as Float64 |
| 0x14 | Binary (0x10) | Read 8-byte length, then that many bytes; return as Binary |
| 0x41 | Array (0x40) | Read elements until Sentinel; return as Array |
| 0xC0 | Object (0x80) | Read key-value pairs until terminator; return as Object |

**Defined subtypes and older parsers:** A parser that does not implement UInt64 (0x05) will parse it as Int64 — values ≤ Int64 max are preserved correctly; larger values will appear as negative numbers. A parser that does not implement DateTime (0x09) will parse it as Float64 — the raw millisecond value is preserved. A parser that does not implement String (0x11), Decimal (0x12), or UUID (0x13) will parse them as Binary — the raw bytes are preserved and remain decodeable.

---

## 5. Value Encoding

### 5.1 Null

**Type byte:** `0x00`

No bytes follow the type indicator. The type indicator is the complete encoding of a Null value.

```
0x00
```

### 5.2 Boolean

**Type byte:** `0x02` (False) | `0x03` (True)

No bytes follow the type indicator. The type indicator encodes both type and value.

```
0x02        → false
0x03        → true
```

### 5.3 Int64

**Type byte:** `0x04`

The 8 bytes following the type indicator are a **signed 64-bit integer** in little-endian two's complement representation.

```
0x04  b0 b1 b2 b3 b4 b5 b6 b7
      └───────── int64 LE ────────┘
```

**Range:** −9,223,372,036,854,775,808 to 9,223,372,036,854,775,807

**Example** — value `42`:
```
0x04  0x2A 0x00 0x00 0x00 0x00 0x00 0x00 0x00
```

**Example** — value `-1`:
```
0x04  0xFF 0xFF 0xFF 0xFF 0xFF 0xFF 0xFF 0xFF
```

### 5.4 UInt64

**Type byte:** `0x05` — subtype of Int64

The 8 bytes following the type indicator are an **unsigned 64-bit integer** in little-endian byte order. The wire layout is identical to Int64.

```
0x05  b0 b1 b2 b3 b4 b5 b6 b7
      └──────── uint64 LE ───────┘
```

**Range:** 0 to 18,446,744,073,709,551,615

**Example** — value `18446744073709551615` (max uint64):
```
0x05  0xFF 0xFF 0xFF 0xFF 0xFF 0xFF 0xFF 0xFF
```

A parser without UInt64 support will parse 0x05 as Int64. Values ≤ 9,223,372,036,854,775,807 (`Int64` max) are preserved correctly; larger values will be misread as negative numbers. In all cases the correct 8 bytes are consumed.

### 5.5 Float64

**Type byte:** `0x08`

The 8 bytes following the type indicator are an **IEEE 754 binary64 (double-precision)** value in little-endian byte order.

```
0x08  b0 b1 b2 b3 b4 b5 b6 b7
      └────── float64 LE ───────┘
```

**Example** — value `3.14`:
```
0x08  0x1F 0x85 0xEB 0x51 0xB8 0x1E 0x09 0x40
```

IEEE 754 special values (NaN, ±Infinity) are permitted by the encoding. Implementations may choose to reject them at the application layer.

### 5.6 DateTime

**Type byte:** `0x09` — subtype of Float64

Encodes a point in time as the **number of milliseconds since the Unix epoch (1970-01-01T00:00:00.000 UTC)**, stored as an IEEE 754 binary64 value in little-endian byte order. The wire layout is identical to Float64.

```
0x09  b0 b1 b2 b3 b4 b5 b6 b7
      └────── float64 LE ───────┘
```

- **Always UTC.** No timezone offset is stored or implied.
- **Negative values** represent instants before the Unix epoch.
- **Fractional milliseconds** may be represented using the fractional part of the double value.
- The double's 53-bit mantissa represents integer millisecond values exactly up to ±2⁵³ ms (~±285,616 years from epoch), sufficient for all practical timestamps.

A parser without DateTime support will parse 0x09 as Float64, preserving the raw numeric value.

### 5.7 Binary

**Type byte:** `0x10`

The 8 bytes following the type indicator are an **unsigned 64-bit integer** in little-endian byte order specifying the payload length in bytes. The payload follows immediately with no padding or alignment.

```
0x10  [len: 8 bytes LE uint64]  [payload: len bytes]
```

**Example** — bytes `[0xDE, 0xAD, 0xBE, 0xEF]`:
```
0x10  0x04 0x00 0x00 0x00 0x00 0x00 0x00 0x00  0xDE 0xAD 0xBE 0xEF
```

**Empty binary** is valid:
```
0x10  0x00 0x00 0x00 0x00 0x00 0x00 0x00 0x00
```

### 5.8 String

**Type byte:** `0x11` — subtype of Binary

Encodes a Unicode string as a UTF-8 byte sequence. The wire layout is identical to Binary: an 8-byte little-endian unsigned integer specifying the byte length, followed by the UTF-8-encoded bytes. No null terminator is included.

```
0x11  [len: 8 bytes LE uint64]  [utf-8 bytes: len bytes]
```

- `len` is the number of **encoded bytes**, not Unicode code points or characters.
- String values must be valid UTF-8. Encoders must not produce invalid UTF-8.
- Decoders must validate UTF-8 and must not silently produce invalid string values in the host language.
- A parser without String support parses 0x11 as Binary, preserving the raw bytes.

**Example** — string `"hello"` (5 bytes: `0x68 0x65 0x6C 0x6C 0x6F`):
```
0x11  0x05 0x00 0x00 0x00 0x00 0x00 0x00 0x00  0x68 0x65 0x6C 0x6C 0x6F
```

### 5.9 Decimal

**Type byte:** `0x12` — subtype of Binary

Encodes an arbitrary-precision decimal number as a UTF-8 string in canonical form. The wire layout is identical to Binary and String: an 8-byte little-endian unsigned integer specifying the byte length, followed by the UTF-8 bytes of the canonical string. No null terminator is included.

```
0x12  [len: 8 bytes LE uint64]  [utf-8 bytes: len bytes]
```

**Canonical form rules:**

1. The decimal separator is always `.` (period). Locale-sensitive separators must not be used.
2. No exponent notation. The value is always expressed in plain decimal form.
3. No trailing zeros after the decimal point.
4. No leading zeros before the decimal point, except a single `0` in the ones place for values with absolute value less than 1 (e.g. `0.5`, not `.5`).
5. Negative zero is normalised to `"0"`.
6. The value zero is always `"0"`.

**Examples:**

| Value | Canonical string |
|-------|-----------------|
| 1 | `"1"` |
| 1.0 | `"1"` |
| 1.50 | `"1.5"` |
| -3.14159 | `"-3.14159"` |
| 0.001 | `"0.001"` |
| 1000000 | `"1000000"` |

**Scale is not preserved.** The values `1`, `1.0`, and `1.00` all encode as `"1"`. Implementations that internally store scale as part of the decimal type (C# `decimal`, Kotlin `BigDecimal`) will not round-trip scale across an encode/decode cycle.

**Canonical production by platform:**

| Platform | Method |
|---|---|
| C# | `value.ToString("G29", CultureInfo.InvariantCulture)`, then strip any trailing `.` |
| Kotlin (JVM) | `value.stripTrailingZeros().toPlainString()` |
| Swift | `value.description` — verify no trailing zeros in output |

A parser without Decimal support parses `0x12` as Binary — the raw UTF-8 bytes are preserved and remain decodeable as a decimal string. Implementations without a native decimal type should decode `0x12` as `String`.

### 5.10 UUID

**Type byte:** `0x13` — subtype of Binary

Encodes a UUID (Universally Unique Identifier, also called GUID) as exactly **16 bytes** in RFC 4122 byte order. The wire layout is identical to Binary with a fixed length of 16.

```
0x13  [len: 8 bytes LE uint64 = 16]  [16 bytes RFC 4122]
```

**RFC 4122 byte order** is big-endian throughout: the 16 bytes correspond directly to the canonical `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` string representation read left to right, with hyphens removed.

**Example** — UUID `550e8400-e29b-41d4-a716-446655440000`:
```
0x13
0x10 0x00 0x00 0x00 0x00 0x00 0x00 0x00   (length = 16)
0x55 0x0e 0x84 0x00 0xe2 0x9b 0x41 0xd4
0xa7 0x16 0x44 0x66 0x55 0x44 0x00 0x00
```

The length field is always `16`. Encoders must always write exactly 16 bytes. Decoders must reject UUID values where the length field is not exactly 16.

A parser without UUID support parses `0x13` as Binary — the 16 raw bytes are preserved.

> **Platform note:** .NET's `Guid.ToByteArray()` returns a mixed-endian layout that does **not** match RFC 4122. .NET implementations must use `Guid.ToByteArray(bigEndian: true)` (.NET 9+) or perform a manual byte swap. See [platform-limitations.md](platform-limitations.md).

### 5.11 Array

**Type byte:** `0x40`

An ordered sequence of zero or more values. Each element is a complete Snowed value beginning with its own type indicator byte. The sequence is terminated by a Sentinel byte (`0xFF`).

```
0x40  [value₁] [value₂] ... [valueₙ]  0xFF
```

- Elements may be of any Snowed type, including nested Arrays and Objects.
- Mixed element types are permitted.
- No element count is encoded; parsers read elements until the Sentinel.

**Empty array:**
```
0x40  0xFF
```

**Example** — array `[true, 42]`:
```
0x40
  0x03                                          (Boolean True)
  0x04 0x2A 0x00 0x00 0x00 0x00 0x00 0x00 0x00  (Int64: 42)
0xFF
```

### 5.12 Object

**Type byte:** `0x80`

An associative collection of key-value pairs. Keys use a compact string encoding (see §6). Values are complete Snowed values of any type. Key-value pairs are encoded sequentially and the object ends at the first of either:

1. A **zero-length key** — a key length byte of `0x00` in key position.
2. A **Sentinel** — a type indicator byte of `0xFF` encountered in value position (after a valid key has been read). The key preceding the Sentinel has no associated value and must be discarded.

```
0x80  [key₁] [value₁] [key₂] [value₂] ...  [terminator]
```

- Values may be of any Snowed type, including nested Objects and Arrays.
- Mixed value types are permitted.
- Key ordering is not significant. Implementations may return keys in any order.
- **Duplicate keys:** behavior is implementation-defined. Implementations must document whether they use first or last occurrence. Encoders should not produce duplicate keys.

**Empty object:**
```
0x80  0x00
```

**Example** — object `{"x": 1}`:
```
0x80
  0x01 0x78                                    (key: "x", length 1)
  0x04 0x01 0x00 0x00 0x00 0x00 0x00 0x00 0x00  (value: Int64 1)
0x00                                            (zero-length key: end of object)
```

### 5.13 Sentinel

**Type byte:** `0xFF`

A single-byte structural marker used exclusively to terminate Arrays (§5.11) and Objects in value position (§5.12). The Sentinel is not a value type. It must not appear as a standalone root value, as a storable element within an Array, or as a storable value within an Object.

---

## 6. Object Key Encoding

Object keys use a compact encoding distinct from the general Binary/String encoding in §5.7–5.8.

```
[length: 1 byte unsigned]  [utf-8 bytes: length bytes]
```

- `length` is a single unsigned byte (0–255) indicating the number of UTF-8-encoded bytes in the key name.
- A `length` of `0x00` terminates the object (see §5.12). No key bytes follow.
- A `length` of `0xFF` (255) is valid and indicates a 255-byte key. The Sentinel (also `0xFF`) can only appear where a **value type byte** is expected, not where a key length byte is expected; there is no ambiguity.
- Keys must be valid UTF-8.
- **Maximum key length:** 255 bytes (UTF-8 encoded).

> **Rationale:** Object keys are always strings. The compact single-byte length prefix avoids the 8-byte overhead of the general Binary encoding for what are typically short keys.

---

## 7. Document Structure

A Snowed document is a **single encoded value** of any type. There is no file header, magic bytes, or version field. The first byte of any Snowed input is the type indicator byte of the root value.

**Any type** may be the root value, including Null, scalars, Arrays, and Objects.

### 7.1 Streaming

When reading from a stream, a parser reads and returns exactly one complete root value. The parser must not consume any bytes beyond the end of that root value. The caller is responsible for reading subsequent values from the stream when the stream carries more than one.

---

## 8. Conformance

A conformant Snowed implementation must:

1. Correctly encode and decode all types defined in §4.2.
2. Use **little-endian** byte order for all multi-byte values: Int64, UInt64, Float64, DateTime, and the length fields of Binary and String.
3. When encountering an **unknown type indicator byte**, round down to the nearest base type per §4.4 and parse using that base type's layout.
4. Handle **both** object termination conditions: a zero-length key in key position, and a Sentinel in value position.
5. Validate that **String values** (0x11) and **object key bytes** are valid UTF-8 before exposing them to application code.
6. **Not** require a document envelope; the first byte of any input is the root type indicator.
7. Have **no runtime dependencies** beyond the host language's standard library.

---

## Appendix A: Type Byte Quick Reference

```
0x00        Null
0x01        (unknown — Null range, parse as Null)
0x02        Boolean False
0x03        Boolean True
0x04        Int64
0x05        UInt64
0x06–0x07   (unknown — Int64 range, parse as Int64)
0x08        Float64
0x09        DateTime
0x0A–0x0F   (unknown — Float64 range, parse as Float64)
0x10        Binary
0x11        String
0x12        Decimal
0x13        UUID
0x14–0x3F   (unknown — Binary range, parse as Binary)
0x40        Array
0x41–0x7F   (unknown — Array range, parse as Array)
0x80        Object
0x81–0xFE   (unknown — Object range, parse as Object)
0xFF        Sentinel
```

---

## Appendix B: Wire Format Summary

| Type     | Total bytes         | Layout |
|----------|---------------------|--------|
| Null     | 1                   | `[TY]` |
| Bool     | 1                   | `[TY]` |
| Int64    | 9                   | `[TY][i0][i1][i2][i3][i4][i5][i6][i7]` |
| UInt64   | 9                   | `[TY][i0][i1][i2][i3][i4][i5][i6][i7]` |
| Float64  | 9                   | `[TY][f0][f1][f2][f3][f4][f5][f6][f7]` |
| DateTime | 9                   | `[TY][f0][f1][f2][f3][f4][f5][f6][f7]` |
| Binary   | 9 + N               | `[TY][l0..l7][payload]` |
| String   | 9 + N               | `[TY][l0..l7][utf-8 bytes]` |
| Decimal  | 9 + N               | `[TY][l0..l7][utf-8 bytes]` |
| UUID     | 25                  | `[TY][0x10 0x00 * 7][16 bytes RFC 4122]` |
| Array    | 2 + Σ(elements)     | `[TY][elem]...[0xFF]` |
| Object   | 2 + Σ(key+value)    | `[TY][klen][kbytes][value]...[0x00]` |
| Sentinel | 1                   | `[0xFF]` |

All length fields (`l0..l7`) are unsigned 64-bit little-endian integers. `[TY]` is the type indicator byte.
