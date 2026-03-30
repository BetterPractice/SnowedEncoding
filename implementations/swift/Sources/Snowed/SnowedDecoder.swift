import Foundation

// MARK: - Public API

/// Decodes `Codable` values from Snowed binary data.
///
/// Usage mirrors `JSONDecoder`:
/// ```swift
/// let decoder = SnowedDecoder()
/// let value = try decoder.decode(MyType.self, from: data)
/// ```
///
/// When decoding numeric types, sane cross-type conversions are attempted
/// (e.g. a Snowed Int64 can be decoded as `Int32` if the value fits).
/// `Date`, `Data`, and `Decimal` are always decoded from their native Snowed
/// types and are not subject to general numeric coercion.
public final class SnowedDecoder {

    /// Contextual user info passed to decoding containers.
    public var userInfo: [CodingUserInfoKey: Any] = [:]

    public init() {}

    /// Decodes a `Codable` value from Snowed binary data.
    public func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        var parser = _SnowedParser(data: data)
        let value = try parser.parseRoot()
        let dec = _SnowedDecoder(value: value, codingPath: [], userInfo: userInfo)
        return try T(from: dec)
    }

    /// Parses Snowed binary data into a ``SnowedValue`` without going through
    /// the `Codable` system.
    public func decode(from data: Data) throws -> SnowedValue {
        var parser = _SnowedParser(data: data)
        return try parser.parseRoot()
    }
}

// MARK: - Parser

private struct _SnowedParser {
    private let bytes: [UInt8]
    private var offset: Int = 0

    init(data: Data) { self.bytes = Array(data) }

    mutating func parseRoot() throws -> SnowedValue {
        let value = try parseNext()
        return value
    }

    // MARK: Read primitives

    mutating func readByte() throws -> UInt8 {
        guard offset < bytes.count else { throw unexpectedEnd() }
        defer { offset += 1 }
        return bytes[offset]
    }

    mutating func readLE64() throws -> UInt64 {
        guard offset + 8 <= bytes.count else { throw unexpectedEnd() }
        let b = bytes
        let i = offset
        offset += 8
        return UInt64(b[i])
             | UInt64(b[i+1]) << 8
             | UInt64(b[i+2]) << 16
             | UInt64(b[i+3]) << 24
             | UInt64(b[i+4]) << 32
             | UInt64(b[i+5]) << 40
             | UInt64(b[i+6]) << 48
             | UInt64(b[i+7]) << 56
    }

    mutating func readBytes(_ count: Int) throws -> Data {
        guard count >= 0, offset + count <= bytes.count else { throw unexpectedEnd() }
        defer { offset += count }
        return Data(bytes[offset ..< offset + count])
    }

    // MARK: Value parsing

    mutating func parseNext() throws -> SnowedValue {
        let typeByte = try readByte()
        return try parseValue(typeByte: typeByte)
    }

    mutating func parseValue(typeByte: UInt8) throws -> SnowedValue {
        switch typeByte {

        // ── Null family (0x00–0x03) ──────────────────────────────────────────
        case 0x00, 0x01:
            return .null

        case 0x02: return .bool(false)
        case 0x03: return .bool(true)

        // ── Int family (0x04–0x07) ───────────────────────────────────────────
        case 0x04:
            return .int64(Int64(bitPattern: try readLE64()))
        case 0x05:
            return .uint64(try readLE64())
        case 0x06, 0x07:                                    // unknown Int subtypes
            return .int64(Int64(bitPattern: try readLE64()))

        // ── Float family (0x08–0x0F) ─────────────────────────────────────────
        case 0x08:
            return .float64(Double(bitPattern: try readLE64()))
        case 0x09:
            let ms = Double(bitPattern: try readLE64())
            return .dateTime(Date(timeIntervalSince1970: ms / 1000.0))
        case 0x0A ... 0x0F:                                 // unknown Float subtypes
            return .float64(Double(bitPattern: try readLE64()))

        // ── Binary family (0x10–0x3F) ────────────────────────────────────────
        case 0x10:
            return .binary(try readLengthPrefixed())
        case 0x11:
            let data = try readLengthPrefixed()
            guard let s = String(data: data, encoding: .utf8) else {
                throw corrupted("Invalid UTF-8 in String value")
            }
            return .string(s)
        case 0x12:
            let data = try readLengthPrefixed()
            guard let s = String(data: data, encoding: .utf8),
                  let d = Decimal(string: s) else {
                throw corrupted("Invalid Decimal string")
            }
            return .decimal(d)
        case 0x13:
            let data = try readLengthPrefixed()
            guard data.count == 16 else {
                throw corrupted("UUID payload must be exactly 16 bytes, got \(data.count)")
            }
            let uuid = data.withUnsafeBytes { ptr -> UUID in
                var bytes = ptr.load(as: uuid_t.self)
                return UUID(uuid: bytes)
            }
            return .uuid(uuid)
        case 0x14 ... 0x3F:                                 // unknown Binary subtypes
            return .binary(try readLengthPrefixed())

        // ── Array family (0x40–0x7F) ─────────────────────────────────────────
        case 0x40 ... 0x7F:
            return .array(try readArrayElements())

        // ── Object family (0x80–0xFE) ────────────────────────────────────────
        case 0x80 ... 0xFE:
            return .object(try readObjectPairs())

        // ── Sentinel (0xFF) ──────────────────────────────────────────────────
        case 0xFF:
            throw corrupted("Unexpected Sentinel (0xFF) in value position")

        default:
            // UInt8 is fully covered above (0x00–0xFF), but the compiler requires
            // a default clause. This path is unreachable.
            throw corrupted("Unhandled type byte 0x\(String(typeByte, radix: 16, uppercase: true))")
        }
    }

