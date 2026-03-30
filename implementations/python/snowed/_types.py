"""
Special types and the SnowedValue tree for the Snowed binary format.
"""
from __future__ import annotations

from dataclasses import dataclass


# ── Encoding wrappers ─────────────────────────────────────────────────────────
# Used in dumps() / dump() to override the default Int64 encoding for plain int.

class SnowedUInt64:
    """
    Wraps an int to force encoding as Snowed UInt64 (0x05).

    All plain Python ``int`` values encode as Int64 (0x04) by default.
    Use this wrapper when the unsigned wire type must be preserved — for
    example, when interoperating with Go ``uint64`` or Kotlin ``ULong`` fields.

    Values decoded from a UInt64 wire value are returned as ``SnowedUInt64``
    instances so that round-trip fidelity is maintained.
    """

    __slots__ = ("value",)

    def __init__(self, value: int) -> None:
        if not isinstance(value, int) or isinstance(value, bool):
            raise TypeError(f"SnowedUInt64 requires int, got {type(value).__name__}")
        if not (0 <= value <= 0xFFFF_FFFF_FFFF_FFFF):
            raise ValueError(f"SnowedUInt64 value {value} is outside the uint64 range")
        self.value = value

    def __eq__(self, other: object) -> bool:
        return isinstance(other, SnowedUInt64) and self.value == other.value

    def __hash__(self) -> int:
        return hash(self.value)

    def __repr__(self) -> str:
        return f"SnowedUInt64({self.value!r})"


class SnowedDecimal:
    """
    Wraps a canonical decimal string for direct Snowed Decimal (0x12) encoding.

    The string must already conform to the canonical rules in §5.9: plain
    decimal notation, no exponent, no trailing zeros, negative zero → "0".
    To encode a ``decimal.Decimal`` object, pass it directly to ``dumps()``
    and the canonicalisation is applied automatically.
    """

    __slots__ = ("value",)

    def __init__(self, value: str) -> None:
        if not isinstance(value, str):
            raise TypeError(f"SnowedDecimal requires str, got {type(value).__name__}")
        self.value = value

    def __eq__(self, other: object) -> bool:
        return isinstance(other, SnowedDecimal) and self.value == other.value

    def __hash__(self) -> int:
        return hash(self.value)

    def __repr__(self) -> str:
        return f"SnowedDecimal({self.value!r})"


class SnowedUUID:
    """
    Wraps exactly 16 bytes in RFC 4122 byte order for Snowed UUID (0x13) encoding.

    To encode a ``uuid.UUID`` object, pass it directly to ``dumps()``.
    """

    __slots__ = ("bytes_",)

    def __init__(self, bytes_: bytes) -> None:
        if not isinstance(bytes_, (bytes, bytearray)):
            raise TypeError(f"SnowedUUID requires bytes, got {type(bytes_).__name__}")
        if len(bytes_) != 16:
            raise ValueError(f"SnowedUUID requires exactly 16 bytes, got {len(bytes_)}")
        self.bytes_ = bytes(bytes_)

    def __eq__(self, other: object) -> bool:
        return isinstance(other, SnowedUUID) and self.bytes_ == other.bytes_

    def __hash__(self) -> int:
        return hash(self.bytes_)

    def __repr__(self) -> str:
        return f"SnowedUUID({self.bytes_!r})"


class SnowedDateTime:
    """
    Wraps a float milliseconds-since-Unix-epoch value for direct Snowed
    DateTime (0x09) encoding.

    To encode a ``datetime.datetime``, pass it directly to ``dumps()``
    (it must be timezone-aware; it will be converted to UTC).
    """

    __slots__ = ("millis",)

    def __init__(self, millis: float) -> None:
        self.millis = float(millis)

    def __eq__(self, other: object) -> bool:
        return isinstance(other, SnowedDateTime) and self.millis == other.millis

    def __hash__(self) -> int:
        return hash(self.millis)

    def __repr__(self) -> str:
        return f"SnowedDateTime({self.millis!r})"


# ── Annotation marker ─────────────────────────────────────────────────────────

class _UInt64Marker:
    """Sentinel used with ``Annotated[int, snowed.uint64]`` on dataclass fields."""
    __slots__ = ()
    def __repr__(self) -> str:
        return "snowed.uint64"

uint64 = _UInt64Marker()


# ── SnowedValue tree ──────────────────────────────────────────────────────────
# Subclasses are defined at module level (Python cannot reference an enclosing
# class by name inside its own body), then attached as class attributes so that
# the ``SnowedValue.Null``, ``SnowedValue.Bool``, … namespace is preserved.

