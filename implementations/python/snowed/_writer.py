"""
Low-level Snowed binary writer.
"""
from __future__ import annotations

import struct

from ._types import (
    SnowedEncodeError,
    SnowedValue,
    _SVNull, _SVBool, _SVInt64, _SVUInt64, _SVFloat64, _SVDateTime,
    _SVBinary, _SVStr, _SVDecimal, _SVUuid, _SVArray, _SVObject,
)


class SnowedWriter:
    """Builds Snowed wire bytes into an in-memory buffer."""

    def __init__(self) -> None:
        self._buf = bytearray()

    def to_bytes(self) -> bytes:
        return bytes(self._buf)

    # ── Primitives ────────────────────────────────────────────────────────────

    def write_null(self) -> None:
        self._buf.append(0x00)

    def write_bool(self, value: bool) -> None:
        # Bool is a 1-byte type — the type byte IS the value (0x02=false, 0x03=true)
        self._buf.append(0x03 if value else 0x02)

    def write_int64(self, value: int) -> None:
        try:
            self._buf.append(0x04)
            self._buf += struct.pack("<q", value)
        except struct.error:
            raise SnowedEncodeError(
                f"Int64 value {value} is outside the signed 64-bit range"
            )

    def write_uint64(self, value: int) -> None:
        try:
            self._buf.append(0x05)
            self._buf += struct.pack("<Q", value)
        except struct.error:
            raise SnowedEncodeError(
                f"UInt64 value {value} is outside the unsigned 64-bit range"
            )

    def write_float64(self, value: float) -> None:
        self._buf.append(0x08)
        self._buf += struct.pack("<d", value)

    def write_datetime(self, millis: float) -> None:
        self._buf.append(0x09)
        self._buf += struct.pack("<d", millis)

    def write_binary(self, data: bytes) -> None:
        self._buf.append(0x10)
        self._buf += struct.pack("<Q", len(data))
        self._buf += data

    def write_string(self, value: str) -> None:
        encoded = value.encode("utf-8")
        self._buf.append(0x11)
        self._buf += struct.pack("<Q", len(encoded))
        self._buf += encoded

    def write_decimal(self, value: str) -> None:
        encoded = value.encode("utf-8")
        self._buf.append(0x12)
        self._buf += struct.pack("<Q", len(encoded))
        self._buf += encoded

    def write_uuid(self, data: bytes) -> None:
        if len(data) != 16:
            raise SnowedEncodeError(f"UUID must be exactly 16 bytes, got {len(data)}")
        self._buf.append(0x13)
        self._buf += struct.pack("<Q", 16)
        self._buf += data

    # ── Structures ────────────────────────────────────────────────────────────

    def write_array_begin(self) -> None:
        self._buf.append(0x40)

    def write_array_end(self) -> None:
        self._buf.append(0xFF)

    def write_object_begin(self) -> None:
        self._buf.append(0x80)

    def write_object_end(self) -> None:
        self._buf.append(0x00)  # zero-length key terminates the object

    def write_key(self, key: str) -> None:
        encoded = key.encode("utf-8")
        if len(encoded) > 255:
            raise SnowedEncodeError(
                f"Object key {key!r} exceeds the 255-byte UTF-8 limit "
                f"({len(encoded)} bytes)"
            )
        self._buf.append(len(encoded))
        self._buf += encoded

    # ── SnowedValue dispatch ──────────────────────────────────────────────────

    def write_value(self, value: SnowedValue) -> None:
        if isinstance(value, _SVNull):
            self.write_null()
        elif isinstance(value, _SVBool):
            self.write_bool(value.value)
        elif isinstance(value, _SVInt64):
            self.write_int64(value.value)
        elif isinstance(value, _SVUInt64):
            self.write_uint64(value.value)
        elif isinstance(value, _SVFloat64):
            self.write_float64(value.value)
        elif isinstance(value, _SVDateTime):
            self.write_datetime(value.millis)
        elif isinstance(value, _SVBinary):
            self.write_binary(value.data)
        elif isinstance(value, _SVStr):
            self.write_string(value.value)
        elif isinstance(value, _SVDecimal):
            self.write_decimal(value.value)
        elif isinstance(value, _SVUuid):
            self.write_uuid(value.bytes_)
        elif isinstance(value, _SVArray):
            self.write_array_begin()
            for elem in value.elements:
                self.write_value(elem)
            self.write_array_end()
        elif isinstance(value, _SVObject):
            self.write_object_begin()
            for k, v in value.entries.items():
                self.write_key(k)
                self.write_value(v)
            self.write_object_end()
        else:
            raise SnowedEncodeError(
                f"Cannot write unknown SnowedValue subtype {type(value).__name__}"
            )
