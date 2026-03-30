package io.snowed

import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.StructureKind
import kotlinx.serialization.encoding.AbstractDecoder
import kotlinx.serialization.encoding.CompositeDecoder
import kotlinx.serialization.encoding.CompositeDecoder.Companion.DECODE_DONE
import kotlinx.serialization.encoding.CompositeDecoder.Companion.UNKNOWN_NAME
import kotlinx.serialization.modules.SerializersModule

/**
 * `kotlinx.serialization` [AbstractDecoder] that reads Snowed wire bytes.
 *
 * Used as the root decoder for scalar values and to initiate structure
 * decoding. Composite structures (objects, arrays, maps) are handled by
 * dedicated sub-decoders returned from [beginStructure].
 */
@OptIn(ExperimentalSerializationApi::class)
internal class SnowedDecoder(
    internal val reader: SnowedReader,
    internal val config: SnowedConfiguration,
    override val serializersModule: SerializersModule,
) : AbstractDecoder() {

    // AbstractDecoder requires this; never called on the root decoder directly.
    override fun decodeElementIndex(descriptor: SerialDescriptor): Int = DECODE_DONE

    // ── Primitives ────────────────────────────────────────────────────────────

    override fun decodeBoolean(): Boolean = when (val b = reader.readTypeByte()) {
        0x02 -> false
        0x03 -> true
        else -> throw SnowedDecodeException("Expected Bool, got 0x${b.toString(16)}")
    }

    override fun decodeByte(): Byte    = readInt64().toByte()
    override fun decodeShort(): Short  = readInt64().toShort()
    override fun decodeInt(): Int      = readInt64().toInt()
    override fun decodeLong(): Long    = readInt64()
    override fun decodeFloat(): Float  = readFloat64().toFloat()
    override fun decodeDouble(): Double = readFloat64()

    override fun decodeChar(): Char {
        val s = readString()
        if (s.length != 1) throw SnowedDecodeException("Expected single-char String for Char, got ${s.length} chars")
        return s[0]
    }

    override fun decodeString(): String = readString()

    override fun decodeNull(): Nothing? {
        val b = reader.readTypeByte()
        if (SnowedReader.baseType(b) != 0x00)
            throw SnowedDecodeException("Expected Null, got 0x${b.toString(16)}")
        return null
    }

    override fun decodeNotNullMark(): Boolean {
        val peeked = reader.peekByte().toInt() and 0xFF
        return SnowedReader.baseType(peeked) != 0x00
    }

    override fun decodeEnum(enumDescriptor: SerialDescriptor): Int {
        val name = readString()
        val index = enumDescriptor.getElementIndex(name)
        if (index == UNKNOWN_NAME)
            throw SnowedDecodeException("Unknown enum constant \"$name\" for ${enumDescriptor.serialName}")
        return index
    }

    override fun decodeInline(descriptor: SerialDescriptor): SnowedDecoder = this

    // ── Structures ────────────────────────────────────────────────────────────

    override fun beginStructure(descriptor: SerialDescriptor): CompositeDecoder {
        return when (descriptor.kind) {
            StructureKind.LIST -> {
                val b = reader.readTypeByte()
                if (SnowedReader.baseType(b) != 0x40)
                    throw SnowedDecodeException("Expected Array, got 0x${b.toString(16)}")
                SnowedListDecoder(reader, config, serializersModule)
            }
            StructureKind.MAP -> {
                val b = reader.readTypeByte()
                if (SnowedReader.baseType(b) != 0x80)
                    throw SnowedDecodeException("Expected Object for Map, got 0x${b.toString(16)}")
                SnowedMapDecoder(reader, config, serializersModule)
            }
            else -> {
                val b = reader.readTypeByte()
                if (SnowedReader.baseType(b) != 0x80)
                    throw SnowedDecodeException("Expected Object, got 0x${b.toString(16)}")
                SnowedObjectDecoder(reader, config, serializersModule)
            }
        }
    }

    // ── Special Snowed types (called by the value-class serializers) ──────────

    fun decodeSnowedValue(): SnowedValue = reader.readValue()
    fun decodeDateTime(): Double = readDateTimeMs()
    fun decodeDecimal(): String  = readDecimalString()
    fun decodeUuid(): ByteArray  = readUuidBytes()

    // ── Internal helpers ──────────────────────────────────────────────────────

    private fun readInt64(): Long {
        val b = reader.readTypeByte()
        if (SnowedReader.baseType(b) != 0x04)
            throw SnowedDecodeException("Expected Int64, got 0x${b.toString(16)}")
        return reader.readLE64().toLong()
    }

    private fun readFloat64(): Double {
        val b = reader.readTypeByte()
        if (SnowedReader.baseType(b) != 0x08)
            throw SnowedDecodeException("Expected Float64, got 0x${b.toString(16)}")
        return Double.fromBits(reader.readLE64().toLong())
    }

    private fun readString(): String {
        val b = reader.readTypeByte()
        if (SnowedReader.baseType(b) != 0x10)
            throw SnowedDecodeException("Expected String, got 0x${b.toString(16)}")
        return reader.readLengthPrefixedString()
    }

    private fun readDateTimeMs(): Double {
        val b = reader.readTypeByte()
        if (b != 0x09) throw SnowedDecodeException("Expected DateTime (0x09), got 0x${b.toString(16)}")
        return Double.fromBits(reader.readLE64().toLong())
    }

    private fun readDecimalString(): String {
        val b = reader.readTypeByte()
        if (b != 0x12) throw SnowedDecodeException("Expected Decimal (0x12), got 0x${b.toString(16)}")
        return reader.readLengthPrefixedString()
    }

    private fun readUuidBytes(): ByteArray {
        val b = reader.readTypeByte()
        if (b != 0x13) throw SnowedDecodeException("Expected UUID (0x13), got 0x${b.toString(16)}")
        val len = reader.readLE64()
        if (len != 16uL) throw SnowedDecodeException("UUID length must be 16, got $len")
        val bytes = ByteArray(16)
        repeat(16) { bytes[it] = reader.readByte() }
        return bytes
    }
}