class SnowedValue:
    """
    Base class for the type-preserving Snowed value tree.

    Use ``snowed.loads_value(data)`` to decode into this tree, which
    preserves all wire-type distinctions (e.g. Int64 vs UInt64, String vs
    Decimal). Use ``snowed.dumps(value)`` to re-encode a ``SnowedValue``.

    Subclasses are accessible as ``SnowedValue.Null``, ``SnowedValue.Bool``,
    ``SnowedValue.Int64``, etc.
    """


@dataclass(eq=True, frozen=True)
class _SVNull(SnowedValue):
    """Snowed Null (0x00)."""


@dataclass(eq=True, frozen=True)
class _SVBool(SnowedValue):
    """Snowed Bool (0x02 / 0x03)."""
    value: bool


@dataclass(eq=True, frozen=True)
class _SVInt64(SnowedValue):
    """Snowed Int64 (0x04) — signed 64-bit integer."""
    value: int


@dataclass(eq=True, frozen=True)
class _SVUInt64(SnowedValue):
    """Snowed UInt64 (0x05) — unsigned 64-bit integer."""
    value: int


@dataclass(eq=True, frozen=True)
class _SVFloat64(SnowedValue):
    """Snowed Float64 (0x08) — IEEE 754 double."""
    value: float


@dataclass(eq=True, frozen=True)
class _SVDateTime(SnowedValue):
    """Snowed DateTime (0x09) — milliseconds since Unix epoch UTC."""
    millis: float


@dataclass(eq=True)
class _SVBinary(SnowedValue):
    """Snowed Binary (0x10) — raw bytes."""
    data: bytes

    def __eq__(self, other: object) -> bool:
        return isinstance(other, _SVBinary) and self.data == other.data

    def __hash__(self) -> int:
        return hash(self.data)


@dataclass(eq=True, frozen=True)
class _SVStr(SnowedValue):
    """Snowed String (0x11) — UTF-8."""
    value: str


@dataclass(eq=True, frozen=True)
class _SVDecimal(SnowedValue):
    """Snowed Decimal (0x12) — canonical UTF-8 decimal string."""
    value: str


@dataclass(eq=True)
class _SVUuid(SnowedValue):
    """Snowed UUID (0x13) — 16 bytes in RFC 4122 order."""
    bytes_: bytes

    def __eq__(self, other: object) -> bool:
        return isinstance(other, _SVUuid) and self.bytes_ == other.bytes_

    def __hash__(self) -> int:
        return hash(self.bytes_)


class _SVArray(SnowedValue):
    """Snowed Array (0x40) — ordered sequence of SnowedValues."""

    def __init__(self, elements: list[SnowedValue] | tuple[SnowedValue, ...]) -> None:
        self.elements: tuple[SnowedValue, ...] = tuple(elements)

    def __eq__(self, other: object) -> bool:
        return isinstance(other, _SVArray) and self.elements == other.elements

    def __hash__(self) -> int:
        return hash(self.elements)

    def __repr__(self) -> str:
        return f"SnowedValue.Array({list(self.elements)!r})"


class _SVObject(SnowedValue):
    """Snowed Object (0x80) — string-keyed map of SnowedValues."""

    def __init__(self, entries: dict[str, SnowedValue]) -> None:
        self.entries: dict[str, SnowedValue] = dict(entries)

    def __eq__(self, other: object) -> bool:
        return isinstance(other, _SVObject) and self.entries == other.entries

    def __hash__(self) -> int:
        return hash(tuple(sorted(self.entries.items())))

    def __repr__(self) -> str:
        return f"SnowedValue.Object({self.entries!r})"


# Attach subclasses as class attributes for the public ``SnowedValue.X`` namespace
SnowedValue.Null     = _SVNull      # type: ignore[attr-defined]
SnowedValue.Bool     = _SVBool      # type: ignore[attr-defined]
SnowedValue.Int64    = _SVInt64     # type: ignore[attr-defined]
SnowedValue.UInt64   = _SVUInt64    # type: ignore[attr-defined]
SnowedValue.Float64  = _SVFloat64   # type: ignore[attr-defined]
SnowedValue.DateTime = _SVDateTime  # type: ignore[attr-defined]
SnowedValue.Binary   = _SVBinary    # type: ignore[attr-defined]
SnowedValue.Str      = _SVStr       # type: ignore[attr-defined]
SnowedValue.Decimal  = _SVDecimal   # type: ignore[attr-defined]
SnowedValue.Uuid     = _SVUuid      # type: ignore[attr-defined]
SnowedValue.Array    = _SVArray     # type: ignore[attr-defined]
SnowedValue.Object   = _SVObject    # type: ignore[attr-defined]


# ── Exceptions ────────────────────────────────────────────────────────────────

class SnowedEncodeError(Exception):
    """Raised when a value cannot be encoded to Snowed wire format."""

class SnowedDecodeError(Exception):
    """Raised when Snowed wire bytes cannot be decoded."""
