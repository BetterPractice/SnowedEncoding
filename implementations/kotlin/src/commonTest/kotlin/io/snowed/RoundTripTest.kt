package io.snowed

import kotlinx.serialization.Serializable
import kotlin.test.*

private val snowed = SnowedFormat()

// ── Helpers ───────────────────────────────────────────────────────────────────

private inline fun <reified T> roundTrip(value: T): T {
    val bytes = snowed.encodeToByteArray(value)
    return snowed.decodeFromByteArray(bytes)
}

// Preferred overload for SnowedValue and its subtypes — avoids serializer-not-found
// errors that occur when the reified T is inferred as a concrete sealed subclass.
private fun roundTrip(value: SnowedValue): SnowedValue {
    val bytes = snowed.encodeToByteArray<SnowedValue>(value)
    return snowed.decodeFromByteArray<SnowedValue>(bytes)
}

private fun assertWire(expected: String, value: SnowedValue) {
    val expectedBytes = expected.trim().split(Regex("\\s+"))
        .map { it.toInt(16).toByte() }.toByteArray()
    val writer = SnowedWriter()
    writer.writeValue(value)
    assertContentEquals(expectedBytes, writer.toByteArray())
}

// ── SnowedValue round-trip ────────────────────────────────────────────────────

class SnowedValueRoundTripTest {

    @Test fun nullValue() = assertEquals(SnowedValue.Null, roundTrip(SnowedValue.Null))
    @Test fun boolFalse() = assertEquals(SnowedValue.Bool(false), roundTrip(SnowedValue.Bool(false)))
    @Test fun boolTrue()  = assertEquals(SnowedValue.Bool(true),  roundTrip(SnowedValue.Bool(true)))

    @Test fun int64Zero()     = assertEquals(SnowedValue.Int64(0),       roundTrip(SnowedValue.Int64(0)))
    @Test fun int64Positive() = assertEquals(SnowedValue.Int64(42),      roundTrip(SnowedValue.Int64(42)))
    @Test fun int64Negative() = assertEquals(SnowedValue.Int64(-1),      roundTrip(SnowedValue.Int64(-1)))
    @Test fun int64Min()      = assertEquals(SnowedValue.Int64(Long.MIN_VALUE), roundTrip(SnowedValue.Int64(Long.MIN_VALUE)))
    @Test fun int64Max()      = assertEquals(SnowedValue.Int64(Long.MAX_VALUE), roundTrip(SnowedValue.Int64(Long.MAX_VALUE)))

    @Test fun uint64Zero()       = assertEquals(SnowedValue.UInt64(0uL),        roundTrip(SnowedValue.UInt64(0uL)))
    @Test fun uint64Max()        = assertEquals(SnowedValue.UInt64(ULong.MAX_VALUE), roundTrip(SnowedValue.UInt64(ULong.MAX_VALUE)))
    @Test fun uint64AboveI64Max() {
        val v = Long.MAX_VALUE.toULong() + 1uL
        assertEquals(SnowedValue.UInt64(v), roundTrip(SnowedValue.UInt64(v)))
    }

    @Test fun float64Pi()          = assertEquals(SnowedValue.Float64(3.14), roundTrip(SnowedValue.Float64(3.14)))
    @Test fun float64Infinity()    = assertEquals(SnowedValue.Float64(Double.POSITIVE_INFINITY), roundTrip(SnowedValue.Float64(Double.POSITIVE_INFINITY)))
    @Test fun float64NegInfinity() = assertEquals(SnowedValue.Float64(Double.NEGATIVE_INFINITY), roundTrip(SnowedValue.Float64(Double.NEGATIVE_INFINITY)))
    @Test fun float64NaN() {
        val rt = roundTrip(SnowedValue.Float64(Double.NaN))
        assertTrue((rt as SnowedValue.Float64).value.isNaN())
    }
    @Test fun float64NegativeZero() {
        val rt = roundTrip(SnowedValue.Float64(-0.0))
        assertTrue((rt as SnowedValue.Float64).value.equals(-0.0))
    }

    @Test fun dateTimeEpoch()    = assertEquals(SnowedValue.DateTime(0.0),          roundTrip(SnowedValue.DateTime(0.0)))
    @Test fun dateTimePositive() = assertEquals(SnowedValue.DateTime(1_700_000_000_000.0), roundTrip(SnowedValue.DateTime(1_700_000_000_000.0)))
    @Test fun dateTimeNegative() = assertEquals(SnowedValue.DateTime(-1_000_000.0), roundTrip(SnowedValue.DateTime(-1_000_000.0)))

