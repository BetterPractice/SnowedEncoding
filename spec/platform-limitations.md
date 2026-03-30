# Snowed Platform Limitations

This document describes known limitations and behavioural differences for each Snowed implementation, arising from type system constraints, runtime environments, or standard library gaps.

Limitations are divided into two categories:
- **Hard limitation** — the platform fundamentally cannot represent the value correctly; data loss or an error is unavoidable.
- **Implementation note** — correct behaviour is achievable but requires explicit handling that differs from the obvious approach.

---

## Swift

### UInt64 values on 32-bit platforms
**Implementation note.** Swift's `UInt` is 32-bit on 32-bit platforms. Encoding a `UInt` that exceeds `UInt32.max` is not possible on those targets. All modern Apple platforms (iOS 7+, macOS 10.7+) are 64-bit; 32-bit targets are effectively unsupported.

### `Decimal` type
**Implementation note.** Swift's `Decimal` maps to the Snowed Decimal type (0x12), encoded as a canonical UTF-8 string per §5.9 of the spec. `Decimal.description` produces the string representation; verify that no trailing zeros appear in output. Scale is not preserved across encode/decode — `Decimal(string: "1.00")` round-trips as `Decimal(string: "1")`.

### `Float` (32-bit)
**Implementation note.** Swift `Float` values are widened to `Double` on encode with no precision loss. On decode, a Snowed Float64 value may be narrowed to `Float` with precision loss if the decoded value is assigned to a `Float` property; this is a consequence of the host type, not the encoding.

### `superEncoder()` / `superEncoder(forKey:)`
**Implementation note.** These methods on `KeyedEncodingContainerProtocol` are required by the `Codable` protocol for class inheritance hierarchies. They must be implemented correctly; a `fatalError` stub is not acceptable in a production implementation.

### UUID
**Implementation note.** Swift's `UUID` type stores 16 bytes in RFC 4122 byte order internally (`uuid` property is a tuple of 16 `UInt8` values). The encoder writes them directly in that order; no byte swap is needed.

---

## Go

### `complex64` / `complex128`
**Hard limitation.** Go's complex number types have no Snowed equivalent and cannot be encoded.

### Decimal (0x12)
**Implementation note.** Go has no native decimal type. Fields holding decimal values must be `string` and carry a `snowed:"name,decimal"` struct tag. The encoder validates the string against the canonical rules (§5.9) and writes 0x12; the decoder reads 0x12 and writes the canonical string back to the field. A 0x12 value arriving on an untagged `string` field returns a decode error.

### `time.Time` timezone
**Implementation note.** Go's `time.Time` carries timezone information. Snowed DateTime is always UTC. The encoder must call `.UTC()` before encoding; the decoder always produces UTC `time.Time` values. Callers who want local time call `.Local()` on the decoded value themselves.

### `time.Time` sub-millisecond precision
**Implementation note.** Go's `time.Time` has nanosecond resolution. Snowed DateTime is stored as a float64 number of milliseconds, which preserves precision down to approximately one microsecond but silently discards sub-microsecond (nanosecond) precision. This loss is a consequence of the wire format, not the implementation.

### `int` / `uint` width on 32-bit platforms
**Implementation note.** Go's `int` and `uint` are platform-dependent (32 or 64 bits). The encoder always widens these to `int64` / `uint64`. On a 32-bit platform, a `uint` value cannot exceed `uint32` max, so no overflow is possible in practice.

### UUID
**Implementation note.** The implementation provides two mechanisms for encoding and decoding UUID (0x13) values:

1. **`snowed.UUID` type** (`type UUID [16]byte`) — a first-class type that encodes/decodes automatically when used as a struct field. Provides `String() string` and `Parse(string) (UUID, error)`. Bytes are stored and written in RFC 4122 order directly.
2. **String field with `snowed:"name,uuid"` tag** — the field holds a UUID in canonical `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` string form. The encoder parses the string (case-insensitive) and writes 0x13; the decoder reads 0x13 and writes lowercase canonical form back to the string field. Invalid string format returns an error on encode; a 0x13 value arriving on an untagged `string` field returns a decode error.

Third-party UUID library values (`[16]byte` underneath) convert to `snowed.UUID` with a direct cast.

---

## Rust

### `no_std` compatibility
**Implementation note.** The crate targets `no_std + alloc` for embedded compatibility (ESP32 via `esp-rs` / ESP-IDF). All standard library types used in the public API (`Vec`, `String`, `BTreeMap`) come from `alloc`. The crate must not transitively require `std`.

### Decimal (0x12)
**Implementation note.** `SnowedValue::Decimal` stores the value as a `String` in canonical form (§5.9). No arithmetic library is included. The optional `rust_decimal` feature provides `From<rust_decimal::Decimal>` and `TryFrom<SnowedValue> for rust_decimal::Decimal`; this is the only anticipated optional dependency.

### UUID
**Implementation note.** UUID (0x13) is stored as `[u8; 16]` in RFC 4122 byte order. The crate imposes no UUID library dependency. Callers using a third-party UUID type convert via raw byte access (e.g. `uuid::Uuid::from_bytes`).

### DateTime
**Implementation note.** `SnowedValue::DateTime` stores milliseconds since the Unix epoch as `f64`, matching the wire representation directly. The crate does not depend on any time library. Callers convert to/from their preferred time type (e.g. `chrono`, `time`) at the application boundary.

### Sub-millisecond DateTime precision
**Implementation note.** The Snowed DateTime wire format is a float64 number of milliseconds. This preserves precision down to approximately one microsecond. Most time libraries (`chrono::DateTime`, `time::OffsetDateTime`) have nanosecond resolution; sub-microsecond precision is silently lost on encode/decode.

---

## C#

