package io.snowed

import kotlinx.serialization.BinaryFormat
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.SerializationStrategy
import kotlinx.serialization.serializer

/**
 * The Snowed binary format entry point.
 *
 * Create an instance with optional configuration:
 *
 * ```kotlin
 * val snowed = SnowedFormat()  // defaults
 *
 * val snowed = SnowedFormat {
 *     namingStrategy = SnowedNamingStrategy.CamelCase
 *     unknownKeys    = UnknownKeyHandling.Ignore
 *     encodeDefaults = true
 * }
 * ```
 *
 * Then encode and decode `@Serializable` types:
 *
 * ```kotlin
 * val bytes: ByteArray = snowed.encodeToByteArray(value)
 * val value: MyType    = snowed.decodeFromByteArray(bytes)
 * ```
 */
class SnowedFormat(
    val configuration: SnowedConfiguration = SnowedConfiguration.Default,
) : BinaryFormat {

    constructor(block: SnowedConfiguration.Builder.() -> Unit)
        : this(SnowedConfiguration(SnowedConfiguration.Builder().also(block)))

    override val serializersModule get() = configuration.serializersModule

    // ── BinaryFormat ──────────────────────────────────────────────────────────

    override fun <T> encodeToByteArray(serializer: SerializationStrategy<T>, value: T): ByteArray {
        val writer = SnowedWriter()
        val encoder = SnowedEncoder(writer, configuration, serializersModule)
        serializer.serialize(encoder, value)
        return writer.toByteArray()
    }

    override fun <T> decodeFromByteArray(deserializer: DeserializationStrategy<T>, bytes: ByteArray): T {
        val reader = SnowedReader(bytes, configuration.maxDepth)
        val decoder = SnowedDecoder(reader, configuration, serializersModule)
        val value = deserializer.deserialize(decoder)
        if (reader.remaining > 0) throw SnowedDecodeException("Trailing bytes after decoded value")
        return value
    }

    // ── Reified convenience extensions ────────────────────────────────────────
    // (Defined as top-level extensions below to get reified type params.)
}

/** Encodes [value] to Snowed bytes using the reified serializer. */
inline fun <reified T> SnowedFormat.encodeToByteArray(value: T): ByteArray =
    encodeToByteArray(serializersModule.serializer(), value)

/** Decodes a value of type [T] from Snowed [bytes] using the reified serializer. */
inline fun <reified T> SnowedFormat.decodeFromByteArray(bytes: ByteArray): T =
    decodeFromByteArray(serializersModule.serializer(), bytes)
