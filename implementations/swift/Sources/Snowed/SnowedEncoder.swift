import Foundation

// MARK: - Public API

/// Encodes `Codable` values into Snowed binary format.
///
/// Usage mirrors `JSONEncoder`:
/// ```swift
/// let encoder = SnowedEncoder()
/// let data = try encoder.encode(myValue)
/// ```
///
/// `Date`, `Data`, and `Decimal` values are intercepted and encoded as their
/// native Snowed types (DateTime 0x09, Binary 0x10, Decimal 0x12) regardless
/// of how the type's own `Encodable` conformance would otherwise encode them.
public final class SnowedEncoder {

    /// Contextual user info passed to encoding containers.
    public var userInfo: [CodingUserInfoKey: Any] = [:]

    public init() {}

    /// Encodes a `Codable` value into Snowed binary data.
    public func encode<T: Encodable>(_ value: T) throws -> Data {
        let enc = _SnowedEncoder(codingPath: [], userInfo: userInfo)
        try value.encode(to: enc)
        return Data(try serializeValue(enc.storage))
    }

    /// Encodes a ``SnowedValue`` directly into Snowed binary data,
    /// bypassing the `Codable` system.
    public func encode(_ value: SnowedValue) throws -> Data {
        Data(try serializeValue(value))
    }
}

// MARK: - Serialization

private func serializeValue(_ value: SnowedValue) throws -> [UInt8] {
    var out: [UInt8] = []
    try appendValue(value, to: &out, codingPath: [])
    return out
}

private func appendValue(_ value: SnowedValue, to out: inout [UInt8], codingPath: [CodingKey]) throws {
    switch value {
    case .null:
        out.append(0x00)

    case .bool(let b):
        out.append(b ? 0x03 : 0x02)

    case .int64(let i):
        out.append(0x04)
        appendLE64(UInt64(bitPattern: i), to: &out)

    case .uint64(let u):
        out.append(0x05)
        appendLE64(u, to: &out)

    case .float64(let f):
        out.append(0x08)
        appendLE64(f.bitPattern, to: &out)

    case .dateTime(let d):
        out.append(0x09)
        appendLE64((d.timeIntervalSince1970 * 1000.0).bitPattern, to: &out)

    case .binary(let data):
        out.append(0x10)
        appendLE64(UInt64(data.count), to: &out)
        out.append(contentsOf: data)

    case .string(let s):
        let utf8 = Array(s.utf8)
        out.append(0x11)
        appendLE64(UInt64(utf8.count), to: &out)
        out.append(contentsOf: utf8)

    case .decimal(let d):
        let utf8 = Array(canonicalDecimalString(d).utf8)
        out.append(0x12)
        appendLE64(UInt64(utf8.count), to: &out)
        out.append(contentsOf: utf8)

    case .uuid(let u):
        out.append(0x13)
        appendLE64(16, to: &out)
        withUnsafeBytes(of: u.uuid) { out.append(contentsOf: $0) }

    case .array(let elements):
        out.append(0x40)
        for element in elements {
            try appendValue(element, to: &out, codingPath: codingPath)
        }
        out.append(0xFF)

    case .object(let dict):
        out.append(0x80)
        for key in dict.keys.sorted() {         // sorted for deterministic output
            let keyUTF8 = Array(key.utf8)
            guard keyUTF8.count <= 255 else {
                throw EncodingError.invalidValue(
                    key,
                    EncodingError.Context(
                        codingPath: codingPath,
                        debugDescription: "Object key '\(key)' exceeds 255 UTF-8 bytes"
                    )
                )
            }
            out.append(UInt8(keyUTF8.count))
            out.append(contentsOf: keyUTF8)
            try appendValue(dict[key]!, to: &out, codingPath: codingPath)
        }
        out.append(0x00)                        // zero-length key terminator
    }
}

@inline(__always)
private func appendLE64(_ value: UInt64, to out: inout [UInt8]) {
    withUnsafeBytes(of: value.littleEndian) { out.append(contentsOf: $0) }
}

// MARK: - Internal encoder