    @Test fun binaryEmpty()   = assertEquals(SnowedValue.Binary(byteArrayOf()), roundTrip(SnowedValue.Binary(byteArrayOf())))
    @Test fun binaryPayload() {
        val v = SnowedValue.Binary(byteArrayOf(0xDE.toByte(), 0xAD.toByte(), 0xBE.toByte(), 0xEF.toByte()))
        assertEquals(v, roundTrip(v))
    }

    @Test fun stringEmpty()   = assertEquals(SnowedValue.Str(""),         roundTrip(SnowedValue.Str("")))
    @Test fun stringHello()   = assertEquals(SnowedValue.Str("hello"),    roundTrip(SnowedValue.Str("hello")))
    @Test fun stringUnicode() = assertEquals(SnowedValue.Str("こんにちは🌨️"), roundTrip(SnowedValue.Str("こんにちは🌨️")))

    @Test fun decimalInteger()    = assertEquals(SnowedValue.Decimal("42"),       roundTrip(SnowedValue.Decimal("42")))
    @Test fun decimalFractional() = assertEquals(SnowedValue.Decimal("3.14159"),  roundTrip(SnowedValue.Decimal("3.14159")))
    @Test fun decimalNegative()   = assertEquals(SnowedValue.Decimal("-0.001"),   roundTrip(SnowedValue.Decimal("-0.001")))
    @Test fun decimalZero()       = assertEquals(SnowedValue.Decimal("0"),        roundTrip(SnowedValue.Decimal("0")))

    @Test fun uuid() {
        val bytes = byteArrayOf(
            0x55, 0x0e, 0x84.toByte(), 0x00, 0xe2.toByte(), 0x9b.toByte(),
            0x41, 0xd4.toByte(), 0xa7.toByte(), 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
        )
        assertEquals(SnowedValue.Uuid(bytes), roundTrip(SnowedValue.Uuid(bytes)))
    }

    @Test fun arrayEmpty()  = assertEquals(SnowedValue.Array(emptyList()), roundTrip(SnowedValue.Array(emptyList())))
    @Test fun arrayMixed() {
        val v = SnowedValue.Array(listOf(SnowedValue.Bool(true), SnowedValue.Int64(42), SnowedValue.Str("hi")))
        assertEquals(v, roundTrip(v))
    }
    @Test fun arrayNested() {
        val inner = SnowedValue.Array(listOf(SnowedValue.Int64(1), SnowedValue.Int64(2)))
        val outer = SnowedValue.Array(listOf(inner, SnowedValue.Null))
        assertEquals(outer, roundTrip(outer))
    }

    @Test fun objectEmpty() = assertEquals(SnowedValue.Object(emptyMap()), roundTrip(SnowedValue.Object(emptyMap())))
    @Test fun objectSingleKey() {
        val v = SnowedValue.Object(mapOf("x" to SnowedValue.Int64(1)))
        assertEquals(v, roundTrip(v))
    }
    @Test fun objectNested() {
        val v = SnowedValue.Object(mapOf(
            "inner" to SnowedValue.Object(mapOf("a" to SnowedValue.Bool(true))),
            "n" to SnowedValue.Null,
        ))
        assertEquals(v, roundTrip(v))
    }
}

// ── Wire-byte tests ───────────────────────────────────────────────────────────

class WireFormatTest {
    @Test fun wireNull()      = assertWire("00", SnowedValue.Null)
    @Test fun wireBoolFalse() = assertWire("02", SnowedValue.Bool(false))
    @Test fun wireBoolTrue()  = assertWire("03", SnowedValue.Bool(true))
    @Test fun wireInt64_42()  = assertWire("04 2A 00 00 00 00 00 00 00", SnowedValue.Int64(42))
    @Test fun wireInt64Minus1() = assertWire("04 FF FF FF FF FF FF FF FF", SnowedValue.Int64(-1))
    @Test fun wireUInt64Max() = assertWire("05 FF FF FF FF FF FF FF FF", SnowedValue.UInt64(ULong.MAX_VALUE))
    @Test fun wireFloat64_314() = assertWire("08 1F 85 EB 51 B8 1E 09 40", SnowedValue.Float64(3.14))
    @Test fun wireBinaryDeadBeef() = assertWire(
        "10 04 00 00 00 00 00 00 00 DE AD BE EF",
        SnowedValue.Binary(byteArrayOf(0xDE.toByte(), 0xAD.toByte(), 0xBE.toByte(), 0xEF.toByte()))
    )
    @Test fun wireStringHello() = assertWire(
        "11 05 00 00 00 00 00 00 00 68 65 6C 6C 6F",
        SnowedValue.Str("hello")
    )
    @Test fun wireUuid() = assertWire(
        "13 10 00 00 00 00 00 00 00 55 0E 84 00 E2 9B 41 D4 A7 16 44 66 55 44 00 00",
        SnowedValue.Uuid(byteArrayOf(
            0x55, 0x0e, 0x84.toByte(), 0x00, 0xe2.toByte(), 0x9b.toByte(),
            0x41, 0xd4.toByte(), 0xa7.toByte(), 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
        ))
    )
    @Test fun wireEmptyArray()  = assertWire("40 FF", SnowedValue.Array(emptyList()))
    @Test fun wireEmptyObject() = assertWire("80 00", SnowedValue.Object(emptyMap()))
    @Test fun wireObjectX1() = assertWire(
        "80 01 78 04 01 00 00 00 00 00 00 00 00",
        SnowedValue.Object(mapOf("x" to SnowedValue.Int64(1)))
    )
}