    private mutating func readLengthPrefixed() throws -> Data {
        let len = try readLE64()
        guard len <= Int.max else { throw corrupted("Binary/String length exceeds platform maximum") }
        return try readBytes(Int(len))
    }

    private mutating func readArrayElements() throws -> [SnowedValue] {
        var elements: [SnowedValue] = []
        while true {
            let next = try readByte()
            if next == 0xFF { break }
            elements.append(try parseValue(typeByte: next))
        }
        return elements
    }

    private mutating func readObjectPairs() throws -> [String: SnowedValue] {
        var dict: [String: SnowedValue] = [:]
        while true {
            let keyLen = try readByte()
            if keyLen == 0x00 { break }                     // zero-length key terminates
            let keyData = try readBytes(Int(keyLen))
            guard let key = String(data: keyData, encoding: .utf8) else {
                throw corrupted("Invalid UTF-8 in object key")
            }
            let valueTypeByte = try readByte()
            if valueTypeByte == 0xFF { break }              // Sentinel in value position terminates
            dict[key] = try parseValue(typeByte: valueTypeByte)
        }
        return dict
    }

    // MARK: Error helpers

    private func unexpectedEnd() -> DecodingError {
        corrupted("Unexpected end of Snowed data")
    }

    private func corrupted(_ message: String) -> DecodingError {
        DecodingError.dataCorrupted(
            DecodingError.Context(codingPath: [], debugDescription: message)
        )
    }
}

// MARK: - Internal decoder

final class _SnowedDecoder: Decoder {
    let value: SnowedValue
    var codingPath: [CodingKey]
    var userInfo: [CodingUserInfoKey: Any]

    init(value: SnowedValue, codingPath: [CodingKey], userInfo: [CodingUserInfoKey: Any]) {
        self.value = value
        self.codingPath = codingPath
        self.userInfo = userInfo
    }

    func container<Key: CodingKey>(keyedBy type: Key.Type) throws -> KeyedDecodingContainer<Key> {
        guard case .object(let dict) = value else {
            throw typeMismatch([String: Any].self, "Expected object")
        }
        return KeyedDecodingContainer(
            _SnowedKeyedDecodingContainer(dict: dict, codingPath: codingPath, userInfo: userInfo)
        )
    }

    func unkeyedContainer() throws -> UnkeyedDecodingContainer {
        guard case .array(let elements) = value else {
            throw typeMismatch([Any].self, "Expected array")
        }
        return _SnowedUnkeyedDecodingContainer(elements: elements, codingPath: codingPath, userInfo: userInfo)
    }

    func singleValueContainer() throws -> SingleValueDecodingContainer {
        _SnowedSingleValueDecodingContainer(value: value, codingPath: codingPath, userInfo: userInfo)
    }

    private func typeMismatch(_ type: Any.Type, _ message: String) -> DecodingError {
        DecodingError.typeMismatch(type, DecodingError.Context(codingPath: codingPath, debugDescription: message))
    }
}

// MARK: - Numeric coercion helpers

