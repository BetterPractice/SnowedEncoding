package io.snowed

/** Base class for all Snowed serialization exceptions. */
open class SnowedException(message: String, cause: Throwable? = null) : Exception(message, cause)

/** Thrown when encoding fails (e.g. a key exceeds 255 bytes). */
class SnowedEncodeException(message: String, cause: Throwable? = null) : SnowedException(message, cause)

/** Thrown when decoding fails (e.g. invalid UTF-8, unexpected EOF, depth exceeded). */
class SnowedDecodeException(message: String, cause: Throwable? = null) : SnowedException(message, cause)
