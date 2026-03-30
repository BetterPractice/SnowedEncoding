import Testing
import Foundation
@testable import Snowed

@Suite("Round Trip")
struct RoundTripTests {

    let encoder = SnowedEncoder()
    let decoder = SnowedDecoder()

    // MARK: - SnowedValue round-trips

    @Test func null() throws {
        #expect(try roundTrip(.null) == .null)
    }

    @Test func boolFalse() throws {
        #expect(try roundTrip(.bool(false)) == .bool(false))
    }

    @Test func boolTrue() throws {
        #expect(try roundTrip(.bool(true)) == .bool(true))
    }

    @Test func int64Zero() throws {
        #expect(try roundTrip(.int64(0)) == .int64(0))
    }

    @Test func int64Positive() throws {
        #expect(try roundTrip(.int64(42)) == .int64(42))
    }

    @Test func int64Negative() throws {
        #expect(try roundTrip(.int64(-1)) == .int64(-1))
    }

    @Test func int64Min() throws {
        #expect(try roundTrip(.int64(.min)) == .int64(.min))
    }

    @Test func int64Max() throws {
        #expect(try roundTrip(.int64(.max)) == .int64(.max))
    }

    @Test func uint64Zero() throws {
        #expect(try roundTrip(.uint64(0)) == .uint64(0))
    }

    @Test func uint64Max() throws {
        #expect(try roundTrip(.uint64(.max)) == .uint64(.max))
    }

    @Test func uint64AboveInt64Max() throws {
        let val = UInt64(Int64.max) + 1
        #expect(try roundTrip(.uint64(val)) == .uint64(val))
    }

    @Test func float64() throws {
        #expect(try roundTrip(.float64(3.14)) == .float64(3.14))
    }

    @Test func float64NegativeZero() throws {
        // -0.0 and 0.0 have different bit patterns; both must survive round-trip
        let rt = try roundTrip(.float64(-0.0))
        guard case .float64(let v) = rt else { Issue.record("Expected .float64"); return }
        #expect(v.sign == .minus)
    }

    @Test func float64Infinity() throws {
        #expect(try roundTrip(.float64(.infinity))  == .float64(.infinity))
        #expect(try roundTrip(.float64(-.infinity)) == .float64(-.infinity))
    }

    @Test func float64NaN() throws {
        let rt = try roundTrip(.float64(.nan))
        guard case .float64(let v) = rt else { Issue.record("Expected .float64"); return }
        #expect(v.isNaN)
    }

    @Test func dateTime() throws {
        let date = Date(timeIntervalSince1970: 1_700_000_000.0)
        let rt = try roundTrip(.dateTime(date))
        guard case .dateTime(let d) = rt else { Issue.record("Expected .dateTime"); return }
        #expect(abs(d.timeIntervalSince1970 - date.timeIntervalSince1970) < 0.001)
    }

    @Test func dateTimeEpoch() throws {
        let epoch = Date(timeIntervalSince1970: 0)
        let rt = try roundTrip(.dateTime(epoch))
        guard case .dateTime(let d) = rt else { Issue.record("Expected .dateTime"); return }
        #expect(abs(d.timeIntervalSince1970) < 0.001)
    }

    @Test func dateTimeNegative() throws {
        let before = Date(timeIntervalSince1970: -1_000.0)
        let rt = try roundTrip(.dateTime(before))
        guard case .dateTime(let d) = rt else { Issue.record("Expected .dateTime"); return }
        #expect(abs(d.timeIntervalSince1970 - (-1_000.0)) < 0.001)
    }

    @Test func binaryEmpty() throws {
        #expect(try roundTrip(.binary(Data())) == .binary(Data()))
    }

    @Test func binaryData() throws {
        let data = Data([0xDE, 0xAD, 0xBE, 0xEF])
        #expect(try roundTrip(.binary(data)) == .binary(data))
    }

    @Test func stringEmpty() throws {
        #expect(try roundTrip(.string("")) == .string(""))
    }

    @Test func stringASCII() throws {
        #expect(try roundTrip(.string("hello")) == .string("hello"))
    }

    @Test func stringUnicode() throws {
        let s = "Héllo wörld 🌍"
        #expect(try roundTrip(.string(s)) == .string(s))
    }

    @Test func decimalInteger() throws {
        #expect(try roundTrip(.decimal(Decimal(42))) == .decimal(Decimal(42)))
    }

    @Test func decimalFractional() throws {
        let d = Decimal(string: "3.14159")!
        #expect(try roundTrip(.decimal(d)) == .decimal(d))
    }

    @Test func decimalNegative() throws {
        let d = Decimal(string: "-99.99")!
        #expect(try roundTrip(.decimal(d)) == .decimal(d))
    }

    @Test func decimalScaleNotPreserved() throws {
        // 1.00 and 1.0 and 1 all canonicalize to "1"
        let rt = try roundTrip(.decimal(Decimal(string: "1.00")!))
        #expect(rt == .decimal(Decimal(1)))
        #expect(rt == .decimal(Decimal(string: "1.0")!))
    }

    @Test func uuidRoundTrip() throws {
        let u = UUID()
        #expect(try roundTrip(.uuid(u)) == .uuid(u))
    }

