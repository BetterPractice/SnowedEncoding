package io.snowed

import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.SerializationStrategy
import java.io.InputStream
import java.io.OutputStream
import java.math.BigDecimal
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.UUID

// ── Streaming I/O ─────────────────────────────────────────────────────────────

/**
 * Encodes [value] to [stream]. Buffers the encoded bytes in memory then
 * flushes to the stream — true streaming encode is a future enhancement
 * limited by the requirement to write the length prefix before the payload.
 */
fun <T> SnowedFormat.encodeTo(stream: OutputStream, serializer: SerializationStrategy<T>, value: T) {
    stream.write(encodeToByteArray(serializer, value))
}

/** Reified overload of [encodeTo]. */
inline fun <reified T> SnowedFormat.encodeTo(stream: OutputStream, value: T) {
    stream.write(encodeToByteArray(value))
}

/**
 * Reads a single Snowed value from [stream] by reading all available bytes.
 * For network streams, prefer the [ByteArray] overload once you have framed
 * the message boundary.
 */
fun <T> SnowedFormat.decodeFrom(stream: InputStream, deserializer: DeserializationStrategy<T>): T =
    decodeFromByteArray(deserializer, stream.readBytes())

/** Reified overload of [decodeFrom]. */
inline fun <reified T> SnowedFormat.decodeFrom(stream: InputStream): T =
    decodeFromByteArray(stream.readBytes())

// ── java.util.UUID ────────────────────────────────────────────────────────────

/**
 * Converts a [UUID] to a [SnowedUuid].
 *
 * The JVM stores UUID as two `Long` fields (`mostSignificantBits` /
 * `leastSignificantBits`). RFC 4122 byte order places the most-significant
 * 8 bytes first, each in big-endian order. Do NOT use `UUID.toByteArray()` —
 * it returns a mixed-endian layout incompatible with the Snowed spec.
 */
fun UUID.toSnowedUuid(): SnowedUuid {
    val buf = ByteBuffer.allocate(16).order(ByteOrder.BIG_ENDIAN)
    buf.putLong(mostSignificantBits)
    buf.putLong(leastSignificantBits)
    return SnowedUuid(buf.array())
}

/** Converts a [SnowedUuid] back to a [UUID]. */
fun SnowedUuid.toUuid(): UUID {
    val buf = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)
    return UUID(buf.getLong(), buf.getLong())
}

// ── java.math.BigDecimal ──────────────────────────────────────────────────────

/**
 * Converts a [BigDecimal] to a [SnowedDecimal] in canonical form (§5.9).
 *
 * Note: scale is not preserved — `1.00` encodes as `"1"`.
 */
fun BigDecimal.toSnowedDecimal(): SnowedDecimal =
    SnowedDecimal(stripTrailingZeros().toPlainString().let {
        // stripTrailingZeros() can produce "1E+2" for whole numbers — use toPlainString() to avoid that,
        // but also handle the edge case where the result is "-0" (normalise to "0").
        if (it == "-0") "0" else it
    })

/** Converts a [SnowedDecimal] back to a [BigDecimal]. */
fun SnowedDecimal.toBigDecimal(): BigDecimal = BigDecimal(value)

// ── java.time.Instant ────────────────────────────────────────────────────────

/**
 * Converts a [java.time.Instant] to a [SnowedDateTime].
 *
 * Sub-millisecond precision (nanoseconds) is silently lost — the Snowed
 * DateTime wire format stores milliseconds as a Float64.
 */
fun java.time.Instant.toSnowedDateTime(): SnowedDateTime =
    SnowedDateTime(toEpochMilli().toDouble())

/** Converts a [SnowedDateTime] back to a [java.time.Instant] (millisecond precision). */
fun SnowedDateTime.toInstant(): java.time.Instant =
    java.time.Instant.ofEpochMilli(millisSinceEpoch.toLong())
