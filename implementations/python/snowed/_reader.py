"""
Low-level Snowed binary reader.
"""
from __future__ import annotations

import struct

from ._types import SnowedDecodeError


class SnowedReader:
    """Reads Snowed wire bytes from a ``bytes`` buffer."""

    def __init__(self, data: bytes, max_depth: int = 64) -> None:
        self._data = memoryview(data)
        self._pos = 0
        self.max_depth = max_depth

    # ── Position helpers ──────────────────────────────────────────────────────

    @property
    def remaining(self) -> int:
        return len(self._data) - self._pos

    def is_exhausted(self) -> bool:
        return self._pos >= len(self._data)

    # ── Byte-level reads ──────────────────────────────────────────────────────

    def read_byte(self) -> int:
        if self._pos >= len(self._data):
            raise SnowedDecodeError("Unexpected end of input")
        b = self._data[self._pos]
        self._pos += 1
        return b

    def peek_byte(self) -> int:
        if self._pos >= len(self._data):
            raise SnowedDecodeError("Unexpected end of input")
        return self._data[self._pos]

    def read_bytes(self, n: int) -> bytes:
        if self._pos + n > len(self._data):
            raise SnowedDecodeError(
                f"Unexpected end of input: need {n} bytes, have {self.remaining}"
            )
        result = bytes(self._data[self._pos : self._pos + n])
        self._pos += n
        return result

    def read_type_byte(self) -> int:
        return self.read_byte()

    # ── Numeric reads ─────────────────────────────────────────────────────────

    def read_le64_unsigned(self) -> int:
        """Read 8 little-endian bytes and return as an unsigned Python int."""
        raw = self.read_bytes(8)
        return struct.unpack("<Q", raw)[0]

    def read_le64_signed(self) -> int:
        """Read 8 little-endian bytes and return as a signed Python int."""
        raw = self.read_bytes(8)
        return struct.unpack("<q", raw)[0]

    def read_length_prefixed_bytes(self) -> bytes:
        """Read an 8-byte LE length prefix followed by that many bytes."""
        length = self.read_le64_unsigned()
        return self.read_bytes(length)

    # ── Forward-compatibility helper ──────────────────────────────────────────

    @staticmethod
    def base_type(b: int) -> int:
        """
        Round an unknown type byte down to the nearest base type, per §4.4.

        Base type ranges:
          0x00–0x01 → Null  (0x00)
          0x02–0x07 → Int64 (0x04)
          0x08–0x0F → Float64 (0x08)
          0x10–0x3F → Binary (0x10)
          0x40–0x7F → Array  (0x40)
          0x80–0xFE → Object (0x80)
          0xFF      → Sentinel
        """
        if b <= 0x01:
            return 0x00
        if b <= 0x07:
            return 0x04
        if b <= 0x0F:
            return 0x08
        if b <= 0x3F:
            return 0x10
        if b <= 0x7F:
            return 0x40
        if b <= 0xFE:
            return 0x80
        return 0xFF