final class _SnowedEncoder: Encoder {
    var codingPath: [CodingKey]
    var userInfo: [CodingUserInfoKey: Any]
    var storage: SnowedValue = .null

    init(codingPath: [CodingKey], userInfo: [CodingUserInfoKey: Any]) {
        self.codingPath = codingPath
        self.userInfo = userInfo
    }

    func container<Key: CodingKey>(keyedBy type: Key.Type) -> KeyedEncodingContainer<Key> {
        let container = _SnowedKeyedEncodingContainer<Key>(
            codingPath: codingPath,
            userInfo: userInfo,
            commit: { [weak self] in self?.storage = $0 }
        )
        return KeyedEncodingContainer(container)
    }

    func unkeyedContainer() -> UnkeyedEncodingContainer {
        _SnowedUnkeyedEncodingContainer(
            codingPath: codingPath,
            userInfo: userInfo,
            commit: { [weak self] in self?.storage = $0 }
        )
    }

    func singleValueContainer() -> SingleValueEncodingContainer {
        _SnowedSingleValueEncodingContainer(
            codingPath: codingPath,
            userInfo: userInfo,
            commit: { [weak self] in self?.storage = $0 }
        )
    }
}

// MARK: - Keyed encoding container

private final class _SnowedKeyedEncodingContainer<Key: CodingKey>: KeyedEncodingContainerProtocol {
    var codingPath: [CodingKey]
    private let userInfo: [CodingUserInfoKey: Any]
    private let commit: (SnowedValue) -> Void
    private var pairs: [(String, SnowedValue)] = []
    private var pendingEncoders: [(String, _SnowedEncoder)] = []

    init(codingPath: [CodingKey], userInfo: [CodingUserInfoKey: Any], commit: @escaping (SnowedValue) -> Void) {
        self.codingPath = codingPath
        self.userInfo = userInfo
        self.commit = commit
    }

    deinit {
        for (k, enc) in pendingEncoders { set(enc.storage, keyStr: k) }
        var dict: [String: SnowedValue] = [:]
        for (k, v) in pairs { dict[k] = v }
        commit(.object(dict))
    }

    private func set(_ value: SnowedValue, keyStr: String) {
        if let idx = pairs.lastIndex(where: { $0.0 == keyStr }) {
            pairs[idx].1 = value
        } else {
            pairs.append((keyStr, value))
        }
    }

    private func set(_ value: SnowedValue, key: Key) { set(value, keyStr: key.stringValue) }

    func encodeNil(forKey key: Key)             throws { set(.null,          key: key) }
    func encode(_ v: Bool,   forKey key: Key)   throws { set(.bool(v),       key: key) }
    func encode(_ v: Int,    forKey key: Key)   throws { set(.int64(Int64(v)), key: key) }
    func encode(_ v: Int8,   forKey key: Key)   throws { set(.int64(Int64(v)), key: key) }
    func encode(_ v: Int16,  forKey key: Key)   throws { set(.int64(Int64(v)), key: key) }
    func encode(_ v: Int32,  forKey key: Key)   throws { set(.int64(Int64(v)), key: key) }
    func encode(_ v: Int64,  forKey key: Key)   throws { set(.int64(v),      key: key) }
    func encode(_ v: UInt,   forKey key: Key)   throws { set(.uint64(UInt64(v)), key: key) }
    func encode(_ v: UInt8,  forKey key: Key)   throws { set(.uint64(UInt64(v)), key: key) }
    func encode(_ v: UInt16, forKey key: Key)   throws { set(.uint64(UInt64(v)), key: key) }
    func encode(_ v: UInt32, forKey key: Key)   throws { set(.uint64(UInt64(v)), key: key) }
    func encode(_ v: UInt64, forKey key: Key)   throws { set(.uint64(v),     key: key) }
    func encode(_ v: Float,  forKey key: Key)   throws { set(.float64(Double(v)), key: key) }
    func encode(_ v: Double, forKey key: Key)   throws { set(.float64(v),    key: key) }
    func encode(_ v: String, forKey key: Key)   throws { set(.string(v),     key: key) }

