"""
Snowed binary serialization format — Python implementation.

Quick start::

    import snowed

    data = snowed.dumps({"key": "value", "count": 42})
    obj  = snowed.loads(data)   # {"key": "value", "count": 42}

See README.md for the full API reference.
"""
from __future__ import annotations

import io
from typing import Any, TypeVar

from ._decoder import decode, decode_dataclass, decode_value
from ._encoder import encode
from ._reader import SnowedReader
from ._types import (
    SnowedDateTime,
    SnowedDecimal,
    SnowedDecodeError,
    SnowedEncodeError,
    SnowedUInt64,
    SnowedUUID,
    SnowedValue,
    uint64,
)
from ._writer import SnowedWriter

__all__ = [
    # Main API
    "dumps",
    "loads",
    "loads_value",
    "dump",
    "load",
    "loads_dataclass",
    # Special types
    "SnowedUInt64",
    "SnowedDecimal",
    "SnowedUUID",
    "SnowedDateTime",
    "SnowedValue",
    # Annotation marker
    "uint64",
    # Exceptions
    "SnowedEncodeError",
    "SnowedDecodeError",
]

T = TypeVar("T")


# ── Encoding ──────────────────────────────────────────────────────────────────

def dumps(obj: Any, *, max_depth: int = 64) -> bytes:
    """
    Encode *obj* to Snowed wire bytes.

    Supported types and their wire encodings:

    - ``None``                → Null (0x00)
    - ``bool``                → Bool (0x02/0x03)
    - ``int``                 → Int64 (0x04)
    - :class:`SnowedUInt64`   → UInt64 (0x05)
    - ``float``               → Float64 (0x08)
    - ``datetime.datetime``   → DateTime (0x09) — must be timezone-aware
    - ``bytes``/``bytearray`` → Binary (0x10)
    - ``str``                 → String (0x11)
    - ``decimal.Decimal``     → Decimal (0x12)
    - ``uuid.UUID``           → UUID (0x13)
    - ``list``/``tuple``      → Array (0x40)
    - ``dict``                → Object (0x80) — keys must be ``str``
    - ``@dataclass`` instance → Object (0x80)
    - :class:`SnowedValue`    → dispatched to the appropriate wire type

    :raises SnowedEncodeError: if *obj* cannot be encoded.
    """
    writer = SnowedWriter()
    encode(obj, writer, max_depth=max_depth)
    return writer.to_bytes()


def dump(obj: Any, fp: io.IOBase, *, max_depth: int = 64) -> None:
    """
    Encode *obj* and write the wire bytes to the file-like object *fp*.

    *fp* must support ``write(bytes)``.
    """
    fp.write(dumps(obj, max_depth=max_depth))  # type: ignore[arg-type]


# ── Decoding ──────────────────────────────────────────────────────────────────

def loads(
    data: bytes | bytearray | memoryview,
    *,
    max_depth: int = 64,
) -> Any:
    """
    Decode Snowed wire bytes to native Python types.

    Returns:

    - Null     → ``None``
    - Bool     → ``bool``
    - Int64    → ``int``
    - UInt64   → :class:`SnowedUInt64` (preserves wire type for re-encoding)
    - Float64  → ``float``
    - DateTime → ``datetime.datetime`` (UTC, timezone-aware)
    - Binary   → ``bytes``
    - String   → ``str``
    - Decimal  → ``decimal.Decimal``
    - UUID     → ``uuid.UUID``
    - Array    → ``list``
    - Object   → ``dict[str, ...]``

    :raises SnowedDecodeError: if *data* is not valid Snowed.
    :raises SnowedDecodeError: if there are trailing bytes after the root value.
    """
    reader = SnowedReader(bytes(data), max_depth=max_depth)
    value = decode(reader)
    if not reader.is_exhausted():
        raise SnowedDecodeError(
            f"Trailing bytes after root value ({reader.remaining} byte(s) remaining)"
        )
    return value


def loads_value(
    data: bytes | bytearray | memoryview,
    *,
    max_depth: int = 64,
) -> SnowedValue:
    """
    Decode Snowed wire bytes to a type-preserving :class:`SnowedValue` tree.

    Unlike :func:`loads`, this preserves all wire-type distinctions — for
    example, Int64 vs UInt64, String vs Decimal vs UUID.

    :raises SnowedDecodeError: if *data* is not valid Snowed.
    """
    reader = SnowedReader(bytes(data), max_depth=max_depth)
    value = decode_value(reader)
    if not reader.is_exhausted():
        raise SnowedDecodeError(
            f"Trailing bytes after root value ({reader.remaining} byte(s) remaining)"
        )
    return value


def loads_dataclass(
    data: bytes | bytearray | memoryview,
    cls: type[T],
    *,
    max_depth: int = 64,
) -> T:
    """
    Decode Snowed wire bytes directly into an instance of the ``@dataclass`` *cls*.

    - Fields present in the wire data are assigned to the dataclass.
    - Fields absent in the wire data use their dataclass default values.
    - Unknown keys in the wire data are ignored.
    - ``Annotated[int, snowed.uint64]`` fields are decoded from UInt64 wire
      values and returned as plain ``int``.

    :raises SnowedDecodeError: if *data* is not valid Snowed or the dataclass
        cannot be constructed from the decoded fields.
    """
    reader = SnowedReader(bytes(data), max_depth=max_depth)
    value = decode_dataclass(reader, cls)
    if not reader.is_exhausted():
        raise SnowedDecodeError(
            f"Trailing bytes after root value ({reader.remaining} byte(s) remaining)"
        )
    return value


def load(fp: io.IOBase, *, max_depth: int = 64) -> Any:
    """
    Read Snowed wire bytes from the file-like object *fp* and decode to
    native Python types.

    *fp* must support ``read() -> bytes``.
    """
    return loads(fp.read(), max_depth=max_depth)  # type: ignore[arg-type]
