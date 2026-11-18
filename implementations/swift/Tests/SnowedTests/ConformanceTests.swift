import Testing
import Foundation
@testable import Snowed

/// Runs the shared cross-implementation conformance vectors in `tests/conformance/vectors/`.
///
/// See `tests/conformance/README.md` for the vector format. For every case with an expected
/// value, the runner checks the decoded value, a semantic re-encode round-trip, and (unless the
/// case sets `"canonical": false`) that re-encoding reproduces the original bytes exactly.
/// Cases with `"error": true` must fail to decode.
@Suite("Conformance Vectors")
struct ConformanceTests {

    static let loaded: Result<[VectorCase], Error> = Result { try loadAllCases() }
    static var allCases: [VectorCase] { (try? loaded.get()) ?? [] }

    @Test func vectorsArePresent() throws {
        let cases = try Self.loaded.get()
        #expect(!cases.isEmpty, "No conformance vectors found in tests/conformance/vectors")
    }

    @Test(arguments: allCases)
    func vector(_ testCase: VectorCase) throws {
        let bytes = try parseHex(testCase.hex)

        if testCase.error == true {
            #expect(throws: (any Error).self, "\(testCase): decoding should fail") {
                _ = try SnowedDecoder().decode(from: Data(bytes))
            }
            return
        }

        guard let expected = testCase.decoded else {
            Issue.record("\(testCase): case has neither `decoded` nor `error`")
            return
        }

        let decoded = try SnowedDecoder().decode(from: Data(bytes))
        if let mismatch = mismatch(decoded, expected, path: "$") {
            Issue.record("\(testCase): decode mismatch at \(mismatch)")
            return
        }

        let reencoded = try SnowedEncoder().encode(decoded)
        let redecoded = try SnowedDecoder().decode(from: reencoded)
        if let mismatch = mismatch(redecoded, expected, path: "$") {
            Issue.record("\(testCase): round-trip mismatch at \(mismatch)")
        }

        if testCase.canonical ?? true {
            #expect(Array(reencoded) == bytes,
                    "\(testCase): re-encoded bytes differ\n  expected: \(testCase.hex)\n  actual:   \(hex(reencoded))")
        }
    }
}

// MARK: - Comparison

/// Returns a description of the first difference between `actual` and `expected`, or nil if they match.
private func mismatch(_ actual: SnowedValue, _ expected: ExpectedValue, path: String) -> String? {
    func fail(_ detail: String) -> String { "\(path): expected \(expected.type), \(detail), got \(actual)" }

    switch (expected.type, actual) {
    case ("null", .null):
        return nil

    case ("bool", .bool(let b)):
        guard case .bool(let e) = expected.value else { return fail("bad vector value") }
        return b == e ? nil : fail("value \(e)")

    case ("int64", .int64(let i)):
        guard let e = expected.value.int64 else { return fail("bad vector value") }
        return i == e ? nil : fail("value \(e)")

    case ("uint64", .uint64(let u)):
        guard let e = expected.value.uint64 else { return fail("bad vector value") }
        return u == e ? nil : fail("value \(e)")

    case ("float64", .float64(let d)):
        guard let e = expected.value.float64 else { return fail("bad vector value") }
        if e.isNaN { return d.isNaN ? nil : fail("NaN") }
        return (d == e && d.sign == e.sign) ? nil : fail("value \(e)")

    case ("datetime", .dateTime(let date)):
        guard let e = expected.value.float64 else { return fail("bad vector value") }
        let ms = date.timeIntervalSince1970 * 1000
        // Date stores seconds as a Double, so allow sub-microsecond drift; the byte-exact
        // check catches real precision loss for canonical cases.
        return abs(ms - e) < 0.001 ? nil : fail("\(e) ms (actual \(ms) ms)")

    case ("binary", .binary(let data)):
        guard case .string(let b64) = expected.value, let e = Data(base64Encoded: b64) else {
            return fail("bad vector value")
        }
        return data == e ? nil : fail("bytes \(hex(e))")

    case ("string", .string(let s)):
        guard case .string(let e) = expected.value else { return fail("bad vector value") }
        return s == e ? nil : fail("value \(e.debugDescription)")

    case ("decimal", .decimal(let d)):
        guard case .string(let e) = expected.value else { return fail("bad vector value") }
        return d.description == e ? nil : fail("value \(e) (actual \(d.description))")

    case ("uuid", .uuid(let u)):
        guard case .string(let e) = expected.value else { return fail("bad vector value") }
        return u.uuidString.lowercased() == e ? nil : fail("value \(e)")

    case ("array", .array(let items)):
        guard case .array(let e) = expected.value else { return fail("bad vector value") }
        guard items.count == e.count else { return fail("\(e.count) elements") }
        for (i, (a, x)) in zip(items, e).enumerated() {
            if let m = mismatch(a, x, path: "\(path)[\(i)]") { return m }
        }
        return nil

    case ("object", .object(let map)):
        guard case .object(let e) = expected.value else { return fail("bad vector value") }
        guard Set(map.keys) == Set(e.keys) else { return fail("keys \(e.keys.sorted())") }
        for key in e.keys.sorted() {
            if let m = mismatch(map[key]!, e[key]!, path: "\(path).\(key)") { return m }
        }
        return nil

    default:
        return fail("wrong type")
    }
}

