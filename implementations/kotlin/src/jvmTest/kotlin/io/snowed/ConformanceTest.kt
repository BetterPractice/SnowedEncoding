package io.snowed

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import org.junit.jupiter.api.DynamicContainer
import org.junit.jupiter.api.DynamicNode
import org.junit.jupiter.api.DynamicTest
import org.junit.jupiter.api.TestFactory
import java.io.File
import java.util.Base64
import kotlin.test.fail

/**
 * Runs the cross-implementation conformance vectors in `tests/conformance/vectors/`
 * (see `tests/conformance/README.md`). Each case becomes a dynamic test named
 * `<suite>/<case>`.
 */
class ConformanceTest {

    private val snowed = SnowedFormat()

    @TestFactory
    fun conformance(): List<DynamicNode> {
        val dir = locateVectorsDir()
        val files = dir.listFiles { f -> f.isFile && f.name.endsWith(".json") }
            ?.sortedBy { it.name }
            .orEmpty()
        if (files.isEmpty()) fail("No .json vector files found in ${dir.absolutePath}")

        return files.map { file ->
            val root = Json.parseToJsonElement(file.readText()) as? JsonObject
                ?: fail("${file.name}: top-level JSON is not an object")
            val suite = root.str("suite") ?: fail("${file.name}: missing \"suite\"")
            val cases = root["cases"] as? JsonArray ?: fail("${file.name}: missing \"cases\" array")
            if (cases.isEmpty()) fail("${file.name}: \"cases\" is empty")

            DynamicContainer.dynamicContainer(suite, cases.map { el ->
                val case = el as? JsonObject ?: fail("$suite: case is not an object")
                val name = case.str("name") ?: fail("$suite: case without \"name\"")
                DynamicTest.dynamicTest("$suite/$name") { runCase(suite, name, case) }
            })
        }
    }

    // ── Case execution ────────────────────────────────────────────────────────

    private fun runCase(suite: String, name: String, case: JsonObject) {
        val id = "$suite/$name"
        val hex = case.str("hex") ?: fail("$id: missing \"hex\"")
        val bytes = parseHex(id, hex)

        val isError = (case["error"] as? JsonPrimitive)?.booleanOrNull == true
        if (isError) {
            val result = runCatching { decode(bytes) }
            val ex = result.exceptionOrNull()
            when {
                ex == null ->
                    fail("$id: expected SnowedDecodeException, but decoded to ${result.getOrNull()}")
                ex !is SnowedDecodeException ->
                    fail("$id: expected SnowedDecodeException, but got ${ex::class.qualifiedName}: ${ex.message}", ex)
            }
            return
        }

        val expected = case["decoded"] as? JsonObject ?: fail("$id: case has neither \"decoded\" nor \"error\": true")
        val canonical = (case["canonical"] as? JsonPrimitive)?.booleanOrNull ?: true

        // 1. Decode and compare.
        val decoded = try {
            decode(bytes)
        } catch (e: Exception) {
            fail("$id: decode failed: ${e::class.simpleName}: ${e.message}", e)
        }
        compare(expected, decoded, "$")?.let { fail("$id: decoded value mismatch: $it (actual: $decoded)") }

        // 2. Semantic round-trip.
        val reencoded = try {
            snowed.encodeToByteArray<SnowedValue>(decoded)
        } catch (e: Exception) {
            fail("$id: re-encode failed: ${e::class.simpleName}: ${e.message}", e)
        }
        val redecoded = try {
            decode(reencoded)
        } catch (e: Exception) {
            fail("$id: decode of re-encoded bytes [${toHex(reencoded)}] failed: ${e::class.simpleName}: ${e.message}", e)
        }
        compare(expected, redecoded, "$")?.let {
            fail("$id: round-trip mismatch: $it (re-encoded: [${toHex(reencoded)}])")
        }

        // 3. Byte-exact canonical re-encoding.
        if (canonical && !reencoded.contentEquals(bytes)) {
            fail("$id: canonical re-encoding mismatch\n  expected: [${toHex(bytes)}]\n  actual:   [${toHex(reencoded)}]")
        }
    }

    private fun decode(bytes: ByteArray): SnowedValue = snowed.decodeFromByteArray<SnowedValue>(bytes)

    // ── Comparison ────────────────────────────────────────────────────────────

