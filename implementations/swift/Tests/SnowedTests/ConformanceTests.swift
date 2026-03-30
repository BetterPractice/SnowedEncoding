import Testing
import Foundation
@testable import Snowed

/// Runs the shared cross-platform conformance test vectors from tests/conformance/vectors/.
///
/// Each vector file is a JSON document describing a suite of cases. Each case
/// provides a hex-encoded Snowed payload and an expected decoded value.
/// This runner decodes the payload and compares the result to the expectation.
///
/// Vector files are only present once conformance vectors have been authored.
/// If none are found, the test passes vacuously with a note.
@Suite("Conformance Vectors")
struct ConformanceTests {

    let decoder = SnowedDecoder()

    @Test func conformanceVectors() throws {
        guard let vectorsURL = vectorsDirectory() else {
            print("[ConformanceTests] No vectors directory found — skipping.")
            return
        }

        let files = try FileManager.default.contentsOfDirectory(
            at: vectorsURL,
            includingPropertiesForKeys: nil
        )
        .filter { $0.pathExtension == "json" }
        .sorted { $0.lastPathComponent < $1.lastPathComponent }

        guard !files.isEmpty else {
            print("[ConformanceTests] No vector files found — skipping.")
            return
        }

        for file in files {
            let suite = try loadSuite(from: file)
            for testCase in suite.cases {
                let bytes = try parseHex(testCase.hex)
                let actual = try decoder.decode(from: Data(bytes))
                let expected = try buildExpected(from: testCase.decoded)
                #expect(actual == expected, "[\(suite.suite)] \(testCase.name): decoded value mismatch")
            }
        }
    }

    // MARK: - Vector location

    private func vectorsDirectory() -> URL? {
        // Walk up from this source file to the repo root and locate the shared vectors.
        let file = URL(fileURLWithPath: #filePath)
        var url = file
        for _ in 0 ..< 4 { url = url.deletingLastPathComponent() }
        let vectors = url.appendingPathComponent("tests/conformance/vectors")
        return FileManager.default.fileExists(atPath: vectors.path) ? vectors : nil
    }

    // MARK: - Vector loading

    private func loadSuite(from url: URL) throws -> VectorSuite {
        try JSONDecoder().decode(VectorSuite.self, from: Data(contentsOf: url))
    }

    // MARK: - Hex parsing

    private func parseHex(_ hex: String) throws -> [UInt8] {
        try hex.split(separator: " ").map { token in
            guard token.count == 2, let byte = UInt8(token, radix: 16) else {
                throw ConformanceError.invalidHex(String(token))
            }
            return byte
        }
    }

    // MARK: - Expected value construction

    private func buildExpected(from decoded: DecodedValue) throws -> SnowedValue {
        switch decoded.type {
        case "null":
            return .null

        case "bool":
            guard case .bool(let b) = decoded.value else {
                throw ConformanceError.invalidExpected("bool requires JSON bool value")
            }
            return .bool(b)

        case "int64":
            switch decoded.value {
            case .number(let n): return .int64(Int64(n))
            case .string(let s):
                guard let i = Int64(s) else {
                    throw ConformanceError.invalidExpected("Cannot parse int64 from '\(s)'")
                }
                return .int64(i)
            default:
                throw ConformanceError.invalidExpected("int64 requires number or string value")
            }

        case "uint64":
            switch decoded.value {
            case .number(let n): return .uint64(UInt64(n))
            case .string(let s):
                guard let u = UInt64(s) else {
                    throw ConformanceError.invalidExpected("Cannot parse uint64 from '\(s)'")
                }
                return .uint64(u)
            default:
                throw ConformanceError.invalidExpected("uint64 requires number or string value")
            }

        case "float64":
            switch decoded.value {
            case .number(let n):       return .float64(n)
            case .string("Infinity"):  return .float64(.infinity)
            case .string("-Infinity"): return .float64(-.infinity)
            case .string("NaN"):       return .float64(.nan)
            default:
                throw ConformanceError.invalidExpected("float64 requires number or special string")
            }

        case "datetime":
            guard case .number(let ms) = decoded.value else {
                throw ConformanceError.invalidExpected("datetime requires numeric ms value")
            }
            return .dateTime(Date(timeIntervalSince1970: ms / 1000.0))

        case "binary":
            guard case .string(let b64) = decoded.value,
                  let data = Data(base64Encoded: b64) else {
                throw ConformanceError.invalidExpected("binary requires base64 string value")
            }
            return .binary(data)

        case "string":
            guard case .string(let s) = decoded.value else {
                throw ConformanceError.invalidExpected("string requires JSON string value")
            }
            return .string(s)

        case "decimal":
            guard case .string(let s) = decoded.value,
                  let d = Decimal(string: s) else {
                throw ConformanceError.invalidExpected("decimal requires string value")
            }
            return .decimal(d)

        case "array":
            guard case .array(let items) = decoded.value else {
                throw ConformanceError.invalidExpected("array requires JSON array value")
            }
            return .array(try items.map { try buildExpected(from: $0) })

        case "object":
            guard case .object(let map) = decoded.value else {
                throw ConformanceError.invalidExpected("object requires JSON object value")
            }
            var result: [String: SnowedValue] = [:]
            for (k, v) in map { result[k] = try buildExpected(from: v) }
            return .object(result)

        default:
            throw ConformanceError.unknownType(decoded.type)
        }
    }
}

// MARK: - Vector file models

private struct VectorSuite: Decodable {
    let suite: String
    let cases: [VectorCase]
}

private struct VectorCase: Decodable {
    let name: String
    let description: String?
    let hex: String
    let decoded: DecodedValue
}

private struct DecodedValue: Decodable {
    let type: String
    let value: JSONValue
}

private enum JSONValue: Decodable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([DecodedValue])
    case object([String: DecodedValue])

    init(from decoder: any Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil()                                      { self = .null;    return }
        if let b = try? c.decode(Bool.self)                   { self = .bool(b); return }
        if let n = try? c.decode(Double.self)                 { self = .number(n); return }
        if let s = try? c.decode(String.self)                 { self = .string(s); return }
        if let a = try? c.decode([DecodedValue].self)         { self = .array(a);  return }
        if let o = try? c.decode([String: DecodedValue].self) { self = .object(o); return }
        throw DecodingError.dataCorruptedError(in: c, debugDescription: "Unrecognised JSON value")
    }
}

// MARK: - Errors

private enum ConformanceError: Error, CustomStringConvertible {
    case invalidHex(String)
    case invalidExpected(String)
    case unknownType(String)

    var description: String {
        switch self {
        case .invalidHex(let s):      return "Invalid hex token: '\(s)'"
        case .invalidExpected(let s): return "Invalid expected value: \(s)"
        case .unknownType(let t):     return "Unknown decoded type: '\(t)'"
        }
    }
}
