package io.snowed

/**
 * Low-level writer that appends Snowed wire bytes to a [ByteArrayBuilder].
 * All multi-byte values are little-endian per the spec.
 */
internal class SnowedWriter {
    private val buf = ByteArrayBuilder()

    fun toByteArray(): ByteArray = buf.toByteArray()

    // ── Primitives ────────────────────────────────────────────────────────────

    fun writeNull() { buf.append(0x00) }
    fun writeBool(v: Boolean) { buf.append(if (v) 0x03 else 0x02) }

    fun writeInt64(v: Long) {
        buf.append(0x04)
        writeLE64(v.toULong())
    }

    fun writeUInt64(v: ULong) {
        buf.append(0x05)
        writeLE64(v)
    }

    fun writeFloat64(v: Double) {
        buf.append(0x08)
        writeLE64(v.toBits().toULong())
    }

    fun writeDateTime(ms: Double) {
        buf.append(0x09)
        writeLE64(ms.toBits().toULong())
    }

    fun writeBinary(bytes: ByteArray) {
        buf.append(0x10)
        writeLE64(bytes.size.toULong())
        buf.appendAll(bytes)
    }

    fun writeString(s: String) {
        val utf8 = s.encodeToByteArray()
        buf.append(0x11)
        writeLE64(utf8.size.toULong())
        buf.appendAll(utf8)
    }

    fun writeDecimal(s: String) {
        val utf8 = s.encodeToByteArray()
        buf.append(0x12)
        writeLE64(utf8.size.toULong())
        buf.appendAll(utf8)
    }

    fun writeUuid(bytes: ByteArray) {
        require(bytes.size == 16) { "UUID must be exactly 16 bytes" }
        buf.append(0x13)
        writeLE64(16u)
        buf.appendAll(bytes)
    }

    fun writeArrayBegin() { buf.append(0x40) }
    fun writeArrayEnd()   { buf.append(0xFF.toByte()) }

    fun writeObjectBegin() { buf.append(0x80) }

    fun writeObjectEnd() {
        buf.append(0x00) // zero-length key terminates object
    }

    fun writeKey(key: String) {
        val utf8 = key.encodeToByteArray()
        if (utf8.size > 255) throw SnowedEncodeException("Object key exceeds 255 bytes: \"$key\"")
        buf.append(utf8.size.toByte())
        buf.appendAll(utf8)
    }

    // ── SnowedValue ───────────────────────────────────────────────────────────

    fun writeValue(value: SnowedValue) {
        when (value) {
            is SnowedValue.Null     -> writeNull()
            is SnowedValue.Bool     -> writeBool(value.value)
            is SnowedValue.Int64    -> writeInt64(value.value)
            is SnowedValue.UInt64   -> writeUInt64(value.value)
            is SnowedValue.Float64  -> writeFloat64(value.value)
            is SnowedValue.DateTime -> writeDateTime(value.millisSinceEpoch)
            is SnowedValue.Binary   -> writeBinary(value.bytes)
            is SnowedValue.Str      -> writeString(value.value)
            is SnowedValue.Decimal  -> writeDecimal(value.value)
            is SnowedValue.Uuid     -> writeUuid(value.bytes)
            is SnowedValue.Array    -> {
                writeArrayBegin()
                value.elements.forEach { writeValue(it) }
                writeArrayEnd()
            }
            is SnowedValue.Object   -> {
                writeObjectBegin()
                value.entries.forEach { (k, v) ->
                    writeKey(k)
                    writeValue(v)
                }
                writeObjectEnd()
            }
        }
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    private fun writeLE64(v: ULong) {
        buf.append((v and 0xFFu).toByte())
        buf.append(((v shr 8)  and 0xFFu).toByte())
        buf.append(((v shr 16) and 0xFFu).toByte())
        buf.append(((v shr 24) and 0xFFu).toByte())
        buf.append(((v shr 32) and 0xFFu).toByte())
        buf.append(((v shr 40) and 0xFFu).toByte())
        buf.append(((v shr 48) and 0xFFu).toByte())
        buf.append(((v shr 56) and 0xFFu).toByte())
    }
}

/** Minimal growable byte buffer for KMP common code (no java.io dependency). */
internal class ByteArrayBuilder {
    private var buf = ByteArray(64)
    private var size = 0

    fun append(b: Byte) {
        ensureCapacity(1)
        buf[size++] = b
    }

    fun append(b: Int) { append(b.toByte()) }

    fun appendAll(bytes: ByteArray) {
        ensureCapacity(bytes.size)
        bytes.copyInto(buf, size)
        size += bytes.size
    }

    fun toByteArray(): ByteArray = buf.copyOf(size)

    private fun ensureCapacity(needed: Int) {
        if (size + needed > buf.size) {
            buf = buf.copyOf(maxOf(buf.size * 2, size + needed))
        }
    }
}