// MARK: - Vector loading

private func vectorsDirectory() -> URL {
    // #filePath is .../implementations/swift/Tests/SnowedTests/ConformanceTests.swift;
    // walk up five levels to the repository root.
    var url = URL(fileURLWithPath: #filePath)
    for _ in 0 ..< 5 { url.deleteLastPathComponent() }
    return url.appendingPathComponent("tests/conformance/vectors", isDirectory: true)
}

private func loadAllCases() throws -> [VectorCase] {
    let files = try FileManager.default
        .contentsOfDirectory(at: vectorsDirectory(), includingPropertiesForKeys: nil)
        .filter { $0.pathExtension == "json" }
        .sorted { $0.lastPathComponent < $1.lastPathComponent }

    return try files.flatMap { file -> [VectorCase] in
        let suite = try JSONDecoder().decode(VectorSuite.self, from: Data(contentsOf: file))
        return suite.cases.map { c in
            var c = c
            c.suite = suite.suite
            return c
        }
    }
}

private func parseHex(_ hex: String) throws -> [UInt8] {
    try hex.split(separator: " ").map { token in
        guard token.count == 2, let byte = UInt8(token, radix: 16) else {
            throw ConformanceError.invalidHex(String(token))
        }
        return byte
    }
}

private func hex(_ bytes: some Sequence<UInt8>) -> String {
    bytes.map { String(format: "%02X", $0) }.joined(separator: " ")
}

// MARK: - Vector file models

private struct VectorSuite: Decodable {
    let suite: String
    let cases: [VectorCase]
}

struct VectorCase: Decodable, Sendable, CustomTestStringConvertible, CustomStringConvertible {
    var suite = ""
    let name: String
    let hex: String
    let decoded: ExpectedValue?
    let error: Bool?
    let canonical: Bool?

    enum CodingKeys: String, CodingKey { case name, hex, decoded, error, canonical }

    var description: String { "[\(suite)] \(name)" }
    var testDescription: String { "\(suite)/\(name)" }
}

struct ExpectedValue: Decodable, Sendable {
    let type: String
    let value: JSONValue
}

indirect enum JSONValue: Decodable, Sendable {
    case null
    case bool(Bool)
    case integer(Int64)
    case unsigned(UInt64)
    case double(Double)
    case string(String)
    case array([ExpectedValue])
    case object([String: ExpectedValue])

    init(from decoder: any Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil()                                      { self = .null; return }
        if let b = try? c.decode(Bool.self)                   { self = .bool(b); return }
        if let i = try? c.decode(Int64.self)                  { self = .integer(i); return }
        if let u = try? c.decode(UInt64.self)                 { self = .unsigned(u); return }
        if let d = try? c.decode(Double.self)                 { self = .double(d); return }
        if let s = try? c.decode(String.self)                 { self = .string(s); return }
        if let a = try? c.decode([ExpectedValue].self)        { self = .array(a); return }
        if let o = try? c.decode([String: ExpectedValue].self) { self = .object(o); return }
        throw DecodingError.dataCorruptedError(in: c, debugDescription: "Unrecognised JSON value")
    }

    /// Integers are read exactly: from a JSON integer or a decimal string, never via Double.
    var int64: Int64? {
        switch self {
        case .integer(let i): return i
        case .string(let s):  return Int64(s)
        default:              return nil
        }
    }

    var uint64: UInt64? {
        switch self {
        case .integer(let i): return i >= 0 ? UInt64(i) : nil
        case .unsigned(let u): return u
        case .string(let s):  return UInt64(s)
        default:              return nil
        }
    }

    var float64: Double? {
        switch self {
        case .integer(let i):      return Double(i)
        case .unsigned(let u):     return Double(u)
        case .double(let d):       return d
        case .string("Infinity"):  return .infinity
        case .string("-Infinity"): return -.infinity
        case .string("NaN"):       return .nan
        case .string("-0"):        return -0.0
        default:                   return nil
        }
    }
}

private enum ConformanceError: Error, CustomStringConvertible {
    case invalidHex(String)

    var description: String {
        switch self {
        case .invalidHex(let s): return "Invalid hex token: '\(s)'"
        }
    }
}