private func coerceToSignedInt<T: FixedWidthInteger & SignedInteger>(
    _ value: SnowedValue,
    as type: T.Type,
    codingPath: [CodingKey]
) throws -> T {
    func overflow(_ raw: Any) -> DecodingError {
        DecodingError.dataCorrupted(DecodingError.Context(
            codingPath: codingPath,
            debugDescription: "Value \(raw) overflows \(T.self)"
        ))
    }
    switch value {
    case .int64(let i):
        guard let r = T(exactly: i) else { throw overflow(i) }
        return r
    case .uint64(let u):
        guard let r = T(exactly: u) else { throw overflow(u) }
        return r
    case .float64(let f):
        guard !f.isNaN, !f.isInfinite, f == f.rounded(.towardZero),
              f >= Double(T.min), f <= Double(T.max),
              let r = T(exactly: Int64(f)) else {
            throw DecodingError.typeMismatch(T.self, DecodingError.Context(
                codingPath: codingPath,
                debugDescription: "Float64 \(f) cannot be losslessly converted to \(T.self)"
            ))
        }
        return r
    default:
        throw DecodingError.typeMismatch(T.self, DecodingError.Context(
            codingPath: codingPath, debugDescription: "Expected integer, got \(value)"
        ))
    }
}

private func coerceToUnsignedInt<T: FixedWidthInteger & UnsignedInteger>(
    _ value: SnowedValue,
    as type: T.Type,
    codingPath: [CodingKey]
) throws -> T {
    func overflow(_ raw: Any) -> DecodingError {
        DecodingError.dataCorrupted(DecodingError.Context(
            codingPath: codingPath,
            debugDescription: "Value \(raw) overflows \(T.self)"
        ))
    }
    switch value {
    case .uint64(let u):
        guard let r = T(exactly: u) else { throw overflow(u) }
        return r
    case .int64(let i):
        guard i >= 0, let r = T(exactly: i) else { throw overflow(i) }
        return r
    case .float64(let f):
        guard !f.isNaN, !f.isInfinite, f >= 0, f == f.rounded(.towardZero),
              f <= Double(T.max),
              let r = T(exactly: UInt64(f)) else {
            throw DecodingError.typeMismatch(T.self, DecodingError.Context(
                codingPath: codingPath,
                debugDescription: "Float64 \(f) cannot be losslessly converted to \(T.self)"
            ))
        }
        return r
    default:
        throw DecodingError.typeMismatch(T.self, DecodingError.Context(
            codingPath: codingPath, debugDescription: "Expected unsigned integer, got \(value)"
        ))
    }
}

private func coerceToDouble(_ value: SnowedValue, codingPath: [CodingKey]) throws -> Double {
    switch value {
    case .float64(let f): return f
    case .int64(let i):   return Double(i)
    case .uint64(let u):  return Double(u)
    default:
        throw DecodingError.typeMismatch(Double.self, DecodingError.Context(
            codingPath: codingPath, debugDescription: "Expected Float64, got \(value)"
        ))
    }
}

// MARK: - Keyed decoding container

private struct _SnowedKeyedDecodingContainer<Key: CodingKey>: KeyedDecodingContainerProtocol {
    let dict: [String: SnowedValue]
    var codingPath: [CodingKey]
    let userInfo: [CodingUserInfoKey: Any]
    var allKeys: [Key] { dict.keys.compactMap { Key(stringValue: $0) } }

    func contains(_ key: Key) -> Bool { dict[key.stringValue] != nil }

    private func require(_ key: Key) throws -> SnowedValue {
        guard let v = dict[key.stringValue] else {
            throw DecodingError.keyNotFound(key, DecodingError.Context(
                codingPath: codingPath, debugDescription: "Key '\(key.stringValue)' not found"
            ))
        }
        return v
    }

    private func path(_ key: Key) -> [CodingKey] { codingPath + [key] }

    func decodeNil(forKey key: Key) throws -> Bool {
        guard let v = dict[key.stringValue] else { return true }
        return v == .null
    }

    func decode(_ type: Bool.Type,   forKey key: Key) throws -> Bool {
        guard case .bool(let b) = try require(key) else {
            throw DecodingError.typeMismatch(Bool.self, DecodingError.Context(
                codingPath: path(key), debugDescription: "Expected Bool"
            ))
        }
        return b
    }