// ── Object decoder (struct / data class) ─────────────────────────────────────

/**
 * Decodes a Snowed Object into a `@Serializable` class.
 *
 * Pre-reads all key-value pairs from the wire into a map so that field
 * ordering in the encoded data does not need to match the class definition.
 * Each field is delivered to its serializer via a [SnowedValueDecoder] wrapping
 * the pre-read [SnowedValue].
 */
@OptIn(ExperimentalSerializationApi::class)
internal class SnowedObjectDecoder(
    private val reader: SnowedReader,
    private val config: SnowedConfiguration,
    override val serializersModule: SerializersModule,
) : AbstractDecoder(), CompositeDecoder {

    private val fields: Map<String, SnowedValue>
    private var elementIndex = 0
    private lateinit var currentDescriptor: SerialDescriptor

    init {
        val raw = mutableMapOf<String, SnowedValue>()
        while (true) {
            val keyLen = reader.readByte().toInt() and 0xFF
            if (keyLen == 0x00) break
            val keyBytes = ByteArray(keyLen) { reader.readByte() }
            val key = keyBytes.decodeToString(throwOnInvalidSequence = true)
            val valueType = reader.readTypeByte()
            if (valueType == 0xFF) break  // Sentinel in value position
            val value = reader.readValueWithType(valueType)
            when (config.duplicateKeys) {
                DuplicateKeyHandling.FirstWins -> if (!raw.containsKey(key)) raw[key] = value
                DuplicateKeyHandling.LastWins  -> raw[key] = value
                DuplicateKeyHandling.Error     ->
                    if (raw.containsKey(key)) throw SnowedDecodeException("Duplicate key \"$key\"")
                    else raw[key] = value
            }
        }
        fields = raw
    }

    override fun decodeElementIndex(descriptor: SerialDescriptor): Int {
        currentDescriptor = descriptor
        while (elementIndex < descriptor.elementsCount) {
            val propName = descriptor.getElementName(elementIndex)
            val wireName = propName.applyNamingStrategy(config.namingStrategy)
            val idx = elementIndex++
            if (fields.containsKey(wireName)) return idx
            // Field absent — skip if optional (has default), else let kotlinx handle it
        }
        return DECODE_DONE
    }

    private fun currentValue(): SnowedValue {
        val propName = currentDescriptor.getElementName(elementIndex - 1)
        val wireName = propName.applyNamingStrategy(config.namingStrategy)
        return fields[wireName] ?: SnowedValue.Null
    }

    override fun decodeNotNullMark(): Boolean = currentValue() !is SnowedValue.Null

    override fun decodeNull(): Nothing? = null

    override fun <T> decodeSerializableElement(
        descriptor: SerialDescriptor,
        index: Int,
        deserializer: DeserializationStrategy<T>,
        previousValue: T?,
    ): T = deserializer.deserialize(SnowedValueDecoder(currentValue(), config, serializersModule))

    override fun decodeBoolean(): Boolean = (currentValue() as SnowedValue.Bool).value
    override fun decodeByte(): Byte       = (currentValue() as SnowedValue.Int64).value.toByte()
    override fun decodeShort(): Short     = (currentValue() as SnowedValue.Int64).value.toShort()
    override fun decodeInt(): Int         = (currentValue() as SnowedValue.Int64).value.toInt()
    override fun decodeLong(): Long       = (currentValue() as SnowedValue.Int64).value
    override fun decodeFloat(): Float     = coerceToDouble(currentValue()).toFloat()
    override fun decodeDouble(): Double   = coerceToDouble(currentValue())
    override fun decodeString(): String   = (currentValue() as SnowedValue.Str).value
    override fun decodeChar(): Char {
        val s = (currentValue() as SnowedValue.Str).value
        if (s.length != 1) throw SnowedDecodeException("Expected single-char for Char field")
        return s[0]
    }
    override fun decodeEnum(enumDescriptor: SerialDescriptor): Int {
        val name = (currentValue() as SnowedValue.Str).value
        return enumDescriptor.getElementIndex(name).also {
            if (it == UNKNOWN_NAME) throw SnowedDecodeException("Unknown enum constant \"$name\"")
        }
    }
    override fun decodeInline(descriptor: SerialDescriptor): AbstractDecoder = this

    override fun beginStructure(descriptor: SerialDescriptor): CompositeDecoder {
        val bytes = SnowedWriter().also { it.writeValue(currentValue()) }.toByteArray()
        return SnowedDecoder(SnowedReader(bytes, config.maxDepth), config, serializersModule)
            .beginStructure(descriptor)
    }

    override fun endStructure(descriptor: SerialDescriptor) {
        if (config.unknownKeys == UnknownKeyHandling.Error) {
            for (key in fields.keys) {
                val known = (0 until descriptor.elementsCount).any { i ->
                    descriptor.getElementName(i).applyNamingStrategy(config.namingStrategy) == key
                }
                if (!known) throw SnowedDecodeException(
                    "Unknown key \"$key\" in ${descriptor.serialName}"
                )
            }
        }
    }

    private fun coerceToDouble(v: SnowedValue): Double = when (v) {
        is SnowedValue.Float64 -> v.value
        is SnowedValue.Int64   -> v.value.toDouble()
        else -> throw SnowedDecodeException("Expected Float64, got $v")
    }
}

