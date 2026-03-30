# Snowed for Python

A Python implementation of the [Snowed binary serialization format](../../spec/snowed-spec.md).

Requires Python 3.10+. No external dependencies.

---

## Quick Start

```python
import snowed
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

p = Point(3, -7)
data = snowed.dumps(p)          # bytes
back = snowed.loads_dataclass(data, Point)
assert back == p
```

---

## API

### `snowed.dumps(obj) -> bytes`

Encodes `obj` to Snowed wire bytes. Accepts any supported Python value (see type mapping below).

### `snowed.loads(data) -> object`

Decodes Snowed wire bytes to native Python types. Objects decode as `dict`, arrays as `list`.

### `snowed.loads_value(data) -> SnowedValue`

Decodes to a type-preserving `SnowedValue` tree. Use this when you need to distinguish Int64 from UInt64 on round-trips, or when the structure is not known at compile time.

### `snowed.dump(obj, fp)` / `snowed.load(fp)`

Streaming equivalents that write to / read from any file-like object (`io.IOBase`).

---

## Type Mapping

| Python type | Snowed wire type | Notes |
|---|---|---|
| `None` | Null (0x00) | |
| `bool` | Bool (0x02/0x03) | Checked before `int` — `bool` subclasses `int` |
| `int` | Int64 (0x04) | Default for all plain `int` values |
| `SnowedUInt64(n)` | UInt64 (0x05) | Explicit wrapper to force unsigned encoding |
| `float` | Float64 (0x08) | |
| `datetime.datetime` | DateTime (0x09) | Must be timezone-aware; encoded as UTC ms since epoch |
| `bytes` / `bytearray` | Binary (0x10) | |
| `str` | String (0x11) | |
| `decimal.Decimal` | Decimal (0x12) | Canonicalised per §5.9 |
| `uuid.UUID` | UUID (0x13) | `uuid.UUID.bytes` is RFC 4122 order; no swap needed |
| `list` / `tuple` | Array (0x40) | |
| `dict` | Object (0x80) | Keys must be `str` |
| `@dataclass` | Object (0x80) | Fields encoded by name; see field annotations below |
| `SnowedValue` | (dispatch) | Type-preserving value; written according to its subtype |

### Decoding (native mode — `loads`)

| Snowed wire type | Python type returned |
|---|---|
| Null | `None` |
| Bool | `bool` |
| Int64 | `int` |
| UInt64 | `SnowedUInt64` — preserves type for round-trips |
| Float64 | `float` |
| DateTime | `datetime.datetime` (UTC, timezone-aware) |
| Binary | `bytes` |
| String | `str` |
| Decimal | `decimal.Decimal` |
| UUID | `uuid.UUID` |
| Array | `list` |
| Object | `dict[str, ...]` |

---

## Integers: Int64 vs UInt64

All plain Python `int` values encode as Int64 (0x04) by default. To encode as UInt64 (0x05) use `SnowedUInt64`:

```python
from snowed import SnowedUInt64

# Dynamic encoding
data = snowed.dumps(SnowedUInt64(2**63))

# In a dataclass field, use the Annotated marker
from typing import Annotated
import snowed

@dataclass
class Counter:
    count: Annotated[int, snowed.uint64]  # encodes as UInt64
    offset: int                           # encodes as Int64 (default)
```

`SnowedUInt64` values decoded via `loads()` are returned as `SnowedUInt64` instances, preserving the wire type for re-encoding.

---

## Dataclasses

`@dataclass` instances encode as Snowed Objects. Field names are used as keys verbatim. Decode back into a dataclass with `snowed.loads_dataclass(data, MyClass)`.

```python
from dataclasses import dataclass, field
from typing import Annotated
import snowed

@dataclass
class Document:
    title: str
    revision: Annotated[int, snowed.uint64]
    tags: list[str] = field(default_factory=list)

doc = Document(title="Spec", revision=1, tags=["v1", "draft"])
data = snowed.dumps(doc)
back = snowed.loads_dataclass(data, Document)
```

Fields are encoded in declaration order. Fields with default values are only encoded if their value differs from the default (configurable via `encode_defaults`).

---

## Special Types

### `SnowedDecimal`

A thin wrapper over `str` for when you want to encode a decimal string directly without going through `decimal.Decimal`:

```python
snowed.dumps(SnowedDecimal("3.14159"))
```

### `SnowedUUID`

A wrapper over 16 raw bytes (RFC 4122 order) for low-level UUID handling:

```python
snowed.dumps(SnowedUUID(some_16_bytes))
```

### `SnowedDateTime`

A wrapper over a `float` milliseconds-since-epoch value for low-level DateTime handling:

```python
snowed.dumps(SnowedDateTime(1_700_000_000_000.0))
```

---

## SnowedValue

For dynamic / schema-less encoding and decoding:

```python
value = snowed.SnowedValue.Object({
    "name": snowed.SnowedValue.Str("Alice"),
    "score": snowed.SnowedValue.Int64(42),
})
data = snowed.dumps(value)
back = snowed.loads_value(data)
assert isinstance(back, snowed.SnowedValue.Object)
```

---

## Configuration

All encode and decode functions accept a `max_depth` keyword argument (default: 64). Decoding a structure nested deeper than `max_depth` raises `SnowedDecodeError`.

```python
data = snowed.dumps(obj, max_depth=128)
obj  = snowed.loads(data, max_depth=128)
obj  = snowed.loads_dataclass(data, MyClass, max_depth=128)
```

---

## Forward Compatibility

Unknown type bytes are rounded down to the nearest base type per §4.4 of the spec:

| Unknown byte range | Decoded as |
|---|---|
| 0x01 | Null |
| 0x06–0x07 | Int64 |
| 0x0A–0x0F | Float64 |
| 0x14–0x3F | Binary |
| 0x41–0x7F | Array element (sentinel still terminates) |
| 0x81–0xFE | Object |

---

## Running Tests

```bash
cd implementations/python
python3 -m venv .venv
.venv/bin/pip install pytest -q
.venv/bin/pytest
```
