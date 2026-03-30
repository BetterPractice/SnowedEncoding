"""
Snowed decoder: wire bytes → Python values.
"""
from __future__ import annotations

import dataclasses
import datetime
import decimal
import struct
import uuid
from typing import Annotated, Any, TypeVar, get_args, get_origin, get_type_hints

from ._reader import SnowedReader
from ._types import (
    SnowedDecodeError,
    SnowedUInt64,
    SnowedValue,
    _SVNull, _SVBool, _SVInt64, _SVUInt64, _SVFloat64, _SVDateTime,
    _SVBinary, _SVStr, _SVDecimal, _SVUuid, _SVArray, _SVObject,
    _UInt64Marker,
)

T = TypeVar("T")

# ── Native decode (returns Python built-in types) ─────────────────────────────

def decode(reader: SnowedReader, *, depth: int = 0) -> Any:
    """Decode one value from *reader*, returning native Python types."""
    type_byte = reader.read_type_byte()
    return _decode_with_type(type_byte, reader, depth=depth)


def _decode_with_type(type_byte: int, reader: SnowedReader, *, depth: int) -> Any:
    base = SnowedReader.base_type(type_byte)

    if base == 0x00:  # Null family
        return None

    if base == 0x04:  # Int64 family
        if type_byte == 0x02:
            return False  # Bool false — type byte only, no payload
        if type_byte == 0x03:
            return True   # Bool true — type byte only, no payload
        if type_byte == 0x05:  # UInt64 — return wrapper to preserve round-trip type
            return SnowedUInt64(reader.read_le64_unsigned())
        # 0x04 and unknown Int subtypes (0x06, 0x07)
        return reader.read_le64_signed()

    if base == 0x08:  # Float64 family
        raw = reader.read_le64_unsigned()
        value = struct.unpack("<d", struct.pack("<Q", raw))[0]
        if type_byte == 0x09:  # DateTime
            return datetime.datetime.fromtimestamp(
                value / 1000.0, tz=datetime.timezone.utc
            )
        return value

    if base == 0x10:  # Binary family
        data = reader.read_length_prefixed_bytes()
        if type_byte == 0x11:  # String
            try:
                return data.decode("utf-8")
            except UnicodeDecodeError as e:
                raise SnowedDecodeError(f"Invalid UTF-8 in String value: {e}") from e
        if type_byte == 0x12:  # Decimal
            try:
                s = data.decode("utf-8")
            except UnicodeDecodeError as e:
                raise SnowedDecodeError(f"Invalid UTF-8 in Decimal value: {e}") from e
            try:
                return decimal.Decimal(s)
            except decimal.InvalidOperation as e:
                raise SnowedDecodeError(f"Invalid Decimal string {s!r}: {e}") from e
        if type_byte == 0x13:  # UUID
            if len(data) != 16:
                raise SnowedDecodeError(
                    f"UUID payload must be 16 bytes, got {len(data)}"
                )
            return uuid.UUID(bytes=data)
        # unknown Binary subtype → raw bytes
        return data

    if base == 0x40:  # Array family
        if depth >= reader.max_depth:
            raise SnowedDecodeError(
                f"Maximum nesting depth {reader.max_depth} exceeded"
            )
        result: list[Any] = []
        while True:
            peeked = reader.peek_byte()
            if peeked == 0xFF:
                reader.read_byte()  # consume sentinel
                break
            item = decode(reader, depth=depth + 1)
            result.append(item)
        return result

    if base == 0x80:  # Object family
        if depth >= reader.max_depth:
            raise SnowedDecodeError(
                f"Maximum nesting depth {reader.max_depth} exceeded"
            )
        result_dict: dict[str, Any] = {}
        while True:
            key_len = reader.read_byte()
            if key_len == 0x00:
                break
            key_bytes = reader.read_bytes(key_len)
            try:
                key = key_bytes.decode("utf-8")
            except UnicodeDecodeError as e:
                raise SnowedDecodeError(f"Invalid UTF-8 in object key: {e}") from e
            vtype = reader.read_type_byte()
            if vtype == 0xFF:  # sentinel in value position ends the object
                break
            value = _decode_with_type(vtype, reader, depth=depth + 1)
            result_dict[key] = value
        return result_dict

    if type_byte == 0xFF:
        raise SnowedDecodeError(
            "Unexpected sentinel byte (0xFF) in value position"
        )

    raise SnowedDecodeError(f"Unknown type byte 0x{type_byte:02x}")


# ── SnowedValue decode (type-preserving) ─────────────────────────────────────

def decode_value(reader: SnowedReader, *, depth: int = 0) -> SnowedValue:
    """Decode one value from *reader*, returning a type-preserving :class:`SnowedValue`."""
    type_byte = reader.read_type_byte()
    return _decode_value_with_type(type_byte, reader, depth=depth)


