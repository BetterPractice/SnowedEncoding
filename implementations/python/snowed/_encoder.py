"""
Snowed encoder: Python values → wire bytes.
"""
from __future__ import annotations

import dataclasses
import datetime
import decimal
import uuid
from typing import Annotated, get_args, get_origin, get_type_hints

from ._types import (
    SnowedDateTime,
    SnowedDecimal,
    SnowedEncodeError,
    SnowedUInt64,
    SnowedUUID,
    SnowedValue,
    _UInt64Marker,
)
from ._writer import SnowedWriter


def encode(obj: object, writer: SnowedWriter, *, depth: int = 0, max_depth: int = 64) -> None:
    """Encode *obj* into *writer*, raising :exc:`SnowedEncodeError` on failure."""
    if depth > max_depth:
        raise SnowedEncodeError(f"Maximum encoding depth {max_depth} exceeded")

    # None
    if obj is None:
        writer.write_null()

    # bool MUST come before int — bool is a subclass of int in Python
    elif isinstance(obj, bool):
        writer.write_bool(obj)

    # Explicit UInt64 wrapper
    elif isinstance(obj, SnowedUInt64):
        writer.write_uint64(obj.value)

    # Plain int → Int64 by default
    elif isinstance(obj, int):
        writer.write_int64(obj)

    elif isinstance(obj, float):
        writer.write_float64(obj)

    # Special Snowed wrappers
    elif isinstance(obj, SnowedDateTime):
        writer.write_datetime(obj.millis)

    elif isinstance(obj, SnowedDecimal):
        writer.write_decimal(obj.value)

    elif isinstance(obj, SnowedUUID):
        writer.write_uuid(obj.bytes_)

    # Type-preserving SnowedValue tree
    elif isinstance(obj, SnowedValue):
        writer.write_value(obj)

    # Standard library types with natural Snowed mappings
    elif isinstance(obj, datetime.datetime):
        _encode_datetime(obj, writer)

    elif isinstance(obj, decimal.Decimal):
        writer.write_decimal(_canonical_decimal(obj))

    elif isinstance(obj, uuid.UUID):
        # uuid.UUID.bytes is already in RFC 4122 (big-endian) byte order
        writer.write_uuid(obj.bytes)

    elif isinstance(obj, (bytes, bytearray, memoryview)):
        writer.write_binary(bytes(obj))

    elif isinstance(obj, str):
        writer.write_string(obj)

    elif isinstance(obj, (list, tuple)):
        writer.write_array_begin()
        for item in obj:
            encode(item, writer, depth=depth + 1, max_depth=max_depth)
        writer.write_array_end()

    elif isinstance(obj, dict):
        writer.write_object_begin()
        for k, v in obj.items():
            if not isinstance(k, str):
                raise SnowedEncodeError(
                    f"Object keys must be str, got {type(k).__name__}"
                )
            writer.write_key(k)
            encode(v, writer, depth=depth + 1, max_depth=max_depth)
        writer.write_object_end()

    elif dataclasses.is_dataclass(obj) and not isinstance(obj, type):
        _encode_dataclass(obj, writer, depth=depth, max_depth=max_depth)

    else:
        raise SnowedEncodeError(
            f"Cannot encode value of type {type(obj).__name__!r}"
        )


# ── Helpers ───────────────────────────────────────────────────────────────────

def _encode_datetime(dt: datetime.datetime, writer: SnowedWriter) -> None:
    if dt.tzinfo is None:
        raise SnowedEncodeError(
            "Cannot encode a naive datetime as Snowed DateTime. "
            "Attach timezone info (e.g. datetime.timezone.utc) or use SnowedDateTime."
        )
    utc = dt.astimezone(datetime.timezone.utc)
    millis = utc.timestamp() * 1000.0
    writer.write_datetime(millis)


def _encode_dataclass(
    obj: object, writer: SnowedWriter, *, depth: int, max_depth: int
) -> None:
    try:
        hints = get_type_hints(type(obj), include_extras=True)
    except Exception:
        hints = {}

    writer.write_object_begin()
    for f in dataclasses.fields(obj):  # type: ignore[arg-type]
        value = getattr(obj, f.name)
        hint = hints.get(f.name)
        writer.write_key(f.name)
        if _is_uint64_annotated(hint):
            if not isinstance(value, int) or isinstance(value, bool):
                raise SnowedEncodeError(
                    f"Field {f.name!r} is annotated as uint64 but holds {type(value).__name__!r}"
                )
            writer.write_uint64(value)
        else:
            encode(value, writer, depth=depth + 1, max_depth=max_depth)
    writer.write_object_end()


def _is_uint64_annotated(hint: object) -> bool:
    """Return True if *hint* is ``Annotated[int, snowed.uint64]``."""
    if get_origin(hint) is Annotated:
        return any(isinstance(a, _UInt64Marker) for a in get_args(hint)[1:])
    return False


def _canonical_decimal(value: decimal.Decimal) -> str:
    """
    Convert a ``decimal.Decimal`` to a canonical Snowed Decimal string per §5.9:
    plain notation, no exponent, no trailing zeros, negative zero → "0".
    """
    if value.is_nan() or value.is_infinite():
        raise SnowedEncodeError(
            f"Cannot encode {value} as Snowed Decimal (NaN and Infinity are not supported)"
        )
    if value.is_zero():
        return "0"

    sign, digits, exp = value.normalize().as_tuple()
    digit_str = "".join(str(d) for d in digits)

    if exp >= 0:
        # e.g. digits=(1,2,3), exp=2  → "12300"
        result = digit_str + "0" * exp
    else:
        abs_exp = -exp
        if abs_exp >= len(digit_str):
            # e.g. digits=(1,), exp=-3  → "0.001"
            result = "0." + "0" * (abs_exp - len(digit_str)) + digit_str
        else:
            # e.g. digits=(3,1,4), exp=-2  → "3.14"
            result = digit_str[:-abs_exp] + "." + digit_str[-abs_exp:]

    return ("-" + result) if sign else result