// ── Forward-compatibility tests (§4.4) ───────────────────────────────────────

class ForwardCompatTest {
    private fun decode(vararg bytes: Int): SnowedValue =
        SnowedReader(bytes.map { it.toByte() }.toByteArray()).readRoot()

    @Test fun unknownNullSubtype0x01()   = assertEquals(SnowedValue.Null, decode(0x01))
    @Test fun unknownIntSubtype0x06() {
        val bytes = intArrayOf(0x06) + IntArray(8) { if (it == 0) 42 else 0 }
        assertEquals(SnowedValue.Int64(42), decode(*bytes))
    }
    @Test fun unknownFloatSubtype0x0A() {
        val bits = 3.14.toBits()
        val bytes = intArrayOf(0x0A) + (0 until 8).map { ((bits shr (it * 8)) and 0xFF).toInt() }.toIntArray()
        assertEquals(SnowedValue.Float64(3.14), decode(*bytes))
    }
    @Test fun unknownBinarySubtype0x14() {
        val payload = byteArrayOf(0x74, 0x65, 0x73, 0x74) // "test"
        val bytes = intArrayOf(0x14, 4, 0, 0, 0, 0, 0, 0, 0) + payload.map { it.toInt() and 0xFF }.toIntArray()
        assertEquals(SnowedValue.Binary(payload), decode(*bytes))
    }
    @Test fun unknownArraySubtype0x41() {
        // 0x41 [true] 0xFF
        assertEquals(SnowedValue.Array(listOf(SnowedValue.Bool(true))), decode(0x41, 0x03, 0xFF))
    }
    @Test fun unknownObjectSubtype0xC0() {
        val bytes = intArrayOf(0xC0, 0x01, 'x'.code, 0x04, 1, 0, 0, 0, 0, 0, 0, 0, 0x00)
        assertEquals(SnowedValue.Object(mapOf("x" to SnowedValue.Int64(1))), decode(*bytes))
    }
}

// ── Decode error tests ────────────────────────────────────────────────────────

class DecodeErrorTest {
    private fun decode(vararg bytes: Int): SnowedValue =
        SnowedReader(bytes.map { it.toByte() }.toByteArray()).readRoot()

    @Test fun unexpectedEof() {
        assertFailsWith<SnowedDecodeException> { decode(0x04, 0x00, 0x00) } // Int64 needs 8 bytes
    }
    @Test fun trailingBytes() {
        assertFailsWith<SnowedDecodeException> { decode(0x00, 0x00) } // null + extra byte
    }
    @Test fun unexpectedSentinel() {
        assertFailsWith<SnowedDecodeException> { decode(0xFF) }
    }
    @Test fun uuidWrongLength() {
        val bytes = intArrayOf(0x13, 15, 0, 0, 0, 0, 0, 0, 0) + IntArray(15)
        assertFailsWith<SnowedDecodeException> { decode(*bytes) }
    }
    @Test fun objectSentinelInValuePosition() {
        // Object with key "x" followed by Sentinel in value position → empty object
        val result = decode(0x80, 0x01, 'x'.code, 0xFF)
        assertEquals(SnowedValue.Object(emptyMap()), result)
    }
}

// ── Typed serialization ───────────────────────────────────────────────────────

@Serializable
data class Point(val x: Long, val y: Long)

@Serializable
data class Outer(val name: String, val inner: Point, val count: ULong)

@Serializable
data class MaybeVal(val value: Long? = null)

@Serializable
enum class Color { Red, Green, Blue }

@Serializable
sealed class Shape {
    @Serializable data class Circle(val radius: Double) : Shape()
    @Serializable data class Rect(val width: Double, val height: Double) : Shape()
}

class TypedSerializationTest {