// ── List decoder ──────────────────────────────────────────────────────────────

@OptIn(ExperimentalSerializationApi::class)
internal class SnowedListDecoder(
    private val reader: SnowedReader,
    private val config: SnowedConfiguration,
    override val serializersModule: SerializersModule,
) : AbstractDecoder(), CompositeDecoder {

    private var index = 0
    private var done = false

    override fun decodeElementIndex(descriptor: SerialDescriptor): Int {
        if (done) return DECODE_DONE
        val peeked = reader.peekByte().toInt() and 0xFF
        if (peeked == 0xFF) {
            reader.readByte()
            done = true
            return DECODE_DONE
        }
        return index++
    }

    private fun elementDecoder() = SnowedDecoder(reader, config, serializersModule)

    override fun <T> decodeSerializableElement(
        descriptor: SerialDescriptor,
        index: Int,
        deserializer: DeserializationStrategy<T>,
        previousValue: T?,
    ): T = deserializer.deserialize(elementDecoder())

    override fun decodeBoolean(): Boolean  = elementDecoder().decodeBoolean()
    override fun decodeByte(): Byte        = elementDecoder().decodeByte()
    override fun decodeShort(): Short      = elementDecoder().decodeShort()
    override fun decodeInt(): Int          = elementDecoder().decodeInt()
    override fun decodeLong(): Long        = elementDecoder().decodeLong()
    override fun decodeFloat(): Float      = elementDecoder().decodeFloat()
    override fun decodeDouble(): Double    = elementDecoder().decodeDouble()
    override fun decodeString(): String    = elementDecoder().decodeString()
    override fun decodeNull(): Nothing?    = elementDecoder().decodeNull()
    override fun decodeNotNullMark(): Boolean = elementDecoder().decodeNotNullMark()

    override fun endStructure(descriptor: SerialDescriptor) {}
}

// ── Map decoder ───────────────────────────────────────────────────────────────

