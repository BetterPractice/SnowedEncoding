# Snowed Encoding

A binary data serialization format optimized for nested structures containing binary blobs, with a native DateTime type and self-describing wire format.

## Specification

- [spec/snowed-spec.md](spec/snowed-spec.md) — canonical wire format specification
- [spec/platform-limitations.md](spec/platform-limitations.md) — known limitations per implementation platform

## Implementations

| Language | Path                             | Status   |
|----------|----------------------------------|----------|
| Swift    | [implementations/swift](implementations/swift)     | Complete |
| C#       | [implementations/csharp](implementations/csharp)   | Complete |
| Rust     | [implementations/rust](implementations/rust)       | Complete |
| Go       | [implementations/go](implementations/go)           | Complete |
| Kotlin   | [implementations/kotlin](implementations/kotlin)   | Complete |
| Python   | [implementations/python](implementations/python)   | Complete |

All implementations target zero runtime dependencies beyond the host language's standard library.

## Conformance Testing

Cross-implementation test vectors are in [tests/conformance](tests/conformance). Each implementation is expected to provide a test runner that validates against these vectors.
