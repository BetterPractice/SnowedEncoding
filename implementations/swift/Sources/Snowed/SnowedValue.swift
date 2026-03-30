import Foundation

/// A type-erased representation of any value in the Snowed binary format.
///
/// Use this type when reading documents without a known schema, or when building
/// Snowed values dynamically. For typed encoding and decoding of Swift structs,
/// use ``SnowedEncoder`` and ``SnowedDecoder`` with `Codable` types instead.
///
/// - Note: This enum is intentionally not `@frozen`. Future versions of the Snowed
///   specification may introduce new type cases. Use `@unknown default` in exhaustive
///   switch statements to remain forward-compatible.
public enum SnowedValue {
    case null
    case bool(Bool)
    case int64(Int64)
    case uint64(UInt64)
    case float64(Double)
    case dateTime(Date)
    case binary(Data)
    case string(String)
    case decimal(Decimal)
    case uuid(UUID)
    case array([SnowedValue])
    case object([String: SnowedValue])
}

// MARK: - Equatable

extension SnowedValue: Equatable {
    public static func == (lhs: SnowedValue, rhs: SnowedValue) -> Bool {
        switch (lhs, rhs) {
        case (.null, .null):                       return true
        case let (.bool(l),    .bool(r)):          return l == r
        case let (.int64(l),   .int64(r)):         return l == r
        case let (.uint64(l),  .uint64(r)):        return l == r
        case let (.float64(l), .float64(r)):       return l == r
        case let (.dateTime(l),.dateTime(r)):      return l == r
        case let (.binary(l),  .binary(r)):        return l == r
        case let (.string(l),  .string(r)):        return l == r
        case let (.decimal(l), .decimal(r)):       return l == r
        case let (.uuid(l),    .uuid(r)):          return l == r
        case let (.array(l),   .array(r)):         return l == r
        case let (.object(l),  .object(r)):        return l == r
        default:                                    return false
        }
    }
}

// MARK: - Internal helpers

/// Returns the canonical Snowed string encoding of a `Decimal`.
///
/// Rules (per spec §5.9):
/// - Invariant `.` decimal separator.
/// - No exponent notation.
/// - No trailing zeros after the decimal point.
/// - No leading zeros before the decimal point (except `0.something`).
/// - Zero and negative zero both produce `"0"`.
internal func canonicalDecimalString(_ value: Decimal) -> String {
    if value.isNaN || value.isZero { return "0" }

    // Decimal.description uses NSDecimalString internally, which produces plain
    // decimal notation (no exponents) with the invariant locale.
    var str = value.description

    if str == "-0" { return "0" }

    if str.contains(".") {
        while str.hasSuffix("0") { str.removeLast() }
        if str.hasSuffix(".")  { str.removeLast() }
    }

    return str
}