@OptIn(ExperimentalSerializationApi::class)
internal class SnowedMapDecoder(
    private val reader: SnowedReader,
    private val config: SnowedConfiguration,
    override val serializersModule: SerializersModule,
) : AbstractDecoder(), CompositeDecoder {

    private val pairs: List<Pair<String, SnowedValue>>
    private var pairIndex = 0
    private var nextIsKey = true

    init {
        val list = mutableListOf<Pair<String, SnowedValue>>()
        while (true) {
            val keyLen = reader.readByte().toInt() and 0xFF
            if (keyLen == 0x00) break
            val keyBytes = ByteArray(keyLen) { reader.readByte() }
            val key = keyBytes.decodeToString(throwOnInvalidSequence = true)
            val valueType = reader.readTypeByte()
            if (valueType == 0xFF) break
            val value = reader.readValueWithType(valueType)
            when (config.duplicateKeys) {
                DuplicateKeyHandling.Error ->
                    if (list.any { it.first == key }) throw SnowedDecodeException("Duplicate key \"$key\"")
                else -> {}
            }
            list.add(key to value)
        }
        pairs = when (config.duplicateKeys) {
            DuplicateKeyHandling.FirstWins -> {
                val seen = mutableSetOf<String>()
                list.filter { (k, _) -> seen.add(k) }
            }
            else -> list
        }
    }

    override fun decodeCollectionSize(descriptor: SerialDescriptor): Int = pairs.size

    override fun decodeElementIndex(descriptor: SerialDescriptor): Int {
        if (pairIndex >= pairs.size) return DECODE_DONE
        return pairIndex * 2 + if (nextIsKey) 0 else 1
    }

    override fun decodeString(): String {
        return if (nextIsKey) {
            nextIsKey = false
            pairs[pairIndex].first
        } else {
            val v = (pairs[pairIndex].second as SnowedValue.Str).value
            pairIndex++
            nextIsKey = true
            v
        }
    }

    override fun <T> decodeSerializableElement(
        descriptor: SerialDescriptor,
        index: Int,
        deserializer: DeserializationStrategy<T>,
        previousValue: T?,
    ): T {
        return if (nextIsKey) {
            nextIsKey = false
            @Suppress("UNCHECKED_CAST")
            pairs[pairIndex].first as T
        } else {
            val v = pairs[pairIndex].second
            pairIndex++
            nextIsKey = true
            deserializer.deserialize(SnowedValueDecoder(v, config, serializersModule))
        }
    }

    override fun endStructure(descriptor: SerialDescriptor) {}
}

// ── SnowedValue-backed decoder ────────────────────────────────────────────────

/**
 * Decodes from a pre-parsed [SnowedValue]. Used by [SnowedObjectDecoder] to
 * deliver individual field values to their serializers without re-reading
 * from the raw byte stream.
 *
 * For nested structures, re-encodes the [SnowedValue] to bytes and delegates
 * to a fresh [SnowedDecoder].
 */
@OptIn(ExperimentalSerializationApi::class)
internal class SnowedValueDecoder(
    private val value: SnowedValue,
    private val config: SnowedConfiguration,
    override val serializersModule: SerializersModule,
) : AbstractDecoder() {

    // Never called on a scalar value decoder.
    override fun decodeElementIndex(descriptor: SerialDescriptor): Int = DECODE_DONE

    override fun decodeBoolean(): Boolean = (value as SnowedValue.Bool).value
    override fun decodeByte(): Byte       = (value as SnowedValue.Int64).value.toByte()
    override fun decodeShort(): Short     = (value as SnowedValue.Int64).value.toShort()
    override fun decodeInt(): Int         = (value as SnowedValue.Int64).value.toInt()
    override fun decodeLong(): Long       = (value as SnowedValue.Int64).value
    override fun decodeFloat(): Float     = coerceToDouble().toFloat()
    override fun decodeDouble(): Double   = coerceToDouble()
    override fun decodeString(): String   = (value as SnowedValue.Str).value
    override fun decodeNull(): Nothing?   = null
    override fun decodeNotNullMark(): Boolean = value !is SnowedValue.Null
    override fun decodeInline(descriptor: SerialDescriptor): AbstractDecoder = this

    override fun decodeChar(): Char {
        val s = (value as SnowedValue.Str).value
        if (s.length != 1) throw SnowedDecodeException("Expected single-char for Char")
        return s[0]
    }

    override fun decodeEnum(enumDescriptor: SerialDescriptor): Int {
        val name = (value as SnowedValue.Str).value
        return enumDescriptor.getElementIndex(name).also {
            if (it == UNKNOWN_NAME) throw SnowedDecodeException("Unknown enum constant \"$name\"")
        }
    }

    override fun beginStructure(descriptor: SerialDescriptor): CompositeDecoder {
        val bytes = SnowedWriter().also { it.writeValue(value) }.toByteArray()
        return SnowedDecoder(SnowedReader(bytes, config.maxDepth), config, serializersModule)
            .beginStructure(descriptor)
    }

    // Called by SnowedDateTimeSerializer / SnowedDecimalSerializer / SnowedUuidSerializer
    // when the serializer receives this decoder.
    fun decodeSnowedValue(): SnowedValue = value
    fun decodeDateTime(): Double  = (value as SnowedValue.DateTime).millisSinceEpoch
    fun decodeDecimal(): String   = (value as SnowedValue.Decimal).value
    fun decodeUuid(): ByteArray   = (value as SnowedValue.Uuid).bytes

    private fun coerceToDouble(): Double = when (value) {
        is SnowedValue.Float64 -> value.value
        is SnowedValue.Int64   -> value.value.toDouble()
        else -> throw SnowedDecodeException("Expected Float64, got $value")
    }
}
