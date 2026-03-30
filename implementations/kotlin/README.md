# Snowed — Kotlin Implementation

Kotlin implementation of the [Snowed binary encoding format](../../spec/snowed-spec.md).

## Requirements

- Kotlin 2.0+
- JVM 21+ (primary target)
- `kotlinx.serialization` 1.7+ (compiler plugin + runtime)
- No runtime dependencies beyond the Kotlin standard library and `kotlinx.serialization`

---

## Architecture

This implementation is built on [`kotlinx.serialization`](https://github.com/Kotlin/kotlinx.serialization).
Rather than walking object graphs using reflection, `SnowedFormat` implements the
`BinaryFormat` interface — the `kotlinx.serialization` compiler plugin generates
`KSerializer` instances for `@Serializable` classes, and those serializers call into
`SnowedEncoder` / `SnowedDecoder` to produce and consume the wire bytes.

This means:

- No reflection at runtime — safe for use with R8 / ProGuard.
- Works with `@Serializable` data classes, sealed classes, objects, and value classes.
- Custom serializers integrate via `SerializersModule`, not a converter list.
- Polymorphic class hierarchies are supported natively via `@Polymorphic` /
  `@SerialName` without any additional configuration.

---

## Usage

### Basic encode and decode

```kotlin
import io.snowed.SnowedFormat

@Serializable
data class SensorReading(
    val deviceId: String,
    val temperature: Double,
    val recordedAt: SnowedDateTime,
)

val snowed = SnowedFormat()

// Encode
val bytes: ByteArray = snowed.encodeToByteArray(reading)

// Decode
val reading: SensorReading = snowed.decodeFromByteArray(bytes)
```

The `encodeToByteArray` / `decodeFromByteArray` extension functions use reified
type parameters and are the primary entry points. The lower-level
`BinaryFormat` overloads that accept an explicit `KSerializer` are also available:

```kotlin
snowed.encodeToByteArray(SensorReading.serializer(), reading)
snowed.decodeFromByteArray(SensorReading.serializer(), bytes)
```

### JVM streaming (JVM target only)

The `BinaryFormat` interface operates on `ByteArray`. On the JVM, extension
functions are provided for `InputStream` and `OutputStream`:

```kotlin
// Encode to stream
snowed.encodeTo(outputStream, reading)

// Decode from stream — reads exactly one value; does not buffer the whole stream
val reading: SensorReading = snowed.decodeFrom(inputStream)
```

`decodeFrom` takes advantage of the length-prefixed Binary wire layout: for large
binary blobs the decoder reads the 8-byte length prefix and streams the payload
directly into the target field without materialising the entire document. See
[Large binary payloads](#large-binary-payloads) below.

### Untyped documents

When no target type is known at compile time, decode into `SnowedValue`:

```kotlin
val value: SnowedValue = snowed.decodeFromByteArray(bytes)

when (value) {
    is SnowedValue.Object -> value.entries["name"]?.let { println(it) }
    is SnowedValue.Array  -> value.elements.forEach { println(it) }
    is SnowedValue.Str    -> println(value.value)
    else -> {}
}
```

Encode a `SnowedValue` directly:

```kotlin
val doc = SnowedValue.Object(mapOf(
    "x" to SnowedValue.Int64(1),
    "y" to SnowedValue.Int64(2),
))
val bytes = snowed.encodeToByteArray(doc)
```

---

## Configuration

`SnowedFormat` is created with an optional configuration lambda following the
`kotlinx.serialization` convention:

```kotlin
val snowed = SnowedFormat {
    namingStrategy     = SnowedNamingStrategy.CamelCase
    unknownKeys        = UnknownKeyHandling.Ignore
    duplicateKeys      = DuplicateKeyHandling.LastWins
    encodeDefaults     = false
    coerceInputValues  = false
    maxDepth           = 64
    classDiscriminator = "type"
    serializersModule  = SerializersModule {
        contextual(MyCustomSerializer)
    }
}
```

### `namingStrategy`

**Type:** `SnowedNamingStrategy?`
**Default:** `null` (property names used as-is)

Controls how Kotlin property names are mapped to Snowed object keys on both
encode and decode. A `@SerialName` annotation on a property always takes
precedence over the naming strategy.

| Value | Behaviour |
|---|---|
| `null` | Property name used unchanged |
| `SnowedNamingStrategy.CamelCase` | `PascalCase` → `camelCase` |
| `SnowedNamingStrategy.SnakeCase` | `camelCase` / `PascalCase` → `snake_case` |

### `unknownKeys`

**Type:** `UnknownKeyHandling`
**Default:** `UnknownKeyHandling.Ignore`

Behaviour when the decoder encounters an object key that has no matching
property in the target `@Serializable` class.

| Value | Behaviour |
|---|---|
| `Ignore` | The key and its value are skipped silently |
| `Error` | A `SnowedDecodeException` is thrown |

### `duplicateKeys`

**Type:** `DuplicateKeyHandling`
**Default:** `DuplicateKeyHandling.LastWins`

Behaviour when the decoder encounters the same key more than once within a
single object. Encoders must not produce duplicate keys.

| Value | Behaviour |
|---|---|
| `FirstWins` | The first occurrence is used; subsequent duplicates are skipped |
| `LastWins` | Each occurrence overwrites the previous; the last value is used |
| `Error` | A `SnowedDecodeException` is thrown |

### `encodeDefaults`

**Type:** `Boolean`
**Default:** `false`

When `false`, properties whose value equals the Kotlin default value (as
declared in the constructor) are omitted from the encoded object. When `true`,
all properties are written regardless of value.

Set to `true` when interoperating with implementations that expect all fields to
be present, or when using `@Required` is not sufficient.

### `coerceInputValues`

**Type:** `Boolean`
**Default:** `false`

When `true`, if a non-nullable property in the target class receives a Snowed
`Null` value from the wire, the decoder substitutes the property's default value
instead of throwing. If the property has no default, decoding still fails.

This is useful for schema evolution where a field was `null`-capable in an older
payload but is now non-nullable in the current data class.

### `maxDepth`

**Type:** `Int`
**Default:** `64`

Maximum nesting depth for Arrays and Objects before a `SnowedDecodeException`
is thrown. Guards against stack overflows from pathologically or maliciously
nested documents.

### `classDiscriminator`

**Type:** `String`
**Default:** `"type"`

The object key used to encode the concrete class name when serializing
polymorphic class hierarchies. Only relevant when `@Polymorphic` /
`@SerialName` are used.

```kotlin
@Serializable
@SerialName("circle")
data class Circle(val radius: Double) : Shape()

// Encoded as: { "type": "circle", "radius": 2.5 }
```

### `serializersModule`

**Type:** `SerializersModule`
**Default:** `EmptySerializersModule()`

The extension point for custom type serializers and polymorphic class
hierarchies. Multiple modules can be combined with `plus`:

```kotlin
val snowed = SnowedFormat {
    serializersModule = myModule + anotherModule
}
```

This replaces the `Converters` list found in the C# implementation with a
composable mechanism: libraries can each ship their own `SerializersModule` and
callers combine them without coordination.

---

## Special Snowed Types

Snowed defines types that have no direct Kotlin equivalent. These are
represented as value class wrappers with bundled serializers so they integrate
naturally with `@Serializable` data classes.

### DateTime

```kotlin
@Serializable(with = SnowedDateTimeSerializer::class)
@JvmInline
value class SnowedDateTime(val millisSinceEpoch: Double)
```

Encodes as wire type `0x09`. Stores milliseconds since the Unix epoch (UTC) as
a `Double`, matching the wire representation directly. Negative values represent
instants before the epoch.

**Converting from `java.time.Instant`:**

```kotlin
fun Instant.toSnowedDateTime() = SnowedDateTime(toEpochMilli().toDouble())
fun SnowedDateTime.toInstant() = Instant.ofEpochMilli(millisSinceEpoch.toLong())
```

Sub-millisecond precision in the `Double` value is preserved on the wire but
lost when converting to `Instant` (millisecond resolution). See
[platform-limitations.md](../../spec/platform-limitations.md).

### Decimal

```kotlin
@Serializable(with = SnowedDecimalSerializer::class)
@JvmInline
value class SnowedDecimal(val value: String)
```

Encodes as wire type `0x12`. The `value` string must be in canonical form per
§5.9 of the spec. No arithmetic is performed by the implementation; the string
is the wire representation.

**Converting from `java.math.BigDecimal`:**

```kotlin
fun BigDecimal.toSnowedDecimal() = SnowedDecimal(stripTrailingZeros().toPlainString())
fun SnowedDecimal.toBigDecimal() = BigDecimal(value)
```

`BigDecimal` is a JVM type. In Kotlin Multiplatform common code, decode `0x12`
as `String` directly. See [platform-limitations.md](../../spec/platform-limitations.md).

### UUID

```kotlin
@Serializable(with = SnowedUuidSerializer::class)
@JvmInline
value class SnowedUuid(val bytes: ByteArray) {
    init { require(bytes.size == 16) { "UUID must be exactly 16 bytes" } }
}
```

Encodes as wire type `0x13` with a fixed 16-byte payload in RFC 4122 byte order.

**Converting from `java.util.UUID`:**

```kotlin
fun UUID.toSnowedUuid(): SnowedUuid {
    val buf = ByteBuffer.allocate(16).order(ByteOrder.BIG_ENDIAN)
    buf.putLong(mostSignificantBits)
    buf.putLong(leastSignificantBits)
    return SnowedUuid(buf.array())
}

fun SnowedUuid.toUuid(): UUID {
    val buf = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)
    return UUID(buf.getLong(), buf.getLong())
}
```

See [platform-limitations.md](../../spec/platform-limitations.md) for the
byte-order details.

---

## SnowedValue

`SnowedValue` is a sealed class covering every Snowed type. It is the untyped
document model, analogous to `JsonElement` in `kotlinx.serialization`.

```kotlin
sealed class SnowedValue {
    object Null                                          : SnowedValue()
    data class Bool(val value: Boolean)                  : SnowedValue()
    data class Int64(val value: Long)                    : SnowedValue()
    data class UInt64(val value: ULong)                  : SnowedValue()
    data class Float64(val value: Double)                : SnowedValue()
    data class DateTime(val millisSinceEpoch: Double)    : SnowedValue()
    data class Binary(val bytes: ByteArray)              : SnowedValue()
    data class Str(val value: String)                    : SnowedValue()
    data class Decimal(val value: String)                : SnowedValue()
    data class Uuid(val bytes: ByteArray)                : SnowedValue()
    data class Array(val elements: List<SnowedValue>)    : SnowedValue()
    data class Object(val entries: Map<String, SnowedValue>) : SnowedValue()
}
```

`SnowedValue` is itself `@Serializable`, so it can appear as a property type in
any `@Serializable` data class where the field type is not known at compile time.

---

## Large Binary Payloads

Binary blobs encoded as `0x10` carry a 64-bit length prefix, supporting payloads
up to 16 exabytes. In practice, holding multi-gigabyte payloads in a `ByteArray`
is impractical. On the JVM, the streaming decode path (`decodeFrom(InputStream)`)
addresses this:

- After reading the 8-byte length prefix the decoder wraps the underlying stream
  in a `LimitedInputStream(stream, length)`.
- The target property type can be `InputStream` to receive the bounded stream
  directly rather than a materialised `ByteArray`.
- The caller is responsible for consuming or discarding the stream before
  decoding continues.

On the encode side, declaring the length upfront is required by the wire format.
When encoding from a stream, callers must provide the content length:

```kotlin
snowed.encodeBinaryField(outputStream, contentLength = fileSize, source = fileInputStream)
```

---

## Polymorphism

Sealed class hierarchies work out of the box with `@Polymorphic` and
`@SerialName`. The class discriminator key defaults to `"type"` (configurable
via `classDiscriminator`).

```kotlin
@Serializable
sealed class Event

@Serializable
@SerialName("login")
data class LoginEvent(val userId: String) : Event()

@Serializable
@SerialName("logout")
data class LogoutEvent(val userId: String, val durationMs: Long) : Event()

// Encoded LoginEvent: { "type": "login", "userId": "abc123" }
val event: Event = snowed.decodeFromByteArray(bytes)
```

Open polymorphism (non-sealed class hierarchies) requires registration in
`serializersModule`:

```kotlin
val snowed = SnowedFormat {
    serializersModule = SerializersModule {
        polymorphic(Event::class) {
            subclass(LoginEvent::class)
            subclass(LogoutEvent::class)
        }
    }
}
```

---

## Exceptions

| Exception | Thrown when |
|---|---|
| `SnowedEncodeException` | Encoding fails (e.g. key exceeds 255 bytes) |
| `SnowedDecodeException` | Decoding fails (e.g. invalid UTF-8, unexpected EOF, unknown key with `Error` handling, depth exceeded) |

Both extend `SnowedException`.

---

## Platform limitations

See [../../spec/platform-limitations.md](../../spec/platform-limitations.md)
for the full list. Key points for Kotlin:

- `BigDecimal` is JVM-only; common code uses `String` for decimal values.
- `java.util.UUID` requires a byte-order conversion — use the `toSnowedUuid()`
  extension rather than accessing `toByteArray()` directly.
- `ULong` values above `Long.MAX_VALUE` require care at JVM bytecode boundaries.
- `LocalDateTime` must not be used for DateTime fields; use `Instant` or
  `SnowedDateTime` directly.
- `Duration` has no Snowed equivalent; encode as Int64 milliseconds by
  convention.

---

## Running tests

```sh
cd implementations/kotlin
./gradlew test
```