    func encode<T: Encodable>(_ value: T, forKey key: Key) throws {
        switch value {
        case let v as Data:    set(.binary(v),   key: key); return
        case let v as Date:    set(.dateTime(v), key: key); return
        case let v as Decimal: set(.decimal(v),  key: key); return
        case let v as UUID:    set(.uuid(v),     key: key); return
        default: break
        }
        let sub = _SnowedEncoder(codingPath: codingPath + [key], userInfo: userInfo)
        try value.encode(to: sub)
        set(sub.storage, keyStr: key.stringValue)
    }

    func nestedContainer<NK: CodingKey>(keyedBy type: NK.Type, forKey key: Key) -> KeyedEncodingContainer<NK> {
        let keyStr = key.stringValue
        let container = _SnowedKeyedEncodingContainer<NK>(
            codingPath: codingPath + [key],
            userInfo: userInfo,
            commit: { [weak self] in self?.set($0, keyStr: keyStr) }
        )
        return KeyedEncodingContainer(container)
    }

    func nestedUnkeyedContainer(forKey key: Key) -> UnkeyedEncodingContainer {
        let keyStr = key.stringValue
        return _SnowedUnkeyedEncodingContainer(
            codingPath: codingPath + [key],
            userInfo: userInfo,
            commit: { [weak self] in self?.set($0, keyStr: keyStr) }
        )
    }

    func superEncoder() -> Encoder                  { makeSuperEncoder(keyStr: "super") }
    func superEncoder(forKey key: Key) -> Encoder   { makeSuperEncoder(keyStr: key.stringValue) }

    private func makeSuperEncoder(keyStr: String) -> Encoder {
        let enc = _SnowedEncoder(codingPath: codingPath + [_SnowedKey(stringValue: keyStr)], userInfo: userInfo)
        pendingEncoders.append((keyStr, enc))
        return enc
    }
}

// MARK: - Unkeyed encoding container

private final class _SnowedUnkeyedEncodingContainer: UnkeyedEncodingContainer {
    var codingPath: [CodingKey]
    private let userInfo: [CodingUserInfoKey: Any]
    private let commit: (SnowedValue) -> Void
    private var elements: [SnowedValue] = []
    private var pendingSuperEncoders: [(Int, _SnowedEncoder)] = []
    var count: Int { elements.count }

    init(codingPath: [CodingKey], userInfo: [CodingUserInfoKey: Any], commit: @escaping (SnowedValue) -> Void) {
        self.codingPath = codingPath
        self.userInfo = userInfo
        self.commit = commit
    }

    deinit {
        for (idx, enc) in pendingSuperEncoders { elements[idx] = enc.storage }
        commit(.array(elements))
    }

    private func push(_ value: SnowedValue) { elements.append(value) }

    func encodeNil()             throws { push(.null) }
    func encode(_ v: Bool)       throws { push(.bool(v)) }
    func encode(_ v: Int)        throws { push(.int64(Int64(v))) }
    func encode(_ v: Int8)       throws { push(.int64(Int64(v))) }
    func encode(_ v: Int16)      throws { push(.int64(Int64(v))) }
    func encode(_ v: Int32)      throws { push(.int64(Int64(v))) }
    func encode(_ v: Int64)      throws { push(.int64(v)) }
    func encode(_ v: UInt)       throws { push(.uint64(UInt64(v))) }
    func encode(_ v: UInt8)      throws { push(.uint64(UInt64(v))) }
    func encode(_ v: UInt16)     throws { push(.uint64(UInt64(v))) }
    func encode(_ v: UInt32)     throws { push(.uint64(UInt64(v))) }
    func encode(_ v: UInt64)     throws { push(.uint64(v)) }
    func encode(_ v: Float)      throws { push(.float64(Double(v))) }
    func encode(_ v: Double)     throws { push(.float64(v)) }
    func encode(_ v: String)     throws { push(.string(v)) }