    func decode(_ type: Int.Type,    forKey key: Key) throws -> Int    { try coerceToSignedInt(try require(key), as: Int.self,    codingPath: path(key)) }
    func decode(_ type: Int8.Type,   forKey key: Key) throws -> Int8   { try coerceToSignedInt(try require(key), as: Int8.self,   codingPath: path(key)) }
    func decode(_ type: Int16.Type,  forKey key: Key) throws -> Int16  { try coerceToSignedInt(try require(key), as: Int16.self,  codingPath: path(key)) }
    func decode(_ type: Int32.Type,  forKey key: Key) throws -> Int32  { try coerceToSignedInt(try require(key), as: Int32.self,  codingPath: path(key)) }
    func decode(_ type: Int64.Type,  forKey key: Key) throws -> Int64  { try coerceToSignedInt(try require(key), as: Int64.self,  codingPath: path(key)) }
    func decode(_ type: UInt.Type,   forKey key: Key) throws -> UInt   { try coerceToUnsignedInt(try require(key), as: UInt.self,   codingPath: path(key)) }
    func decode(_ type: UInt8.Type,  forKey key: Key) throws -> UInt8  { try coerceToUnsignedInt(try require(key), as: UInt8.self,  codingPath: path(key)) }
    func decode(_ type: UInt16.Type, forKey key: Key) throws -> UInt16 { try coerceToUnsignedInt(try require(key), as: UInt16.self, codingPath: path(key)) }
    func decode(_ type: UInt32.Type, forKey key: Key) throws -> UInt32 { try coerceToUnsignedInt(try require(key), as: UInt32.self, codingPath: path(key)) }
    func decode(_ type: UInt64.Type, forKey key: Key) throws -> UInt64 { try coerceToUnsignedInt(try require(key), as: UInt64.self, codingPath: path(key)) }

    func decode(_ type: Float.Type,  forKey key: Key) throws -> Float  { Float(try coerceToDouble(try require(key), codingPath: path(key))) }
    func decode(_ type: Double.Type, forKey key: Key) throws -> Double { try coerceToDouble(try require(key), codingPath: path(key)) }

    func decode(_ type: String.Type, forKey key: Key) throws -> String {
        guard case .string(let s) = try require(key) else {
            throw DecodingError.typeMismatch(String.self, DecodingError.Context(
                codingPath: path(key), debugDescription: "Expected String"
            ))
        }
        return s
    }

    func decode<T: Decodable>(_ type: T.Type, forKey key: Key) throws -> T {
        let val = try require(key)
        return try decodeTyped(val, as: type, codingPath: path(key), userInfo: userInfo)
    }

    func nestedContainer<NK: CodingKey>(keyedBy type: NK.Type, forKey key: Key) throws -> KeyedDecodingContainer<NK> {
        guard case .object(let d) = try require(key) else {
            throw DecodingError.typeMismatch([String: Any].self, DecodingError.Context(
                codingPath: path(key), debugDescription: "Expected object"
            ))
        }
        return KeyedDecodingContainer(_SnowedKeyedDecodingContainer<NK>(dict: d, codingPath: path(key), userInfo: userInfo))
    }

    func nestedUnkeyedContainer(forKey key: Key) throws -> UnkeyedDecodingContainer {
        guard case .array(let elems) = try require(key) else {
            throw DecodingError.typeMismatch([Any].self, DecodingError.Context(
                codingPath: path(key), debugDescription: "Expected array"
            ))
        }
        return _SnowedUnkeyedDecodingContainer(elements: elems, codingPath: path(key), userInfo: userInfo)
    }

    func superDecoder() throws -> Decoder {
        let val = dict["super"] ?? .null
        return _SnowedDecoder(value: val, codingPath: codingPath + [_SnowedKey(stringValue: "super")], userInfo: userInfo)
    }

    func superDecoder(forKey key: Key) throws -> Decoder {
        let val = dict[key.stringValue] ?? .null
        return _SnowedDecoder(value: val, codingPath: path(key), userInfo: userInfo)
    }
}

// MARK: - Unkeyed decoding container

private final class _SnowedUnkeyedDecodingContainer: UnkeyedDecodingContainer {
    let elements: [SnowedValue]
    var codingPath: [CodingKey]
    let userInfo: [CodingUserInfoKey: Any]
    var currentIndex: Int = 0
    var count: Int?        { elements.count }
    var isAtEnd: Bool      { currentIndex >= elements.count }

    init(elements: [SnowedValue], codingPath: [CodingKey], userInfo: [CodingUserInfoKey: Any]) {
        self.elements = elements
        self.codingPath = codingPath
        self.userInfo = userInfo
    }