def _decode_value_with_type(
    type_byte: int, reader: SnowedReader, *, depth: int
) -> SnowedValue:
    base = SnowedReader.base_type(type_byte)

    if base == 0x00:
        return SnowedValue.Null()

    if base == 0x04:
        if type_byte == 0x02:
            return _SVBool(False)
        if type_byte == 0x03:
            return _SVBool(True)
        if type_byte == 0x05:
            return _SVUInt64(reader.read_le64_unsigned())
        return _SVInt64(reader.read_le64_signed())

    if base == 0x08:
        raw = reader.read_le64_unsigned()
        value = struct.unpack("<d", struct.pack("<Q", raw))[0]
        if type_byte == 0x09:
            return _SVDateTime(value)
        return _SVFloat64(value)

    if base == 0x10:
        data = reader.read_length_prefixed_bytes()
        if type_byte == 0x11:
            try:
                return _SVStr(data.decode("utf-8"))
            except UnicodeDecodeError as e:
                raise SnowedDecodeError(f"Invalid UTF-8 in String: {e}") from e
        if type_byte == 0x12:
            try:
                return _SVDecimal(data.decode("utf-8"))
            except UnicodeDecodeError as e:
                raise SnowedDecodeError(f"Invalid UTF-8 in Decimal: {e}") from e
        if type_byte == 0x13:
            if len(data) != 16:
                raise SnowedDecodeError(
                    f"UUID payload must be 16 bytes, got {len(data)}"
                )
            return _SVUuid(data)
        return _SVBinary(data)

    if base == 0x40:
        if depth >= reader.max_depth:
            raise SnowedDecodeError(
                f"Maximum nesting depth {reader.max_depth} exceeded"
            )
        elements: list[SnowedValue] = []
        while True:
            peeked = reader.peek_byte()
            if peeked == 0xFF:
                reader.read_byte()
                break
            elements.append(decode_value(reader, depth=depth + 1))
        return _SVArray(elements)

    if base == 0x80:
        if depth >= reader.max_depth:
            raise SnowedDecodeError(
                f"Maximum nesting depth {reader.max_depth} exceeded"
            )
        entries: dict[str, SnowedValue] = {}
        while True:
            key_len = reader.read_byte()
            if key_len == 0x00:
                break
            key_bytes = reader.read_bytes(key_len)
            try:
                key = key_bytes.decode("utf-8")
            except UnicodeDecodeError as e:
                raise SnowedDecodeError(f"Invalid UTF-8 in object key: {e}") from e
            vtype = reader.read_type_byte()
            if vtype == 0xFF:
                break
            entries[key] = _decode_value_with_type(vtype, reader, depth=depth + 1)
        return _SVObject(entries)

    if type_byte == 0xFF:
        raise SnowedDecodeError("Unexpected sentinel (0xFF) in value position")

    raise SnowedDecodeError(f"Unknown type byte 0x{type_byte:02x}")


# ── Dataclass decode ──────────────────────────────────────────────────────────

def decode_dataclass(reader: SnowedReader, cls: type[T], *, depth: int = 0) -> T:
    """
    Decode a Snowed Object into an instance of the ``@dataclass`` *cls*.

    - Unknown keys in the wire data are ignored by default.
    - Fields absent in the wire data use their dataclass default values.
    """
    if not dataclasses.is_dataclass(cls):
        raise SnowedDecodeError(f"{cls.__name__} is not a dataclass")

    type_byte = reader.read_type_byte()
    if SnowedReader.base_type(type_byte) != 0x80:
        raise SnowedDecodeError(
            f"Expected Object (0x80) for dataclass, got 0x{type_byte:02x}"
        )

    if depth >= reader.max_depth:
        raise SnowedDecodeError(
            f"Maximum nesting depth {reader.max_depth} exceeded"
        )

    # Read all key-value pairs into a raw dict first (handles field reordering)
    raw: dict[str, Any] = {}
    while True:
        key_len = reader.read_byte()
        if key_len == 0x00:
            break
        key_bytes = reader.read_bytes(key_len)
        try:
            key = key_bytes.decode("utf-8")
        except UnicodeDecodeError as e:
            raise SnowedDecodeError(f"Invalid UTF-8 in object key: {e}") from e
        vtype = reader.read_type_byte()
        if vtype == 0xFF:
            break
        raw[key] = _decode_with_type(vtype, reader, depth=depth + 1)

    # Resolve type hints (including Annotated) to handle uint64 fields
    try:
        hints = get_type_hints(cls, include_extras=True)
    except Exception:
        hints = {}

    fields = dataclasses.fields(cls)  # type: ignore[arg-type]
    kwargs: dict[str, Any] = {}

    for f in fields:
        if f.name in raw:
            value = raw[f.name]
            hint = hints.get(f.name)
            # For Annotated[int, snowed.uint64] fields, unwrap SnowedUInt64 → int
            if _is_uint64_annotated(hint) and isinstance(value, SnowedUInt64):
                value = value.value
            kwargs[f.name] = value
        elif f.default is not dataclasses.MISSING:
            kwargs[f.name] = f.default
        elif f.default_factory is not dataclasses.MISSING:  # type: ignore[misc]
            kwargs[f.name] = f.default_factory()  # type: ignore[misc]
        # else: missing required field — let the dataclass constructor raise

    try:
        return cls(**kwargs)  # type: ignore[return-value]
    except TypeError as e:
        raise SnowedDecodeError(
            f"Failed to construct {cls.__name__}: {e}"
        ) from e


def _is_uint64_annotated(hint: object) -> bool:
    if get_origin(hint) is Annotated:
        return any(isinstance(a, _UInt64Marker) for a in get_args(hint)[1:])
    return False
