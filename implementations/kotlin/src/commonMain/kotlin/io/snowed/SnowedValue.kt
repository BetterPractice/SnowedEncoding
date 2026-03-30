package io.snowed

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder

/**
 * A type-erased value in the Snowed binary format.
 *
 * Covers every type defined in the Snowed spec v0.1.0-draft. Use [SnowedFormat]
 * to encode and decode typed values; use this class when the document structure
 * is not known at compile time.
 */
@Serializable(with = SnowedValueSerializer::class)
sealed class SnowedValue {
    /** Null (0x00) */
    object Null : SnowedValue()

    /** Boolean (0x02 / 0x03) */
    data class Bool(val value: Boolean) : SnowedValue()

    /** Signed 64-bit integer (0x04) */
    data class Int64(val value: Long) : SnowedValue()

    /** Unsigned 64-bit integer (0x05) */
    data class UInt64(val value: ULong) : SnowedValue()

    /** IEEE 754 double-precision float (0x08) */
    data class Float64(val value: Double) : SnowedValue()

    /** Milliseconds since Unix epoch UTC, stored as Double (0x09) */
    data class DateTime(val millisSinceEpoch: Double) : SnowedValue()

    /** Raw binary payload (0x10) */
    data class Binary(val bytes: ByteArray) : SnowedValue() {
        override fun equals(other: Any?): Boolean =
            other is Binary && bytes.contentEquals(other.bytes)
        override fun hashCode(): Int = bytes.contentHashCode()
    }

    /** UTF-8 string (0x11) */
    data class Str(val value: String) : SnowedValue()

    /** Arbitrary-precision decimal, stored as a canonical string (0x12) */
    data class Decimal(val value: String) : SnowedValue()

    /** UUID — exactly 16 bytes in RFC 4122 byte order (0x13) */
    data class Uuid(val bytes: ByteArray) : SnowedValue() {
        init { require(bytes.size == 16) { "UUID must be exactly 16 bytes" } }
        override fun equals(other: Any?): Boolean =
            other is Uuid && bytes.contentEquals(other.bytes)
        override fun hashCode(): Int = bytes.contentHashCode()
    }

    /** Ordered sequence of values (0x40) */
    data class Array(val elements: List<SnowedValue>) : SnowedValue()

    /** Key-value map with UTF-8 string keys (0x80) */
    data class Object(val entries: Map<String, SnowedValue>) : SnowedValue()
}

/**
 * `KSerializer` for [SnowedValue]. Delegates to [SnowedEncoder] / [SnowedDecoder]
 * which understand the wire layout directly.
 */
internal object SnowedValueSerializer : KSerializer<SnowedValue> {
    override val descriptor: SerialDescriptor =
        buildClassSerialDescriptor("io.snowed.SnowedValue")

    override fun serialize(encoder: Encoder, value: SnowedValue) {
        require(encoder is SnowedEncoderFacade) { "SnowedValue can only be serialized by SnowedEncoder" }
        encoder.encodeSnowedValue(value)
    }

    override fun deserialize(decoder: Decoder): SnowedValue = when (decoder) {
        is SnowedDecoder      -> decoder.decodeSnowedValue()
        is SnowedValueDecoder -> decoder.decodeSnowedValue()
        else -> throw SnowedDecodeException("SnowedValue can only be deserialized by a Snowed decoder")
    }
}