    func encode<T: Encodable>(_ value: T) throws {
        switch value {
        case let v as Data:    push(.binary(v));   return
        case let v as Date:    push(.dateTime(v)); return
        case let v as Decimal: push(.decimal(v));  return
        case let v as UUID:    push(.uuid(v));     return
        default: break
        }
        let idx = elements.count
        elements.append(.null)                      // reserve slot
        let sub = _SnowedEncoder(codingPath: codingPath + [_SnowedKey(intValue: idx)], userInfo: userInfo)
        try value.encode(to: sub)
        elements[idx] = sub.storage
    }

    func nestedContainer<NK: CodingKey>(keyedBy type: NK.Type) -> KeyedEncodingContainer<NK> {
        let idx = elements.count
        elements.append(.null)
        let container = _SnowedKeyedEncodingContainer<NK>(
            codingPath: codingPath + [_SnowedKey(intValue: idx)],
            userInfo: userInfo,
            commit: { [weak self] in self?.elements[idx] = $0 }
        )
        return KeyedEncodingContainer(container)
    }

    func nestedUnkeyedContainer() -> UnkeyedEncodingContainer {
        let idx = elements.count
        elements.append(.null)
        return _SnowedUnkeyedEncodingContainer(
            codingPath: codingPath + [_SnowedKey(intValue: idx)],
            userInfo: userInfo,
            commit: { [weak self] in self?.elements[idx] = $0 }
        )
    }

    func superEncoder() -> Encoder {
        let idx = elements.count
        elements.append(.null)
        let enc = _SnowedEncoder(codingPath: codingPath + [_SnowedKey(intValue: idx)], userInfo: userInfo)
        pendingSuperEncoders.append((idx, enc))
        return enc
    }
}

// MARK: - Single-value encoding container

private final class _SnowedSingleValueEncodingContainer: SingleValueEncodingContainer {
    var codingPath: [CodingKey]
    private let userInfo: [CodingUserInfoKey: Any]
    private let commit: (SnowedValue) -> Void

    init(codingPath: [CodingKey], userInfo: [CodingUserInfoKey: Any], commit: @escaping (SnowedValue) -> Void) {
        self.codingPath = codingPath
        self.userInfo = userInfo
        self.commit = commit
    }

    func encodeNil()         throws { commit(.null) }
    func encode(_ v: Bool)   throws { commit(.bool(v)) }
    func encode(_ v: Int)    throws { commit(.int64(Int64(v))) }
    func encode(_ v: Int8)   throws { commit(.int64(Int64(v))) }
    func encode(_ v: Int16)  throws { commit(.int64(Int64(v))) }
    func encode(_ v: Int32)  throws { commit(.int64(Int64(v))) }
    func encode(_ v: Int64)  throws { commit(.int64(v)) }
    func encode(_ v: UInt)   throws { commit(.uint64(UInt64(v))) }
    func encode(_ v: UInt8)  throws { commit(.uint64(UInt64(v))) }
    func encode(_ v: UInt16) throws { commit(.uint64(UInt64(v))) }
    func encode(_ v: UInt32) throws { commit(.uint64(UInt64(v))) }
    func encode(_ v: UInt64) throws { commit(.uint64(v)) }
    func encode(_ v: Float)  throws { commit(.float64(Double(v))) }
    func encode(_ v: Double) throws { commit(.float64(v)) }
    func encode(_ v: String) throws { commit(.string(v)) }

    func encode<T: Encodable>(_ value: T) throws {
        switch value {
        case let v as Data:    commit(.binary(v));   return
        case let v as Date:    commit(.dateTime(v)); return
        case let v as Decimal: commit(.decimal(v));  return
        case let v as UUID:    commit(.uuid(v));     return
        default: break
        }
        let sub = _SnowedEncoder(codingPath: codingPath, userInfo: userInfo)
        try value.encode(to: sub)
        commit(sub.storage)
    }
}

// MARK: - Coding key helper

internal struct _SnowedKey: CodingKey {
    var stringValue: String
    var intValue: Int?

    init(stringValue: String) { self.stringValue = stringValue }
    init(intValue: Int)       { self.intValue = intValue; self.stringValue = "\(intValue)" }
}