    @Test func uuidKnownValue() throws {
        // 550e8400-e29b-41d4-a716-446655440000 in RFC 4122 byte order
        let expected: [UInt8] = [
            0x13,
            0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,  // length = 16
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
            0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00
        ]
        let u = UUID(uuidString: "550e8400-e29b-41d4-a716-446655440000")!
        #expect([UInt8](try encoder.encode(SnowedValue.uuid(u))) == expected)
    }

    @Test func uuidWireFormatDecodes() throws {
        let bytes: [UInt8] = [
            0x13,
            0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
            0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00
        ]
        let value = try decoder.decode(from: Data(bytes))
        let expected = UUID(uuidString: "550e8400-e29b-41d4-a716-446655440000")!
        #expect(value == .uuid(expected))
    }

    @Test func uuidInvalidLengthThrows() {
        // UUID with wrong payload length should throw
        let bytes: [UInt8] = [
            0x13,
            0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,  // length = 4, wrong
            0x01, 0x02, 0x03, 0x04
        ]
        #expect(throws: (any Error).self) { try decoder.decode(from: Data(bytes)) }
    }

    struct WithUUID: Codable, Equatable {
        var id: UUID
    }

    @Test func codableUUID() throws {
        let original = WithUUID(id: UUID())
        let data = try encoder.encode(original)
        #expect(try decoder.decode(WithUUID.self, from: data) == original)
    }

    @Test func arrayEmpty() throws {
        #expect(try roundTrip(.array([])) == .array([]))
    }

    @Test func arrayMixed() throws {
        let arr: SnowedValue = .array([.null, .bool(true), .int64(7), .string("x")])
        #expect(try roundTrip(arr) == arr)
    }

    @Test func arrayNested() throws {
        let arr: SnowedValue = .array([.array([.int64(1), .int64(2)]), .array([])])
        #expect(try roundTrip(arr) == arr)
    }

    @Test func objectEmpty() throws {
        #expect(try roundTrip(.object([:])) == .object([:]))
    }

    @Test func objectSimple() throws {
        let obj: SnowedValue = .object(["name": .string("Alice"), "age": .int64(30)])
        #expect(try roundTrip(obj) == obj)
    }

    @Test func objectNested() throws {
        let inner: SnowedValue = .object(["x": .int64(1)])
        let outer: SnowedValue = .object(["child": inner, "flag": .bool(false)])
        #expect(try roundTrip(outer) == outer)
    }

    @Test func objectMaxLengthKey() throws {
        let key = String(repeating: "k", count: 255)
        let obj: SnowedValue = .object([key: .null])
        #expect(try roundTrip(obj) == obj)
    }

    @Test func objectKeyTooLong() {
        let key = String(repeating: "k", count: 256)
        let obj: SnowedValue = .object([key: .null])
        #expect(throws: (any Error).self) { try encoder.encode(obj) }
    }

    // MARK: - Codable round-trips

    struct Simple: Codable, Equatable {
        var name: String
        var count: Int
        var ratio: Double
        var active: Bool
    }

    @Test func codableSimpleStruct() throws {
        let original = Simple(name: "test", count: 42, ratio: 0.5, active: true)
        let data = try encoder.encode(original)
        #expect(try decoder.decode(Simple.self, from: data) == original)
    }

    struct WithOptional: Codable, Equatable {
        var value: String?
    }

    @Test func codableOptionalNil() throws {
        let original = WithOptional(value: nil)
        let data = try encoder.encode(original)
        #expect(try decoder.decode(WithOptional.self, from: data) == original)
    }

    @Test func codableOptionalPresent() throws {
        let original = WithOptional(value: "hello")
        let data = try encoder.encode(original)
        #expect(try decoder.decode(WithOptional.self, from: data) == original)
    }

    struct WithData: Codable, Equatable {
        var blob: Data
    }

    @Test func codableData() throws {
        let original = WithData(blob: Data([1, 2, 3, 4]))
        let data = try encoder.encode(original)
        #expect(try decoder.decode(WithData.self, from: data) == original)
    }

    struct WithDate: Codable {
        var ts: Date
    }

    @Test func codableDate() throws {
        let original = WithDate(ts: Date(timeIntervalSince1970: 1_000_000.0))
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(WithDate.self, from: data)
        #expect(abs(decoded.ts.timeIntervalSince1970 - original.ts.timeIntervalSince1970) < 0.001)
    }

    struct WithDecimal: Codable, Equatable {
        var price: Decimal
    }

    @Test func codableDecimal() throws {
        let original = WithDecimal(price: Decimal(string: "19.99")!)
        let data = try encoder.encode(original)
        #expect(try decoder.decode(WithDecimal.self, from: data) == original)
    }

    @Test func codableArray() throws {
        let original = [1, 2, 3]
        let data = try encoder.encode(original)
        #expect(try decoder.decode([Int].self, from: data) == original)
    }

    @Test func codableNestedStruct() throws {
        struct Inner: Codable, Equatable { var x: Int }
        struct Outer: Codable, Equatable { var inner: Inner; var label: String }
        let original = Outer(inner: Inner(x: 7), label: "test")
        let data = try encoder.encode(original)
        #expect(try decoder.decode(Outer.self, from: data) == original)
    }

