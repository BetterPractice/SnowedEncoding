package io.snowed

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.math.BigDecimal
import java.time.Instant
import java.util.UUID
import kotlinx.serialization.Serializable
import kotlin.test.*

private val snowed = SnowedFormat()

class JvmStreamingTest {

    @Test fun encodeDecodeViaStreams() {
        val p = Point(3, -7)
        val baos = ByteArrayOutputStream()
        snowed.encodeTo(baos, p)
        val result: Point = snowed.decodeFrom(ByteArrayInputStream(baos.toByteArray()))
        assertEquals(p, result)
    }
}

class UuidConversionTest {

    @Test fun javaUuidRoundTrip() {
        val uuid = UUID.fromString("550e8400-e29b-41d4-a716-446655440000")
        val snowed = uuid.toSnowedUuid()
        assertEquals(uuid, snowed.toUuid())
    }

    @Test fun uuidBytesAreRfc4122() {
        val uuid = UUID.fromString("550e8400-e29b-41d4-a716-446655440000")
        val bytes = uuid.toSnowedUuid().bytes
        // First 4 bytes = time_low: 55 0e 84 00
        assertEquals(0x55.toByte(), bytes[0])
        assertEquals(0x0e.toByte(), bytes[1])
        assertEquals(0x84.toByte(), bytes[2])
        assertEquals(0x00.toByte(), bytes[3])
    }

    @Test fun encodeDecodeUuidField() {
        @Serializable
        data class Doc(val id: SnowedUuid)
        val uuid = UUID.randomUUID()
        val doc = Doc(uuid.toSnowedUuid())
        val bytes = snowed.encodeToByteArray(doc)
        val back = snowed.decodeFromByteArray<Doc>(bytes)
        assertEquals(uuid, back.id.toUuid())
    }
}

class BigDecimalConversionTest {

    @Test fun zero()         = assertEquals(SnowedDecimal("0"),     BigDecimal.ZERO.toSnowedDecimal())
    @Test fun negativeZero() = assertEquals(SnowedDecimal("0"),     BigDecimal("-0").toSnowedDecimal())
    @Test fun integer()      = assertEquals(SnowedDecimal("42"),    BigDecimal("42").toSnowedDecimal())
    @Test fun negative()     = assertEquals(SnowedDecimal("-7"),    BigDecimal("-7").toSnowedDecimal())
    @Test fun fractional()   = assertEquals(SnowedDecimal("3.14"),  BigDecimal("3.14").toSnowedDecimal())
    @Test fun trailingZeros() = assertEquals(SnowedDecimal("3.1"),  BigDecimal("3.10").toSnowedDecimal())
    @Test fun allZeros()     = assertEquals(SnowedDecimal("3"),     BigDecimal("3.00").toSnowedDecimal())
    @Test fun small()        = assertEquals(SnowedDecimal("0.001"), BigDecimal("0.001").toSnowedDecimal())

    @Test fun toBigDecimalRoundTrip() {
        val orig = BigDecimal("2.71828")
        assertEquals(orig, orig.toSnowedDecimal().toBigDecimal())
    }
}

class InstantConversionTest {

    @Test fun epochRoundTrip() {
        val instant = Instant.EPOCH
        assertEquals(instant, instant.toSnowedDateTime().toInstant())
    }

    @Test fun modernInstantRoundTrip() {
        val instant = Instant.ofEpochMilli(1_700_000_000_000L)
        assertEquals(instant, instant.toSnowedDateTime().toInstant())
    }

    @Test fun preEpochRoundTrip() {
        val instant = Instant.ofEpochMilli(-1_000_000L)
        assertEquals(instant, instant.toSnowedDateTime().toInstant())
    }

    @Test fun subMillisecondPrecisionLost() {
        // Instants with nanosecond precision lose sub-ms on round-trip
        val withNanos  = Instant.ofEpochSecond(0, 999_999)  // 999 microseconds
        val roundTripped = withNanos.toSnowedDateTime().toInstant()
        assertNotEquals(withNanos, roundTripped)
        assertEquals(Instant.EPOCH, roundTripped) // truncated to 0 ms
    }
}