    private func nextValue() throws -> SnowedValue {
        guard !isAtEnd else {
            throw DecodingError.valueNotFound(SnowedValue.self, DecodingError.Context(
                codingPath: codingPath + [_SnowedKey(intValue: currentIndex)],
                debugDescription: "Unkeyed container is at end"
            ))
        }
        defer { currentIndex += 1 }
        return elements[currentIndex]
    }

    private func currentPath() -> [CodingKey] { codingPath + [_SnowedKey(intValue: currentIndex)] }

    func decodeNil() throws -> Bool {
        guard !isAtEnd, case .null = elements[currentIndex] else { return false }
        currentIndex += 1
        return true
    }

    func decode(_ t: Bool.Type)   throws -> Bool   {
        guard case .bool(let b) = try nextValue() else {
            throw DecodingError.typeMismatch(Bool.self, DecodingError.Context(codingPath: currentPath(), debugDescription: "Expected Bool"))
        }
        return b
    }
    func decode(_ t: Int.Type)    throws -> Int    { try coerceToSignedInt(try nextValue(), as: Int.self,    codingPath: currentPath()) }
    func decode(_ t: Int8.Type)   throws -> Int8   { try coerceToSignedInt(try nextValue(), as: Int8.self,   codingPath: currentPath()) }
    func decode(_ t: Int16.Type)  throws -> Int16  { try coerceToSignedInt(try nextValue(), as: Int16.self,  codingPath: currentPath()) }
    func decode(_ t: Int32.Type)  throws -> Int32  { try coerceToSignedInt(try nextValue(), as: Int32.self,  codingPath: currentPath()) }
    func decode(_ t: Int64.Type)  throws -> Int64  { try coerceToSignedInt(try nextValue(), as: Int64.self,  codingPath: currentPath()) }
    func decode(_ t: UInt.Type)   throws -> UInt   { try coerceToUnsignedInt(try nextValue(), as: UInt.self,   codingPath: currentPath()) }
    func decode(_ t: UInt8.Type)  throws -> UInt8  { try coerceToUnsignedInt(try nextValue(), as: UInt8.self,  codingPath: currentPath()) }
    func decode(_ t: UInt16.Type) throws -> UInt16 { try coerceToUnsignedInt(try nextValue(), as: UInt16.self, codingPath: currentPath()) }
    func decode(_ t: UInt32.Type) throws -> UInt32 { try coerceToUnsignedInt(try nextValue(), as: UInt32.self, codingPath: currentPath()) }
    func decode(_ t: UInt64.Type) throws -> UInt64 { try coerceToUnsignedInt(try nextValue(), as: UInt64.self, codingPath: currentPath()) }
    func decode(_ t: Float.Type)  throws -> Float  { Float(try coerceToDouble(try nextValue(), codingPath: currentPath())) }
    func decode(_ t: Double.Type) throws -> Double { try coerceToDouble(try nextValue(), codingPath: currentPath()) }
    func decode(_ t: String.Type) throws -> String {
        guard case .string(let s) = try nextValue() else {
            throw DecodingError.typeMismatch(String.self, DecodingError.Context(codingPath: currentPath(), debugDescription: "Expected String"))
        }
        return s
    }
    func decode<T: Decodable>(_ type: T.Type) throws -> T {
        let val = try nextValue()
        return try decodeTyped(val, as: type, codingPath: currentPath(), userInfo: userInfo)
    }

    func nestedContainer<NK: CodingKey>(keyedBy type: NK.Type) throws -> KeyedDecodingContainer<NK> {
        guard case .object(let d) = try nextValue() else {
            throw DecodingError.typeMismatch([String: Any].self, DecodingError.Context(codingPath: currentPath(), debugDescription: "Expected object"))
        }
        return KeyedDecodingContainer(_SnowedKeyedDecodingContainer<NK>(dict: d, codingPath: currentPath(), userInfo: userInfo))
    }

    func nestedUnkeyedContainer() throws -> UnkeyedDecodingContainer {
        guard case .array(let elems) = try nextValue() else {
            throw DecodingError.typeMismatch([Any].self, DecodingError.Context(codingPath: currentPath(), debugDescription: "Expected array"))
        }
        return _SnowedUnkeyedDecodingContainer(elements: elems, codingPath: currentPath(), userInfo: userInfo)
    }

