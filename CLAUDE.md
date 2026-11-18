# Snowed Encoding — Project Guide

## What is Snowed?

Snowed is a custom binary serialization format. Self-describing (no external schema), logically similar to JSON, optimized for nested structures with binary blob payloads and a native DateTime type. Wire format is little-endian throughout.

## Approach

When adding new features or types, prefer writing spec/documentation first, then implement.

## Spec

- `spec/snowed-spec.md` — canonical wire format spec (v0.1.0-draft)
- `spec/platform-limitations.md` — per-platform notes (integer sizes, UUID byte order, decimal support, etc.)

## Type System

Base types define the parsing layout. Unknown type bytes round down to the nearest **base type** — not the nearest known type. Example: 0x0A → Float64 (0x08), not DateTime (0x09).

| Byte | Type | Notes |
|------|------|-------|
| 0x00 | Null | |
| 0x02 | BoolFalse | Int64 subtype |
| 0x03 | BoolTrue | Int64 subtype |
| 0x04 | Int64 | Base type |
| 0x05 | UInt64 | Int64 subtype — older parsers read large values as negative signed (acceptable) |
| 0x08 | Float64 | Base type |
| 0x09 | DateTime | Float64 subtype — ms since Unix epoch UTC |
| 0x10 | Binary | Base type — 8-byte LE length prefix |
| 0x11 | String | Binary subtype — UTF-8 |
| 0x12 | Decimal | Binary subtype — canonical UTF-8 string, no trailing zeros, no exponent, negative zero → "0" |
| 0x13 | UUID | Binary subtype — 16 bytes, RFC 4122 byte order (big-endian throughout) |
| 0x40 | Array | Base type — terminated by Sentinel (0xFF) |
| 0x80 | Object | Base type — terminated by zero-length key or Sentinel in value position |
| 0xFF | Sentinel | Only appears in value position; 0xFF in key-length position = valid 255-byte key |

## Object Key Encoding

1-byte length prefix (0–255) + UTF-8 bytes. Length 0x00 terminates the object. Max key = 255 UTF-8 bytes.

## Platform-Specific Gotchas

- **.NET `Guid`**: mixed-endian layout — must manually reverse bytes 0–3, 4–5, 6–7 to get RFC 4122 order
- **Swift `UUID`**: stores RFC 4122 natively, no swap needed

## Implementations

| Language | Status | Notes |
|----------|--------|-------|
| Swift | Complete | Uses `Codable` |
| C# | Complete | `Snowed` (core) + `Snowed.AspNetCore` (ASP.NET Core formatters) |
| Go | Complete | Standard library only; `Marshal`/`Unmarshal` + streaming `Encoder`/`Decoder`; `snowed` struct tags |
| Python | Complete | No dependencies; `dumps`/`loads` + streaming; `SnowedUInt64` wrapper; `Annotated[int, snowed.uint64]` for dataclass fields |
| Kotlin | Complete | KMP; `kotlinx.serialization`; JVM streaming + UUID/BigDecimal/Instant extensions |
| Rust | Complete | `no_std + alloc`; encode/decode/serde complete |

## Running Tests

```bash
# Swift
swift test   # from repo root (Package.swift lives there)

# C#
cd implementations/csharp && dotnet test

# Rust
cd implementations/rust && cargo test --features serde

# Kotlin
cd implementations/kotlin && ./gradlew jvmTest

# Go
cd implementations/go && go test ./snowed

# Python
cd implementations/python && python3 -m venv .venv && .venv/bin/pip install pytest -q && .venv/bin/pytest
```

## Implementation Notes

### Swift
- Tests use **Swift Testing** (`@Suite`, `@Test`, `#expect`) — not XCTest
- `SnowedValue` enum is not `@frozen` — use `@unknown default` in exhaustive switches

### C#
- Reflection-based serializer for v1; source generation is planned for a future version
- `SnowedValue` uses `private protected` constructor to prevent external subclassing
- `InternalsVisibleTo("Snowed.Tests")` is set in `Snowed.csproj`

### Python
- No external dependencies; Python 3.10+ (uses structural pattern matching in internals)
- `dumps(obj)` / `loads(data)` + `dump(obj, fp)` / `load(fp)` streaming
- All plain `int` values encode as Int64 (0x04) by default
- `SnowedUInt64(n)` wrapper forces UInt64 (0x05); decoded UInt64 values return as `SnowedUInt64` to preserve round-trip fidelity
- `Annotated[int, snowed.uint64]` on dataclass fields encodes/decodes as UInt64; value is unwrapped to plain `int` on decode
- `bool` is a subclass of `int` in Python — encoder checks `bool` before `int`
- `datetime.datetime` must be timezone-aware; naive datetimes raise `SnowedEncodeError`
- `decimal.Decimal` → Decimal (0x12); canonical form computed via `as_tuple()` to avoid scientific notation
- `uuid.UUID.bytes` is already RFC 4122 order; no byte swap needed
- `SnowedValue` subclasses defined at module level, attached as class attributes (`SnowedValue.Null`, `SnowedValue.Bool`, etc.)
- `loads_dataclass(data, MyClass)` decodes directly into a `@dataclass` instance

### Go
- Standard library only, no external dependencies
- `Marshal` / `Unmarshal` + streaming `NewEncoder` / `NewDecoder` (io.Reader/Writer)
- `Marshaler` / `Unmarshaler` interfaces for custom types
- `snowed` struct tag: field rename, `-` to skip, `omitempty`, `decimal`, `uuid`
- `snowed.UUID` type ([16]byte); `ParseUUID` accepts hyphenated and unhyphenated forms
- `time.Time` maps to DateTime (0x09); always encoded/decoded as UTC
- Map/object keys sorted deterministically before writing
- Forward compatibility: unknown subtype bytes round down to base type (per §4.4)

### Kotlin
- KMP (Kotlin Multiplatform) — `commonMain` / `commonTest` + `jvmMain` / `jvmTest`
- Uses `kotlinx.serialization` (`BinaryFormat`); compiler plugin generates serializers for `@Serializable` classes
- `SnowedEncoderFacade` interface implemented by all three encoder classes (root, composite, map) so special-type serializers work regardless of nesting depth
- `SnowedObjectDecoder` pre-reads all fields into a map; values delivered via `SnowedValueDecoder` to handle field reordering
- JVM extras (`jvmMain`): `InputStream`/`OutputStream` streaming, `java.util.UUID`, `java.math.BigDecimal`, `java.time.Instant` conversions
- `SnowedFormat { }` DSL builder; configuration: `namingStrategy`, `encodeDefaults`, `unknownKeys`, `duplicateKeys`, `maxDepth`, `classDiscriminator`, `serializersModule`

### Rust
- `#![no_std]` + `alloc` — compatible with embedded targets (ESP32 via `esp-rs` / ESP-IDF)
- Zero mandatory dependencies
- `SnowedValue::Decimal` stores a `String` internally — no arithmetic library needed, matches the wire format directly
- Optional `serde` feature: `features = ["serde"]` — exposes `to_bytes` / `from_bytes` and `Serializer` / `Deserializer`; UUID/DateTime/Decimal use marker tokens (`$snowed::uuid`, etc.)
- Optional `rust_decimal` feature: `features = ["rust_decimal"]` — adds `From<rust_decimal::Decimal>` and `TryFrom<SnowedValue>` conversions
