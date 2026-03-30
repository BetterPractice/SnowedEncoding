# Conformance Tests

This directory contains the cross-implementation conformance test suite for the Snowed encoding format. All Snowed implementations must pass these tests.

## Structure

```
conformance/
├── README.md
└── vectors/
    ├── null.json
    ├── boolean.json
    ├── int64.json
    ├── float64.json
    ├── datetime.json
    ├── binary.json
    ├── string.json
    ├── array.json
    ├── object.json
    ├── nested.json
    └── forward_compat.json
```

## Vector File Format

Each file in `vectors/` is a JSON document describing a suite of test cases:

```json
{
  "suite": "<suite name>",
  "cases": [
    {
      "name": "<case name>",
      "description": "<human-readable description>",
      "hex": "<space-separated hex bytes of the Snowed encoding>",
      "decoded": {
        "type": "<snowed type>",
        "value": "<see Value Representations below>"
      }
    }
  ]
}
```

The `hex` field contains the complete Snowed encoding of a single root value, expressed as space-separated uppercase hex bytes (e.g. `"04 2A 00 00 00 00 00 00 00"`).

## Value Representations

JSON has a limited type system. The following conventions are used in the `decoded.value` field:

| Snowed Type | `decoded.type` | `decoded.value` representation |
|-------------|----------------|-------------------------------|
| Null        | `"null"`        | `null` |
| Boolean     | `"bool"`        | `true` or `false` |
| Int64       | `"int64"`       | JSON number for values in ±2⁵³; string (e.g. `"9007199254740993"`) for larger values |
| Float64     | `"float64"`     | JSON number, or `"Infinity"`, `"-Infinity"`, `"NaN"` as strings |
| DateTime    | `"datetime"`    | JSON number (milliseconds since Unix epoch) |
| Binary      | `"binary"`      | Base64-encoded string |
| String      | `"string"`      | JSON string |
| Array       | `"array"`       | JSON array of decoded value objects: `[{"type": "...", "value": ...}]` |
| Object      | `"object"`      | JSON object mapping each key to a decoded value object: `{"key": {"type": "...", "value": ...}}` |

## Running Conformance Tests

Each implementation provides a conformance test runner. See the implementation's README for instructions. In general:

- Read each `.json` file from `vectors/`
- For each case, decode the bytes in `hex` using the implementation under test
- Assert the decoded value matches the `decoded` field
- Additionally, re-encode the decoded value and assert the result matches the original `hex` bytes (round-trip test)