    // MARK: - Numeric coercion

    @Test func int64DecodedAsInt32() throws {
        let data = try encoder.encode(SnowedValue.int64(100))
        #expect(try decoder.decode(Int32.self, from: data) == 100)
    }

    @Test func int64OverflowsInt32() throws {
        let data = try encoder.encode(SnowedValue.int64(Int64(Int32.max) + 1))
        #expect(throws: (any Error).self) { try decoder.decode(Int32.self, from: data) }
    }

    @Test func uint64DecodedAsInt64WhenSafe() throws {
        let data = try encoder.encode(SnowedValue.uint64(1000))
        #expect(try decoder.decode(Int64.self, from: data) == 1000)
    }

    @Test func float64DecodedAsIntWhenWhole() throws {
        let data = try encoder.encode(SnowedValue.float64(7.0))
        #expect(try decoder.decode(Int64.self, from: data) == 7)
    }

    @Test func float64WithFractionFailsIntDecode() throws {
        let data = try encoder.encode(SnowedValue.float64(7.5))
        #expect(throws: (any Error).self) { try decoder.decode(Int64.self, from: data) }
    }

    // MARK: - Wire format spot checks

    @Test func nullWireFormat() throws {
        #expect([UInt8](try encoder.encode(SnowedValue.null)) == [0x00])
    }

    @Test func boolFalseWireFormat() throws {
        #expect([UInt8](try encoder.encode(SnowedValue.bool(false))) == [0x02])
    }

    @Test func boolTrueWireFormat() throws {
        #expect([UInt8](try encoder.encode(SnowedValue.bool(true))) == [0x03])
    }

    @Test func int64WireFormat() throws {
        // 42 little-endian = 0x2A followed by seven 0x00 bytes
        let expected: [UInt8] = [0x04, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        #expect([UInt8](try encoder.encode(SnowedValue.int64(42))) == expected)
    }

    @Test func uint64MaxWireFormat() throws {
        let expected: [UInt8] = [0x05, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        #expect([UInt8](try encoder.encode(SnowedValue.uint64(UInt64.max))) == expected)
    }

    @Test func stringWireFormat() throws {
        let expected: [UInt8] = [
            0x11,
            0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x68, 0x65, 0x6C, 0x6C, 0x6F
        ]
        #expect([UInt8](try encoder.encode(SnowedValue.string("hello"))) == expected)
    }

    @Test func emptyArrayWireFormat() throws {
        #expect([UInt8](try encoder.encode(SnowedValue.array([]))) == [0x40, 0xFF])
    }

    @Test func emptyObjectWireFormat() throws {
        #expect([UInt8](try encoder.encode(SnowedValue.object([:]))) == [0x80, 0x00])
    }

    // MARK: - Forward compatibility

    @Test func unknownNullSubtypeParsedAsNull() throws {
        #expect(try decoder.decode(from: Data([0x01])) == .null)
    }

    @Test func unknownIntSubtypeParsedAsInt64() throws {
        var bytes: [UInt8] = [0x06]
        bytes += withUnsafeBytes(of: UInt64(99).littleEndian) { Array($0) }
        #expect(try decoder.decode(from: Data(bytes)) == .int64(99))
    }

    @Test func unknownFloatSubtypeParsedAsFloat64() throws {
        var bytes: [UInt8] = [0x0A]
        bytes += withUnsafeBytes(of: Double(1.0).bitPattern.littleEndian) { Array($0) }
        #expect(try decoder.decode(from: Data(bytes)) == .float64(1.0))
    }

    @Test func unknownBinarySubtypeParsedAsBinary() throws {
        // 0x14 is an unknown byte in the Binary family; should round-down to Binary
        var bytes: [UInt8] = [0x14]
        bytes += withUnsafeBytes(of: UInt64(2).littleEndian) { Array($0) }
        bytes += [0xAB, 0xCD]
        #expect(try decoder.decode(from: Data(bytes)) == .binary(Data([0xAB, 0xCD])))
    }

    // MARK: - Object termination

    @Test func objectTerminatedByZeroLengthKey() throws {
        let bytes: [UInt8] = [
            0x80,
            0x01, 0x61,     // key "a"
            0x03,           // Boolean True
            0x00            // zero-length key terminator
        ]
        #expect(try decoder.decode(from: Data(bytes)) == .object(["a": .bool(true)]))
    }

    @Test func objectTerminatedBySentinel() throws {
        // The key before the Sentinel is discarded per spec
        let bytes: [UInt8] = [
            0x80,
            0x01, 0x61,     // key "a"
            0x03,           // Boolean True
            0x01, 0x62,     // key "b" — discarded
            0xFF            // Sentinel in value position
        ]
        #expect(try decoder.decode(from: Data(bytes)) == .object(["a": .bool(true)]))
    }

    // MARK: - Helper

    private func roundTrip(_ value: SnowedValue) throws -> SnowedValue {
        try decoder.decode(from: encoder.encode(value))
    }
}
