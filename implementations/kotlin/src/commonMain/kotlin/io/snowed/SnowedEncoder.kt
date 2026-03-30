package io.snowed

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerializationStrategy
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.StructureKind
import kotlinx.serialization.encoding.AbstractEncoder
import kotlinx.serialization.encoding.CompositeEncoder
import kotlinx.serialization.modules.SerializersModule

/**
 * Common interface implemented by all three Snowed encoder classes so that
 * custom serializers for Snowed-specific types (DateTime, Decimal, UUID,
 * SnowedValue) can call the right write method regardless of whether they
 * are encoding a top-level value, an object field, or a collection element.
 */
internal interface SnowedEncoderFacade {
    fun encodeSnowedValue(value: SnowedValue)
    fun encodeDateTime(ms: Double)
    fun encodeDecimal(s: String)
    fun encodeUuid(bytes: ByteArray)
}

/**
 * `kotlinx.serialization` [AbstractEncoder] that writes Snowed wire bytes.
 *
 * The root encoder handles scalars and top-level `beginStructure` dispatch.
 * Composite structures are handled by [SnowedCompositeEncoder] and
 * [SnowedMapEncoder].
 *
 * Key design: `encodeElement` (called by every `encode*Element` method in
 * `AbstractEncoder`) is where per-field key writing happens for Objects.
 * This ensures primitive fields (Long, Boolean, etc.) and complex fields
 * (nested objects, collections) all emit their key before the value.
 */
@OptIn(ExperimentalSerializationApi::class)
internal class SnowedEncoder(
    private val writer: SnowedWriter,
    private val config: SnowedConfiguration,
    override val serializersModule: SerializersModule,
) : AbstractEncoder(), SnowedEncoderFacade {

    // ── Primitives ────────────────────────────────────────────────────────────

    override fun encodeBoolean(value: Boolean) = writer.writeBool(value)
    override fun encodeByte(value: Byte)        = writer.writeInt64(value.toLong())
    override fun encodeShort(value: Short)      = writer.writeInt64(value.toLong())
    override fun encodeInt(value: Int)          = writer.writeInt64(value.toLong())
    override fun encodeLong(value: Long)        = writer.writeInt64(value)
    override fun encodeFloat(value: Float)      = writer.writeFloat64(value.toDouble())
    override fun encodeDouble(value: Double)    = writer.writeFloat64(value)
    override fun encodeChar(value: Char)        = writer.writeString(value.toString())
    override fun encodeString(value: String)    = writer.writeString(value)
    override fun encodeNull()                   = writer.writeNull()
    override fun encodeInline(descriptor: SerialDescriptor): SnowedEncoder = this

    override fun encodeEnum(enumDescriptor: SerialDescriptor, index: Int) {
        writer.writeString(enumDescriptor.getElementName(index))
    }

    override fun shouldEncodeElementDefault(descriptor: SerialDescriptor, index: Int): Boolean =
        config.encodeDefaults

    // ── Structures ────────────────────────────────────────────────────────────

    override fun beginStructure(descriptor: SerialDescriptor): CompositeEncoder {
        return when (descriptor.kind) {
            StructureKind.LIST -> {
                writer.writeArrayBegin()
                SnowedCompositeEncoder(writer, config, serializersModule, isObject = false)
            }
            StructureKind.MAP -> {
                writer.writeObjectBegin()
                SnowedMapEncoder(writer, config, serializersModule)
            }
            else -> {
                writer.writeObjectBegin()
                SnowedCompositeEncoder(writer, config, serializersModule, isObject = true)
            }
        }
    }

    // ── Special Snowed types (SnowedEncoderFacade) ────────────────────────────

    override fun encodeSnowedValue(value: SnowedValue) = writer.writeValue(value)
    override fun encodeDateTime(ms: Double)            = writer.writeDateTime(ms)
    override fun encodeDecimal(s: String)              = writer.writeDecimal(s)
    override fun encodeUuid(bytes: ByteArray)          = writer.writeUuid(bytes)
}

// ── Composite encoder (Object / Array body) ───────────────────────────────────

