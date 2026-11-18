package io.snowed

/**
 * Low-level reader that parses Snowed wire bytes from a [ByteArray].
 * All multi-byte values are little-endian per the spec.
 */
internal class SnowedReader(private val data: ByteArray, private val maxDepth: Int = 64) {
    private var pos = 0
    private var depth = 0

    val remaining: Int get() = data.size - pos

    // ── Public entry point ────────────────────────────────────────────────────

    fun readRoot(): SnowedValue {
        val value = readValue()
        if (remaining > 0) throw SnowedDecodeException("Trailing bytes after root value")
        return value
    }

    // ── Type-specific readers (called by SnowedDecoder) ───────────────────────

    fun readByte(): Byte = takeByte()

    fun peekByte(): Byte {
        if (pos >= data.size) throw SnowedDecodeException("Unexpected EOF")
        return data[pos]
    }

    fun readTypeByte(): Int = takeByte().toInt() and 0xFF

    fun readLE64(): ULong {
        val b = takeBytes(8)
        return (b[0].toULong() and 0xFFu) or
               ((b[1].toULong() and 0xFFu) shl 8)  or
               ((b[2].toULong() and 0xFFu) shl 16) or
               ((b[3].toULong() and 0xFFu) shl 24) or
               ((b[4].toULong() and 0xFFu) shl 32) or
               ((b[5].toULong() and 0xFFu) shl 40) or
               ((b[6].toULong() and 0xFFu) shl 48) or
               ((b[7].toULong() and 0xFFu) shl 56)
    }

    fun readLengthPrefixedBytes(): ByteArray {
        val len = readLE64()
        val count = len.toLong().let {
            if (it < 0 || it > Int.MAX_VALUE) throw SnowedDecodeException("Length overflow: $len")
            it.toInt()
        }
        return takeBytes(count)
    }

    fun readLengthPrefixedString(): String {
        val bytes = readLengthPrefixedBytes()
        return decodeUtf8(bytes, "String")
    }

    // ── SnowedValue ───────────────────────────────────────────────────────────

    fun readValue(): SnowedValue {
        val typeByte = readTypeByte()
        return readValueWithType(typeByte)
    }

    fun readValueWithType(typeByte: Int): SnowedValue {
        if (++depth > maxDepth) throw SnowedDecodeException("Maximum nesting depth ($maxDepth) exceeded")
        try {
            return when (baseType(typeByte)) {
                0x00 -> when (typeByte) {
                    0x02 -> SnowedValue.Bool(false)
                    0x03 -> SnowedValue.Bool(true)
                    else -> SnowedValue.Null
                }
                0x04 -> when (typeByte) {
                    0x05 -> SnowedValue.UInt64(readLE64())
                    else -> SnowedValue.Int64(readLE64().toLong())
                }
                0x08 -> when (typeByte) {
                    0x09 -> SnowedValue.DateTime(Double.fromBits(readLE64().toLong()))
                    else -> SnowedValue.Float64(Double.fromBits(readLE64().toLong()))
                }
                0x10 -> when (typeByte) {
                    0x11 -> SnowedValue.Str(readLengthPrefixedString())
                    0x12 -> SnowedValue.Decimal(readLengthPrefixedString())
                    0x13 -> {
                        val len = readLE64()
                        if (len != 16uL) throw SnowedDecodeException("UUID length must be 16, got $len")
                        SnowedValue.Uuid(takeBytes(16))
                    }
                    else -> SnowedValue.Binary(readLengthPrefixedBytes())
                }
                0x40 -> {
                    val elements = mutableListOf<SnowedValue>()
                    while (true) {
                        val next = readTypeByte()
                        if (next == 0xFF) break
                        elements.add(readValueWithType(next))
                    }
                    SnowedValue.Array(elements)
                }
                0x80 -> {
                    val entries = mutableMapOf<String, SnowedValue>()
                    while (true) {
                        val keyLen = (takeByte().toInt() and 0xFF)
                        if (keyLen == 0x00) break
                        val keyBytes = takeBytes(keyLen)
                        val key = decodeUtf8(keyBytes, "Object key")
                        val valueType = readTypeByte()
                        if (valueType == 0xFF) break // Sentinel in value position
                        entries[key] = readValueWithType(valueType)
                    }
                    SnowedValue.Object(entries)
                }
                0xFF -> throw SnowedDecodeException("Unexpected Sentinel (0xFF) as root value")
                else -> throw SnowedDecodeException("Unhandled type byte 0x${typeByte.toString(16)}")
            }
        } finally {
            depth--
        }
    }

    /** Skip one complete value without materialising it. */
    fun skipValue() {
        val typeByte = readTypeByte()
        skipValueWithType(typeByte)
    }

    private fun skipValueWithType(typeByte: Int) {
        when (baseType(typeByte)) {
            0x00 -> Unit // no payload
            0x04 -> takeBytes(8)
            0x08 -> takeBytes(8)
            0x10 -> readLengthPrefixedBytes() // discard
            0x40 -> while (true) {
                val next = readTypeByte()
                if (next == 0xFF) break
                skipValueWithType(next)
            }
            0x80 -> while (true) {
                val keyLen = (takeByte().toInt() and 0xFF)
                if (keyLen == 0x00) break
                takeBytes(keyLen)
                val valueType = readTypeByte()
                if (valueType == 0xFF) break
                skipValueWithType(valueType)
            }
            0xFF -> throw SnowedDecodeException("Unexpected Sentinel while skipping")
        }
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    private fun takeByte(): Byte {
        if (pos >= data.size) throw SnowedDecodeException("Unexpected EOF")
        return data[pos++]
    }

    private fun takeBytes(count: Int): ByteArray {
        if (pos + count > data.size) throw SnowedDecodeException("Unexpected EOF")
        val slice = data.copyOfRange(pos, pos + count)
        pos += count
        return slice
    }

    companion object {
        /** §4.4 — round down to nearest base type. */
        fun baseType(b: Int): Int = when {
            b <= 0x03 -> 0x00
            b <= 0x07 -> 0x04
            b <= 0x0F -> 0x08
            b <= 0x3F -> 0x10
            b <= 0x7F -> 0x40
            b <= 0xFE -> 0x80
            else      -> 0xFF
        }
    }
}

private fun decodeUtf8(bytes: ByteArray, what: String): String =
    try {
        bytes.decodeToString(throwOnInvalidSequence = true)
    } catch (e: CharacterCodingException) {
        throw SnowedDecodeException("$what is not valid UTF-8", e)
    }
