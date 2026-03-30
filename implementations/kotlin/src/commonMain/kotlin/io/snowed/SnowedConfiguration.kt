package io.snowed

import kotlinx.serialization.modules.EmptySerializersModule
import kotlinx.serialization.modules.SerializersModule

// ── Enums ─────────────────────────────────────────────────────────────────────

/** How to handle object keys in encoded data that have no matching property on the target type. */
enum class UnknownKeyHandling {
    /** Silently skip unrecognised keys and their values (default). */
    Ignore,
    /** Throw [SnowedDecodeException] when an unrecognised key is encountered. */
    Error,
}

/** How to handle duplicate keys within an encoded Object. */
enum class DuplicateKeyHandling {
    /** The first occurrence wins; subsequent duplicates are skipped. */
    FirstWins,
    /** The last occurrence wins; earlier values are overwritten (default). */
    LastWins,
    /** Throw [SnowedDecodeException] when a duplicate key is encountered. */
    Error,
}

/** How property names are mapped to Snowed object keys. */
enum class SnowedNamingStrategy {
    /** `PascalCase` → `camelCase`. */
    CamelCase,
    /** `camelCase` / `PascalCase` → `snake_case`. */
    SnakeCase,
}

// ── Configuration ─────────────────────────────────────────────────────────────

/**
 * Immutable configuration for [SnowedFormat]. Build with the [SnowedFormat]
 * constructor lambda:
 *
 * ```kotlin
 * val snowed = SnowedFormat {
 *     namingStrategy = SnowedNamingStrategy.CamelCase
 *     unknownKeys    = UnknownKeyHandling.Ignore
 * }
 * ```
 */
class SnowedConfiguration internal constructor(builder: Builder) {
    val namingStrategy: SnowedNamingStrategy? = builder.namingStrategy
    val unknownKeys: UnknownKeyHandling = builder.unknownKeys
    val duplicateKeys: DuplicateKeyHandling = builder.duplicateKeys
    val encodeDefaults: Boolean = builder.encodeDefaults
    val coerceInputValues: Boolean = builder.coerceInputValues
    val maxDepth: Int = builder.maxDepth
    val classDiscriminator: String = builder.classDiscriminator
    val serializersModule: SerializersModule = builder.serializersModule

    class Builder {
        /** `null` = property name used as-is. */
        var namingStrategy: SnowedNamingStrategy? = null
        var unknownKeys: UnknownKeyHandling = UnknownKeyHandling.Ignore
        var duplicateKeys: DuplicateKeyHandling = DuplicateKeyHandling.LastWins
        /** When `false`, properties equal to their default value are omitted. */
        var encodeDefaults: Boolean = false
        /**
         * When `true`, a `null` value arriving for a non-nullable property
         * is coerced to the property's default value instead of throwing.
         */
        var coerceInputValues: Boolean = false
        var maxDepth: Int = 64
        /** Object key used for the class discriminator in polymorphic encoding. */
        var classDiscriminator: String = "type"
        var serializersModule: SerializersModule = EmptySerializersModule()
    }

    companion object {
        val Default = SnowedConfiguration(Builder())
    }
}

/** Converts a property name according to the given [SnowedNamingStrategy], or returns it unchanged. */
internal fun String.applyNamingStrategy(strategy: SnowedNamingStrategy?): String = when (strategy) {
    null -> this
    SnowedNamingStrategy.CamelCase -> if (isEmpty() || !first().isUpperCase()) this
        else first().lowercaseChar() + substring(1)
    SnowedNamingStrategy.SnakeCase -> fold(StringBuilder()) { sb, c ->
        if (c.isUpperCase() && sb.isNotEmpty()) sb.append('_').append(c.lowercaseChar())
        else sb.append(c.lowercaseChar())
    }.toString()
}
