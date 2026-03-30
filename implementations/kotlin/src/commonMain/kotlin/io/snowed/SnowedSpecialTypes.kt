package io.snowed

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder

// ── DateTime ──────────────────────────────────────────────────────────────────

/**
 * A point in time encoded as milliseconds since the Unix epoch (UTC) as
 * a [Double], matching the Snowed DateTime wire type (0x09).
 *
 * Negative values represent instants before the epoch. Fractional values
 * represent sub-millisecond precision.
 *
 * On the JVM, convert to/from `java.time.Instant` using the extension
 * functions in `SnowedJvmExtensions.kt`.
 */
@Serializable(with = SnowedDateTimeSerializer::class)
@JvmInline
value class SnowedDateTime(val millisSinceEpoch: Double)

internal object SnowedDateTimeSerializer : KSerializer<SnowedDateTime> {
    override val descriptor: SerialDescriptor =
        buildClassSerialDescriptor("io.snowed.SnowedDateTime")

    override fun serialize(encoder: Encoder, value: SnowedDateTime) {
        require(encoder is SnowedEncoderFacade) { "SnowedDateTime can only be serialized by SnowedEncoder" }
        encoder.encodeDateTime(value.millisSinceEpoch)
    }

    override fun deserialize(decoder: Decoder): SnowedDateTime = when (decoder) {
        is SnowedDecoder      -> SnowedDateTime(decoder.decodeDateTime())
        is SnowedValueDecoder -> SnowedDateTime(decoder.decodeDateTime())
        else -> throw SnowedDecodeException("SnowedDateTime can only be deserialized by a Snowed decoder")
    }
}

// ── Decimal ───────────────────────────────────────────────────────────────────

/**
 * An arbitrary-precision decimal number stored as a canonical UTF-8 string,
 * matching the Snowed Decimal wire type (0x12).
 *
 * The [value] string must conform to the canonical rules in §5.9 of the spec:
 * plain decimal notation, no exponent, no trailing zeros, no leading zeros,
 * negative zero normalised to `"0"`.
 *
 * On the JVM, convert to/from `java.math.BigDecimal` using:
 * ```kotlin
 * bigDecimal.stripTrailingZeros().toPlainString()
 * ```
 * Note: `BigDecimal` is JVM-only; in common code the string is the value.
 */
@Serializable(with = SnowedDecimalSerializer::class)
@JvmInline
value class SnowedDecimal(val value: String)

internal object SnowedDecimalSerializer : KSerializer<SnowedDecimal> {
    override val descriptor: SerialDescriptor =
        buildClassSerialDescriptor("io.snowed.SnowedDecimal")

    override fun serialize(encoder: Encoder, value: SnowedDecimal) {
        require(encoder is SnowedEncoderFacade) { "SnowedDecimal can only be serialized by SnowedEncoder" }
        encoder.encodeDecimal(value.value)
    }

    override fun deserialize(decoder: Decoder): SnowedDecimal = when (decoder) {
        is SnowedDecoder      -> SnowedDecimal(decoder.decodeDecimal())
        is SnowedValueDecoder -> SnowedDecimal(decoder.decodeDecimal())
        else -> throw SnowedDecodeException("SnowedDecimal can only be deserialized by a Snowed decoder")
    }
}

// ── UUID ──────────────────────────────────────────────────────────────────────

/**
 * A UUID stored as exactly 16 bytes in RFC 4122 byte order, matching the
 * Snowed UUID wire type (0x13).
 *
 * On the JVM, convert to/from `java.util.UUID` using the extension functions
 * in `SnowedJvmExtensions.kt`. The JVM's `UUID.toByteArray()` must not be
 * used directly — it returns a mixed-endian layout. Use the provided
 * extensions which write `mostSignificantBits` then `leastSignificantBits`
 * in big-endian order.
 */
@Serializable(with = SnowedUuidSerializer::class)
@JvmInline
value class SnowedUuid(val bytes: ByteArray) {
    init { require(bytes.size == 16) { "UUID must be exactly 16 bytes" } }
}

internal object SnowedUuidSerializer : KSerializer<SnowedUuid> {
    override val descriptor: SerialDescriptor =
        buildClassSerialDescriptor("io.snowed.SnowedUuid")

    override fun serialize(encoder: Encoder, value: SnowedUuid) {
        require(encoder is SnowedEncoderFacade) { "SnowedUuid can only be serialized by SnowedEncoder" }
        encoder.encodeUuid(value.bytes)
    }

    override fun deserialize(decoder: Decoder): SnowedUuid = when (decoder) {
        is SnowedDecoder      -> SnowedUuid(decoder.decodeUuid())
        is SnowedValueDecoder -> SnowedUuid(decoder.decodeUuid())
        else -> throw SnowedDecodeException("SnowedUuid can only be deserialized by a Snowed decoder")
    }
}