    func superDecoder() throws -> Decoder {
        _SnowedDecoder(value: try nextValue(), codingPath: currentPath(), userInfo: userInfo)
    }
}

// MARK: - Single-value decoding container

private struct _SnowedSingleValueDecodingContainer: SingleValueDecodingContainer {
    let value: SnowedValue
    var codingPath: [CodingKey]
    let userInfo: [CodingUserInfoKey: Any]

    func decodeNil() -> Bool { value == .null }

    func decode(_ t: Bool.Type)   throws -> Bool {
        guard case .bool(let b) = value else {
            throw DecodingError.typeMismatch(Bool.self, DecodingError.Context(codingPath: codingPath, debugDescription: "Expected Bool"))
        }
        return b
    }
    func decode(_ t: Int.Type)    throws -> Int    { try coerceToSignedInt(value,   as: Int.self,    codingPath: codingPath) }
    func decode(_ t: Int8.Type)   throws -> Int8   { try coerceToSignedInt(value,   as: Int8.self,   codingPath: codingPath) }
    func decode(_ t: Int16.Type)  throws -> Int16  { try coerceToSignedInt(value,   as: Int16.self,  codingPath: codingPath) }
    func decode(_ t: Int32.Type)  throws -> Int32  { try coerceToSignedInt(value,   as: Int32.self,  codingPath: codingPath) }
    func decode(_ t: Int64.Type)  throws -> Int64  { try coerceToSignedInt(value,   as: Int64.self,  codingPath: codingPath) }
    func decode(_ t: UInt.Type)   throws -> UInt   { try coerceToUnsignedInt(value, as: UInt.self,   codingPath: codingPath) }
    func decode(_ t: UInt8.Type)  throws -> UInt8  { try coerceToUnsignedInt(value, as: UInt8.self,  codingPath: codingPath) }
    func decode(_ t: UInt16.Type) throws -> UInt16 { try coerceToUnsignedInt(value, as: UInt16.self, codingPath: codingPath) }
    func decode(_ t: UInt32.Type) throws -> UInt32 { try coerceToUnsignedInt(value, as: UInt32.self, codingPath: codingPath) }
    func decode(_ t: UInt64.Type) throws -> UInt64 { try coerceToUnsignedInt(value, as: UInt64.self, codingPath: codingPath) }
    func decode(_ t: Float.Type)  throws -> Float  { Float(try coerceToDouble(value, codingPath: codingPath)) }
    func decode(_ t: Double.Type) throws -> Double { try coerceToDouble(value, codingPath: codingPath) }
    func decode(_ t: String.Type) throws -> String {
        guard case .string(let s) = value else {
            throw DecodingError.typeMismatch(String.self, DecodingError.Context(codingPath: codingPath, debugDescription: "Expected String"))
        }
        return s
    }
    func decode<T: Decodable>(_ type: T.Type) throws -> T {
        try decodeTyped(value, as: type, codingPath: codingPath, userInfo: userInfo)
    }
}

// MARK: - Typed decode helper (handles Date, Data, Decimal interception)

private func decodeTyped<T: Decodable>(
    _ value: SnowedValue,
    as type: T.Type,
    codingPath: [CodingKey],
    userInfo: [CodingUserInfoKey: Any]
) throws -> T {
    if type == Date.self {
        guard case .dateTime(let d) = value else {
            throw DecodingError.typeMismatch(Date.self, DecodingError.Context(
                codingPath: codingPath, debugDescription: "Expected DateTime (0x09)"
            ))
        }
        return d as! T
    }
    if type == Data.self {
        guard case .binary(let d) = value else {
            throw DecodingError.typeMismatch(Data.self, DecodingError.Context(
                codingPath: codingPath, debugDescription: "Expected Binary (0x10)"
            ))
        }
        return d as! T
    }
    if type == Decimal.self {
        guard case .decimal(let d) = value else {
            throw DecodingError.typeMismatch(Decimal.self, DecodingError.Context(
                codingPath: codingPath, debugDescription: "Expected Decimal (0x12)"
            ))
        }
        return d as! T
    }
    if type == UUID.self {
        guard case .uuid(let u) = value else {
            throw DecodingError.typeMismatch(UUID.self, DecodingError.Context(
                codingPath: codingPath, debugDescription: "Expected UUID (0x13)"
            ))
        }
        return u as! T
    }
    let dec = _SnowedDecoder(value: value, codingPath: codingPath, userInfo: userInfo)
    return try T(from: dec)
}
