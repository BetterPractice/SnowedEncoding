# Snowed — Swift Implementation

Swift implementation of the [Snowed binary encoding format](../../spec/snowed-spec.md).

## Requirements

- Swift 5.9+
- No runtime dependencies beyond Foundation

## Usage

### Codable types

```swift
import Snowed

struct Profile: Codable {
    var name: String
    var score: Int64
    var avatar: Data
    var createdAt: Date
}

let encoder = SnowedEncoder()
let data = try encoder.encode(profile)

let decoder = SnowedDecoder()
let profile = try decoder.decode(Profile.self, from: data)
```

### Arbitrary documents (SnowedValue)

```swift
let decoder = SnowedDecoder()
let value = try decoder.decode(from: data)

if case .object(let obj) = value,
   case .string(let name) = obj["name"] {
    print(name)
}
```

Use `@unknown default` when switching over `SnowedValue` cases to remain
forward-compatible with future type additions:

```swift
switch value {
case .null:         print("null")
case .string(let s): print(s)
// ... other cases ...
@unknown default:   print("unknown future type")
}
```

## Platform limitations

See [../../spec/platform-limitations.md](../../spec/platform-limitations.md) for the full list. Key points:

- `Decimal` encodes as Snowed Decimal (0x12) via canonical string; scale is not preserved across round-trips.
- `Float` values are widened to `Double` on encode.

## Future work

- **Async streaming API:** An interface accepting `AsyncSequence<UInt8>` and
  emitting `AsyncSequence<SnowedValue>` is planned for a future version. This
  will suit the primary streaming/network use case without requiring the entire
  document to be buffered into `Data` first. The current `Data`-based API will
  remain unchanged alongside it.

## Running tests

```sh
swift test
```