    @Test fun primitiveBoolean()  { assertEquals(true,  roundTrip<Boolean>(true)) }
    @Test fun primitiveByte()     { assertEquals(42.toByte(), roundTrip<Byte>(42)) }
    @Test fun primitiveShort()    { assertEquals(1000.toShort(), roundTrip<Short>(1000)) }
    @Test fun primitiveInt()      { assertEquals(-100_000, roundTrip<Int>(-100_000)) }
    @Test fun primitiveLong()     { assertEquals(Long.MIN_VALUE, roundTrip(Long.MIN_VALUE)) }
    @Test fun primitiveFloat()    { assertEquals(1.5f, roundTrip(1.5f)) }
    @Test fun primitiveDouble()   { assertEquals(3.14, roundTrip(3.14)) }
    @Test fun primitiveString()   { assertEquals("hello", roundTrip("hello")) }
    @Test fun primitiveChar()     { assertEquals('Z', roundTrip('Z')) }

    @Test fun optionNone()     { assertNull(roundTrip<Long?>(null)) }
    @Test fun optionSome()     { assertEquals(42L, roundTrip<Long?>(42L)) }

    @Test fun enumAllValues() {
        for (c in Color.entries) assertEquals(c, roundTrip(c))
    }

    @Test fun simpleStruct() {
        val p = Point(7, -3)
        assertEquals(p, roundTrip(p))
    }

    @Test fun nestedStruct() {
        val v = Outer("test", Point(1, 2), 99uL)
        assertEquals(v, roundTrip(v))
    }

    @Test fun optionFieldNone() = assertEquals(MaybeVal(null), roundTrip(MaybeVal(null)))
    @Test fun optionFieldSome() = assertEquals(MaybeVal(7L),   roundTrip(MaybeVal(7L)))

    @Test fun listOfLong() {
        val v = listOf(1L, 2L, 3L, -99L)
        assertEquals(v, roundTrip(v))
    }

    @Test fun nestedList() {
        val v = listOf(listOf(1L, 2L), listOf(3L, 4L))
        assertEquals(v, roundTrip(v))
    }

    @Test fun mapStringToLong() {
        val v = mapOf("alpha" to 1L, "beta" to -2L)
        assertEquals(v, roundTrip(v))
    }
}

// ── Special types ─────────────────────────────────────────────────────────────

class SpecialTypeTest {

    @Test fun dateTimeEpoch() {
        val dt = SnowedDateTime(0.0)
        assertEquals(dt.millisSinceEpoch, roundTrip(dt).millisSinceEpoch)
    }

    @Test fun dateTimePositive() {
        val dt = SnowedDateTime(1_700_000_000_000.0)
        assertEquals(dt.millisSinceEpoch, roundTrip(dt).millisSinceEpoch)
    }

    @Test fun decimalRoundTrip() {
        val d = SnowedDecimal("3.14159")
        assertEquals(d.value, roundTrip(d).value)
    }

    @Test fun uuidRoundTrip() {
        val bytes = byteArrayOf(
            0x6b, 0xa7.toByte(), 0xb8.toByte(), 0x10,
            0x9d.toByte(), 0xad.toByte(), 0x11, 0xd1.toByte(),
            0x80.toByte(), 0xb4.toByte(), 0x00, 0xc0.toByte(),
            0x4f, 0xd4.toByte(), 0x30, 0xc8.toByte(),
        )
        val u = SnowedUuid(bytes)
        assertContentEquals(u.bytes, roundTrip(u).bytes)
    }
}

// ── Configuration tests ───────────────────────────────────────────────────────

@Serializable
data class Named(val firstName: String, val lastName: String)

class ConfigurationTest {

    @Test fun camelCaseNamingStrategy() {
        val snowed = SnowedFormat { namingStrategy = SnowedNamingStrategy.CamelCase }
        val v = Named("Jane", "Doe")
        val bytes = snowed.encodeToByteArray(v)
        // Decode as untyped to inspect keys
        val raw = snowed.decodeFromByteArray<SnowedValue>(bytes) as SnowedValue.Object
        assertTrue("firstName" in raw.entries)
        assertTrue("lastName" in raw.entries)
        assertEquals(v, snowed.decodeFromByteArray<Named>(bytes))
    }

    @Test fun snakeCaseNamingStrategy() {
        val snowed = SnowedFormat { namingStrategy = SnowedNamingStrategy.SnakeCase }
        val v = Named("Jane", "Doe")
        val bytes = snowed.encodeToByteArray(v)
        val raw = snowed.decodeFromByteArray<SnowedValue>(bytes) as SnowedValue.Object
        assertTrue("first_name" in raw.entries)
        assertTrue("last_name" in raw.entries)
        assertEquals(v, snowed.decodeFromByteArray<Named>(bytes))
    }