@OptIn(ExperimentalSerializationApi::class)
internal class SnowedCompositeEncoder(
    private val writer: SnowedWriter,
    private val config: SnowedConfiguration,
    override val serializersModule: SerializersModule,
    private val isObject: Boolean,
) : AbstractEncoder(), CompositeEncoder, SnowedEncoderFacade {

    // ── Key writing ───────────────────────────────────────────────────────────
    //
    // AbstractEncoder calls encodeElement(descriptor, index) before EVERY
    // encode*Element method (encodeLongElement, encodeStringElement,
    // encodeSerializableElement, etc.).  Writing the key here ensures it is
    // emitted exactly once for every field, regardless of the field's type.

    override fun encodeElement(descriptor: SerialDescriptor, index: Int): Boolean {
        if (isObject) {
            val name = descriptor.getElementName(index).applyNamingStrategy(config.namingStrategy)
            writer.writeKey(name)
        }
        return true
    }

    override fun shouldEncodeElementDefault(descriptor: SerialDescriptor, index: Int): Boolean =
        config.encodeDefaults

    // ── Primitives ────────────────────────────────────────────────────────────

    override fun encodeBoolean(value: Boolean) = writer.writeBool(value)
    override fun encodeByte(value: Byte)        = writer.writeInt64(value.toLong())
    override fun encodeShort(value: Short)      = writer.writeInt64(value.toLong())
    override fun encodeInt(value: Int)          = writer.writeInt64(value.toLong())
    override fun encodeLong(value: Long)        = writer.writeInt64(value)
    override fun encodeFloat(value: Float)      = writer.writeFloat64(value.toDouble())
    override fun encodeDouble(value: Double)    = writer.writeFloat64(value)
    override fun encodeChar(value: Char)        = writer.writeString(value.toString())
    override fun encodeString(value: String)    = writer.writeString(value)
    override fun encodeNull()                   = writer.writeNull()
    override fun encodeInline(descriptor: SerialDescriptor): SnowedCompositeEncoder = this

    override fun encodeEnum(enumDescriptor: SerialDescriptor, index: Int) {
        writer.writeString(enumDescriptor.getElementName(index))
    }

    // ── Nested structures ─────────────────────────────────────────────────────

    override fun beginStructure(descriptor: SerialDescriptor): CompositeEncoder {
        return when (descriptor.kind) {
            StructureKind.LIST -> {
                writer.writeArrayBegin()
                SnowedCompositeEncoder(writer, config, serializersModule, isObject = false)
            }
            StructureKind.MAP -> {
                writer.writeObjectBegin()
                SnowedMapEncoder(writer, config, serializersModule)
            }
            else -> {
                writer.writeObjectBegin()
                SnowedCompositeEncoder(writer, config, serializersModule, isObject = true)
            }
        }
    }

    override fun endStructure(descriptor: SerialDescriptor) {
        if (isObject) writer.writeObjectEnd() else writer.writeArrayEnd()
    }

    // ── Special Snowed types (SnowedEncoderFacade) ────────────────────────────

    override fun encodeSnowedValue(value: SnowedValue) = writer.writeValue(value)
    override fun encodeDateTime(ms: Double)            = writer.writeDateTime(ms)
    override fun encodeDecimal(s: String)              = writer.writeDecimal(s)
    override fun encodeUuid(bytes: ByteArray)          = writer.writeUuid(bytes)
}

// ── Map encoder ───────────────────────────────────────────────────────────────

/**
 * Handles `Map<String, V>` encoding.
 *
 * The `kotlinx.serialization` map serializer calls `encodeSerializableElement`
 * with alternating even (key) and odd (value) indices. Even-index calls receive
 * the key; odd-index calls receive the value. We track parity with [isKey].
 */
@OptIn(ExperimentalSerializationApi::class)
internal class SnowedMapEncoder(
    private val writer: SnowedWriter,
    private val config: SnowedConfiguration,
    override val serializersModule: SerializersModule,
) : AbstractEncoder(), CompositeEncoder, SnowedEncoderFacade {

    private var isKey = true

    // Map serializer calls encodeSerializableElement for every key and value.
    // We override it to handle the key/value alternation directly rather than
    // routing through encodeElement, since keys must be written as Snowed object
    // keys (1-byte length prefix) not as regular String values.
    override fun <T> encodeSerializableElement(
        descriptor: SerialDescriptor,
        index: Int,
        serializer: SerializationStrategy<T>,
        value: T,
    ) {
        if (isKey) {
            // Serialize the key into a temporary buffer and extract the string.
            val tmpWriter = SnowedWriter()
            val tmpEncoder = SnowedEncoder(tmpWriter, config, serializersModule)
            serializer.serialize(tmpEncoder, value)
            val keyBytes = tmpWriter.toByteArray()
            // Expected: 0x11 <8-byte-len> <utf8>
            if (keyBytes.size < 9 || keyBytes[0] != 0x11.toByte()) {
                throw SnowedEncodeException("Map key must be a String value")
            }
            val utf8 = keyBytes.copyOfRange(9, keyBytes.size)
            val keyStr = utf8.decodeToString()
            writer.writeKey(keyStr)
            isKey = false
        } else {
            serializer.serialize(SnowedCompositeEncoder(writer, config, serializersModule, isObject = false).also {
                // We want to write values inline, not wrapped in array.
                // Route through the root encoder for values.
            }.let { SnowedEncoder(writer, config, serializersModule) }, value)
            isKey = true
        }
    }

    override fun encodeBoolean(value: Boolean) = writer.writeBool(value)
    override fun encodeByte(value: Byte)        = writer.writeInt64(value.toLong())
    override fun encodeShort(value: Short)      = writer.writeInt64(value.toLong())
    override fun encodeInt(value: Int)          = writer.writeInt64(value.toLong())
    override fun encodeLong(value: Long)        = writer.writeInt64(value)
    override fun encodeFloat(value: Float)      = writer.writeFloat64(value.toDouble())
    override fun encodeDouble(value: Double)    = writer.writeFloat64(value)
    override fun encodeNull()                   = writer.writeNull()
    override fun encodeString(value: String)    = writer.writeString(value)
    override fun encodeInline(descriptor: SerialDescriptor): SnowedMapEncoder = this

    override fun endStructure(descriptor: SerialDescriptor) {
        writer.writeObjectEnd()
    }

    // ── Special Snowed types (SnowedEncoderFacade) ────────────────────────────

    override fun encodeSnowedValue(value: SnowedValue) = writer.writeValue(value)
    override fun encodeDateTime(ms: Double)            = writer.writeDateTime(ms)
    override fun encodeDecimal(s: String)              = writer.writeDecimal(s)
    override fun encodeUuid(bytes: ByteArray)          = writer.writeUuid(bytes)
}