### `decimal`
**Implementation note.** C#'s `decimal` maps to the Snowed Decimal type (0x12), encoded as a canonical UTF-8 string per §5.9 of the spec. Use `value.ToString("G29", CultureInfo.InvariantCulture)` and strip any trailing `.` character. Scale is not preserved across encode/decode — `1.00M` round-trips as `1M`.

### `DateTimeOffset` timezone
**Implementation note.** `DateTimeOffset` carries a UTC offset. Snowed DateTime is always UTC. The encoder must normalise to UTC (`.ToUniversalTime()`) before encoding; the decoder always produces UTC `DateTimeOffset` values.

### `DateOnly` / `TimeOnly`
**Hard limitation.** These .NET 6+ types have no direct Snowed equivalent. `DateOnly` can be encoded as a DateTime at midnight UTC with an explicit convention; `TimeOnly` has no natural encoding. Both require application-level handling.

### `Guid`
**Implementation note.** `Guid` maps to the Snowed UUID type (0x13). The Snowed spec requires RFC 4122 byte order (big-endian throughout). .NET's `Guid.ToByteArray()` returns a **mixed-endian** layout and must not be used directly. Use `Guid.ToByteArray(bigEndian: true)` (.NET 9+), or on earlier targets manually swap bytes: the first 4 bytes, next 2 bytes, and next 2 bytes each need their byte order reversed.

### `TimeSpan`
**Hard limitation.** No Snowed equivalent. Must be encoded by convention (e.g. as Int64 total milliseconds) at the application level.

---

## Kotlin

### `ULong` on JVM
**Implementation note.** Kotlin's `ULong` is an inline class backed by `Long` on the JVM. Values in the range `Long.MAX_VALUE + 1` to `ULong.MAX_VALUE` are representable as `ULong` in Kotlin source but require care at JVM bytecode boundaries. The Snowed encoder and decoder must operate on the `ULong` type directly and avoid passing these values through APIs that accept `Long`.

### `BigDecimal`
**Implementation note.** Kotlin's `BigDecimal` maps to the Snowed Decimal type (0x12), encoded as a canonical UTF-8 string per §5.9 of the spec. Use `value.stripTrailingZeros().toPlainString()`. Scale is not preserved across encode/decode. Note: `BigDecimal` is a JVM type and is unavailable in Kotlin common code for Kotlin Multiplatform targets; those targets decode `0x12` as `String`.

### `BigInteger`
**Hard limitation.** No Snowed equivalent. Values must be converted to `Long` or `ULong` before encoding, with potential overflow.

### `Instant` vs `LocalDateTime`
**Implementation note.** Snowed DateTime is always UTC. `LocalDateTime` has no timezone and must not be used as a DateTime value without explicit UTC normalisation. `Instant` is the correct Kotlin type to map to Snowed DateTime. `ZonedDateTime` / `OffsetDateTime` timezone information is discarded on encode.

### UUID
**Implementation note.** The JVM's `java.util.UUID` stores the UUID as two `Long` values (`mostSignificantBits` and `leastSignificantBits`). RFC 4122 byte order places the most-significant 8 bytes first. The encoder must write `mostSignificantBits` as big-endian followed by `leastSignificantBits` as big-endian. The decoder reconstructs with `UUID(msb, lsb)`.

### `Duration`
**Hard limitation.** No Snowed equivalent. Must be encoded by convention at the application level.

---

## Python

### `bool` subclasses `int`
**Implementation note.** In Python, `bool` is a subclass of `int`. The encoder must check `isinstance(obj, bool)` before `isinstance(obj, int)`; otherwise `True` and `False` are encoded as Int64 (1 and 0) instead of Bool (0x03 and 0x02).

### Int64 vs UInt64 — no native unsigned type
**Implementation note.** Python has no `uint64` type. All plain `int` values encode as Int64 (0x04) by default. Use `SnowedUInt64(n)` to force UInt64 (0x05) encoding. For `@dataclass` fields, use `Annotated[int, snowed.uint64]`. Decoded UInt64 values are returned as `SnowedUInt64` instances to preserve round-trip fidelity.

### Arbitrary-precision `int`
**Implementation note.** Python integers have no fixed width. Values outside the Int64 range (−2⁶³ to 2⁶³−1) cannot be encoded as Int64 and will raise `SnowedEncodeError`. Values outside the UInt64 range (0 to 2⁶⁴−1) will raise `SnowedEncodeError` when passed as `SnowedUInt64`. There is no Snowed type for arbitrary-precision integers.

### `decimal.Decimal` — canonical form
**Implementation note.** `decimal.Decimal` maps to the Snowed Decimal type (0x12). Canonical form is computed via `as_tuple()` decomposition to avoid Python's `normalize()` returning scientific notation (e.g. `Decimal('1E+3')`). NaN and Infinity raise `SnowedEncodeError`.

### `datetime.datetime` — naive datetimes rejected
**Implementation note.** Python's `datetime.datetime` can be naive (no timezone) or aware. Snowed DateTime is always UTC. The encoder rejects naive datetimes with `SnowedEncodeError`; callers must attach `datetime.timezone.utc` or another timezone. Decoded DateTime values are always UTC-aware `datetime.datetime` instances.

### `datetime.datetime` sub-millisecond precision
**Implementation note.** Python's `datetime.datetime` has microsecond resolution. Snowed DateTime is stored as a float64 number of milliseconds, which preserves precision down to approximately one microsecond. Sub-microsecond precision is silently discarded.

### `uuid.UUID`
**Implementation note.** Python's `uuid.UUID.bytes` property returns bytes in RFC 4122 (big-endian) order. No byte swap is needed. The encoder writes these bytes directly; the decoder constructs `uuid.UUID(bytes=data)`.