    @Test fun unknownKeysIgnore() {
        // Build bytes with an extra key the target class doesn't have
        val writer = SnowedWriter()
        writer.writeObjectBegin()
        writer.writeKey("firstName"); writer.writeString("Jane")
        writer.writeKey("lastName");  writer.writeString("Doe")
        writer.writeKey("age");       writer.writeInt64(30)
        writer.writeObjectEnd()
        val snowed = SnowedFormat { unknownKeys = UnknownKeyHandling.Ignore }
        val result = snowed.decodeFromByteArray<Named>(writer.toByteArray())
        assertEquals(Named("Jane", "Doe"), result)
    }

    @Test fun unknownKeysError() {
        val writer = SnowedWriter()
        writer.writeObjectBegin()
        writer.writeKey("firstName"); writer.writeString("Jane")
        writer.writeKey("lastName");  writer.writeString("Doe")
        writer.writeKey("age");       writer.writeInt64(30)
        writer.writeObjectEnd()
        val snowed = SnowedFormat { unknownKeys = UnknownKeyHandling.Error }
        assertFailsWith<SnowedDecodeException> {
            snowed.decodeFromByteArray<Named>(writer.toByteArray())
        }
    }

    @Test fun duplicateKeysLastWins() {
        val writer = SnowedWriter()
        writer.writeObjectBegin()
        writer.writeKey("firstName"); writer.writeString("Jane")
        writer.writeKey("firstName"); writer.writeString("Alice")
        writer.writeKey("lastName");  writer.writeString("Doe")
        writer.writeObjectEnd()
        val snowed = SnowedFormat { duplicateKeys = DuplicateKeyHandling.LastWins }
        val result = snowed.decodeFromByteArray<Named>(writer.toByteArray())
        assertEquals("Alice", result.firstName)
    }

    @Test fun duplicateKeysFirstWins() {
        val writer = SnowedWriter()
        writer.writeObjectBegin()
        writer.writeKey("firstName"); writer.writeString("Jane")
        writer.writeKey("firstName"); writer.writeString("Alice")
        writer.writeKey("lastName");  writer.writeString("Doe")
        writer.writeObjectEnd()
        val snowed = SnowedFormat { duplicateKeys = DuplicateKeyHandling.FirstWins }
        val result = snowed.decodeFromByteArray<Named>(writer.toByteArray())
        assertEquals("Jane", result.firstName)
    }

    @Test fun duplicateKeysError() {
        val writer = SnowedWriter()
        writer.writeObjectBegin()
        writer.writeKey("firstName"); writer.writeString("Jane")
        writer.writeKey("firstName"); writer.writeString("Alice")
        writer.writeKey("lastName");  writer.writeString("Doe")
        writer.writeObjectEnd()
        val snowed = SnowedFormat { duplicateKeys = DuplicateKeyHandling.Error }
        assertFailsWith<SnowedDecodeException> {
            snowed.decodeFromByteArray<Named>(writer.toByteArray())
        }
    }

    @Test fun encodeDefaultsFalse() {
        @Serializable data class WithDefault(val x: Long = 0L, val y: Long)
        val snowed = SnowedFormat { encodeDefaults = false }
        val bytes = snowed.encodeToByteArray(WithDefault(x = 0L, y = 5L))
        val raw = snowed.decodeFromByteArray<SnowedValue>(bytes) as SnowedValue.Object
        assertFalse("x" in raw.entries, "default value should be omitted")
        assertTrue("y" in raw.entries)
    }

    @Test fun encodeDefaultsTrue() {
        @Serializable data class WithDefault(val x: Long = 0L, val y: Long)
        val snowed = SnowedFormat { encodeDefaults = true }
        val bytes = snowed.encodeToByteArray(WithDefault(x = 0L, y = 5L))
        val raw = snowed.decodeFromByteArray<SnowedValue>(bytes) as SnowedValue.Object
        assertTrue("x" in raw.entries, "default value should be present")
    }

    @Test fun maxDepthExceeded() {
        // Build a deeply nested array that exceeds default maxDepth of 64
        fun nest(depth: Int): SnowedValue =
            if (depth == 0) SnowedValue.Null
            else SnowedValue.Array(listOf(nest(depth - 1)))
        val deep = nest(65)
        val writer = SnowedWriter()
        writer.writeValue(deep)
        val snowed = SnowedFormat { maxDepth = 64 }
        assertFailsWith<SnowedDecodeException> {
            snowed.decodeFromByteArray<SnowedValue>(writer.toByteArray())
        }
    }
}