    /** Returns a mismatch description, or null if [actual] matches [expected]. */
    private fun compare(expected: JsonObject, actual: SnowedValue, path: String): String? {
        val type = expected.str("type") ?: return "$path: vector has no \"type\""
        val value = expected["value"] ?: JsonNull

        fun mismatch(want: Any?, got: Any?) = "$path: expected $type $want, got $got"

        return when (type) {
            "null" -> if (actual is SnowedValue.Null) null else mismatch("null", actual)

            "bool" -> {
                val want = (value as? JsonPrimitive)?.booleanOrNull ?: return "$path: bad bool value $value"
                if (actual is SnowedValue.Bool && actual.value == want) null else mismatch(want, actual)
            }

            "int64" -> {
                val want = (value as? JsonPrimitive)?.content?.toLongOrNull()
                    ?: return "$path: bad int64 value $value"
                if (actual is SnowedValue.Int64 && actual.value == want) null else mismatch(want, actual)
            }

            "uint64" -> {
                val want = (value as? JsonPrimitive)?.content?.toULongOrNull()
                    ?: return "$path: bad uint64 value $value"
                if (actual is SnowedValue.UInt64 && actual.value == want) null else mismatch(want, actual)
            }

            "float64" -> {
                val want = parseDouble(value) ?: return "$path: bad float64 value $value"
                if (actual is SnowedValue.Float64 && doubleMatches(want, actual.value)) null
                else mismatch(want, actual)
            }

            "datetime" -> {
                val want = parseDouble(value) ?: return "$path: bad datetime value $value"
                if (actual is SnowedValue.DateTime && doubleMatches(want, actual.millisSinceEpoch)) null
                else mismatch(want, actual)
            }

            "binary" -> {
                val b64 = (value as? JsonPrimitive)?.takeIf { it.isString }?.content
                    ?: return "$path: bad binary value $value"
                val want = try { Base64.getDecoder().decode(b64) } catch (e: IllegalArgumentException) {
                    return "$path: bad base64 \"$b64\""
                }
                if (actual is SnowedValue.Binary && actual.bytes.contentEquals(want)) null
                else mismatch("[${toHex(want)}]", (actual as? SnowedValue.Binary)?.let { "Binary[${toHex(it.bytes)}]" } ?: actual)
            }

            "string" -> {
                val want = (value as? JsonPrimitive)?.takeIf { it.isString }?.content
                    ?: return "$path: bad string value $value"
                if (actual is SnowedValue.Str && actual.value == want) null else mismatch("\"$want\"", actual)
            }

            "decimal" -> {
                val want = (value as? JsonPrimitive)?.takeIf { it.isString }?.content
                    ?: return "$path: bad decimal value $value"
                if (actual is SnowedValue.Decimal && actual.value == want) null else mismatch("\"$want\"", actual)
            }

            "uuid" -> {
                val want = (value as? JsonPrimitive)?.takeIf { it.isString }?.content
                    ?: return "$path: bad uuid value $value"
                val got = (actual as? SnowedValue.Uuid)?.let { formatUuid(it.bytes) }
                if (got == want) null else mismatch(want, got ?: actual)
            }

            "array" -> {
                val want = value as? JsonArray ?: return "$path: bad array value $value"
                if (actual !is SnowedValue.Array) return mismatch("of ${want.size} elements", actual)
                if (actual.elements.size != want.size) {
                    return "$path: expected array of ${want.size} elements, got ${actual.elements.size}"
                }
                want.indices.firstNotNullOfOrNull { i ->
                    val w = want[i] as? JsonObject ?: return "$path[$i]: element is not a value object"
                    compare(w, actual.elements[i], "$path[$i]")
                }
            }

            "object" -> {
                val want = value as? JsonObject ?: return "$path: bad object value $value"
                if (actual !is SnowedValue.Object) return mismatch("with keys ${want.keys}", actual)
                if (actual.entries.keys != want.keys) {
                    return "$path: expected object keys ${want.keys}, got ${actual.entries.keys}"
                }
                want.entries.firstNotNullOfOrNull { (k, w) ->
                    val wo = w as? JsonObject ?: return "$path.$k: entry is not a value object"
                    compare(wo, actual.entries.getValue(k), "$path.\"$k\"")
                }
            }

            else -> "$path: unknown vector type \"$type\""
        }
    }

    /** Parses a float64/datetime JSON value from its literal text (no lossy intermediate). */
    private fun parseDouble(value: JsonElement): Double? {
        val p = value as? JsonPrimitive ?: return null
        if (p is JsonNull) return null
        return when (p.content) {
            "Infinity" -> Double.POSITIVE_INFINITY
            "-Infinity" -> Double.NEGATIVE_INFINITY
            "NaN" -> Double.NaN
            "-0" -> -0.0
            else -> if (p.isString) null else p.content.toDoubleOrNull()
        }
    }

    /** NaN matches any NaN; otherwise raw bits must match (so 0.0 and -0.0 differ). */
    private fun doubleMatches(want: Double, got: Double): Boolean =
        if (want.isNaN()) got.isNaN() else want.toRawBits() == got.toRawBits()

    // ── Helpers ───────────────────────────────────────────────────────────────

    private fun JsonObject.str(key: String): String? =
        (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.content

    private fun parseHex(id: String, hex: String): ByteArray {
        val tokens = hex.trim().split(Regex("\\s+")).filter { it.isNotEmpty() }
        return ByteArray(tokens.size) { i ->
            val t = tokens[i]
            if (t.length != 2) fail("$id: bad hex token \"$t\"")
            (t.toIntOrNull(16) ?: fail("$id: bad hex token \"$t\"")).toByte()
        }
    }

    private fun toHex(bytes: ByteArray): String =
        bytes.joinToString(" ") { "%02X".format(it.toInt() and 0xFF) }

    private fun formatUuid(bytes: ByteArray): String {
        val h = bytes.joinToString("") { "%02x".format(it.toInt() and 0xFF) }
        return "${h.substring(0, 8)}-${h.substring(8, 12)}-${h.substring(12, 16)}-${h.substring(16, 20)}-${h.substring(20)}"
    }

    private fun locateVectorsDir(): File {
        val start = File(System.getProperty("user.dir")).absoluteFile
        var dir: File? = start
        while (dir != null) {
            val candidate = File(dir, "tests/conformance/vectors")
            if (candidate.isDirectory) return candidate
            dir = dir.parentFile
        }
        fail("Conformance vectors directory tests/conformance/vectors not found above ${start.path}")
    }
}
