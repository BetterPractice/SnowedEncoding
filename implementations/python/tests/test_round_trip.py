"""
Snowed Python implementation tests.

Covers wire format correctness, round-trips for all types,
dataclass encoding/decoding, special types, uint64 annotation,
SnowedValue, forward compatibility, and error cases.
"""
from __future__ import annotations

import datetime
import decimal
import io
import math
import struct
import uuid
from dataclasses import dataclass, field
from typing import Annotated

import pytest

import snowed
from snowed import (
    SnowedDateTime,
    SnowedDecimal,
    SnowedDecodeError,
    SnowedEncodeError,
    SnowedUInt64,
    SnowedUUID,
    SnowedValue,
    uint64,
)


# ── Helpers ───────────────────────────────────────────────────────────────────

def rt(value):
    """Native round-trip: dumps → loads."""
    return snowed.loads(snowed.dumps(value))


def rt_value(value: SnowedValue) -> SnowedValue:
    """SnowedValue round-trip."""
    return snowed.loads_value(snowed.dumps(value))


def wire(hex_str: str) -> bytes:
    """Convert a hex string (space-separated) to bytes."""
    return bytes(int(b, 16) for b in hex_str.split())


# ── Wire format correctness ───────────────────────────────────────────────────

class TestWireFormat:

    def test_null(self):
        assert snowed.dumps(None) == wire("00")

    def test_bool_false(self):
        assert snowed.dumps(False) == wire("02")

    def test_bool_true(self):
        assert snowed.dumps(True) == wire("03")

    def test_int64_zero(self):
        assert snowed.dumps(0) == wire("04") + b"\x00" * 8

    def test_int64_one(self):
        assert snowed.dumps(1) == wire("04") + b"\x01" + b"\x00" * 7

    def test_int64_negative(self):
        # -1 is 0xFFFFFFFFFFFFFFFF in little-endian two's complement
        assert snowed.dumps(-1) == wire("04") + b"\xff" * 8

    def test_uint64_zero(self):
        assert snowed.dumps(SnowedUInt64(0)) == wire("05") + b"\x00" * 8

    def test_float64(self):
        expected = wire("08") + struct.pack("<d", 3.14)
        assert snowed.dumps(3.14) == expected

    def test_datetime_epoch(self):
        expected = wire("09") + struct.pack("<d", 0.0)
        assert snowed.dumps(SnowedDateTime(0.0)) == expected

    def test_binary_empty(self):
        assert snowed.dumps(b"") == wire("10") + b"\x00" * 8

    def test_binary_payload(self):
        payload = b"\xde\xad\xbe\xef"
        expected = wire("10") + struct.pack("<Q", 4) + payload
        assert snowed.dumps(payload) == expected

    def test_string_empty(self):
        assert snowed.dumps("") == wire("11") + b"\x00" * 8

    def test_string_hello(self):
        expected = wire("11") + struct.pack("<Q", 5) + b"hello"
        assert snowed.dumps("hello") == expected

    def test_decimal(self):
        expected = wire("12") + struct.pack("<Q", 4) + b"3.14"
        assert snowed.dumps(SnowedDecimal("3.14")) == expected

    def test_uuid(self):
        raw = bytes(range(16))
        expected = wire("13") + struct.pack("<Q", 16) + raw
        assert snowed.dumps(SnowedUUID(raw)) == expected

    def test_array_empty(self):
        assert snowed.dumps([]) == wire("40 ff")

    def test_array_one_element(self):
        assert snowed.dumps([None]) == wire("40 00 ff")

    def test_object_empty(self):
        assert snowed.dumps({}) == wire("80 00")

    def test_object_single_key(self):
        data = snowed.dumps({"x": None})
        assert data == wire("80") + b"\x01x\x00" + wire("00")


# ── Null ──────────────────────────────────────────────────────────────────────

class TestNull:
    def test_round_trip(self):
        assert rt(None) is None

    def test_snowed_value(self):
        assert rt_value(SnowedValue.Null()) == SnowedValue.Null()


# ── Bool ──────────────────────────────────────────────────────────────────────

class TestBool:
    def test_false(self):
        assert rt(False) is False

    def test_true(self):
        assert rt(True) is True

    def test_bool_not_encoded_as_int(self):
        # bool is a subclass of int; must be encoded as Bool, not Int64
        assert snowed.dumps(True)[0] == 0x03
        assert snowed.dumps(False)[0] == 0x02

    def test_snowed_value_false(self):
        assert rt_value(SnowedValue.Bool(False)) == SnowedValue.Bool(False)

    def test_snowed_value_true(self):
        assert rt_value(SnowedValue.Bool(True)) == SnowedValue.Bool(True)


# ── Int64 ─────────────────────────────────────────────────────────────────────

class TestInt64:
    def test_zero(self):    assert rt(0) == 0
    def test_positive(self): assert rt(42) == 42
    def test_negative(self): assert rt(-1) == -1
    def test_min(self):     assert rt(-(2**63)) == -(2**63)
    def test_max(self):     assert rt(2**63 - 1) == 2**63 - 1

    def test_overflow_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps(2**63)  # exceeds Int64 max; use SnowedUInt64

    def test_underflow_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps(-(2**63) - 1)

    def test_snowed_value(self):
        assert rt_value(SnowedValue.Int64(-99)) == SnowedValue.Int64(-99)


# ── UInt64 ────────────────────────────────────────────────────────────────────

class TestUInt64:
    def test_zero(self):
        assert rt(SnowedUInt64(0)) == SnowedUInt64(0)

    def test_max(self):
        assert rt(SnowedUInt64(2**64 - 1)) == SnowedUInt64(2**64 - 1)

    def test_above_int64_max(self):
        v = SnowedUInt64(2**63)
        assert rt(v) == v

    def test_wire_type_preserved(self):
        # Decoded value must be SnowedUInt64, not plain int
        result = rt(SnowedUInt64(1))
        assert isinstance(result, SnowedUInt64)
        assert result.value == 1

    def test_snowed_value(self):
        assert rt_value(SnowedValue.UInt64(2**63)) == SnowedValue.UInt64(2**63)

    def test_invalid_type_raises(self):
        with pytest.raises(TypeError):
            SnowedUInt64(3.14)  # type: ignore

    def test_out_of_range_raises(self):
        with pytest.raises(ValueError):
            SnowedUInt64(2**64)

    def test_negative_raises(self):
        with pytest.raises(ValueError):
            SnowedUInt64(-1)


# ── Float64 ───────────────────────────────────────────────────────────────────

class TestFloat64:
    def test_pi(self):          assert rt(3.14) == 3.14
    def test_negative(self):    assert rt(-2.718) == -2.718
    def test_zero(self):        assert rt(0.0) == 0.0
    def test_negative_zero(self):
        result = rt(-0.0)
        assert result == 0.0
        assert math.copysign(1.0, result) == -1.0  # sign preserved

    def test_infinity(self):    assert rt(math.inf) == math.inf
    def test_neg_infinity(self): assert rt(-math.inf) == -math.inf
    def test_nan(self):
        result = rt(math.nan)
        assert math.isnan(result)

    def test_snowed_value(self):
        assert rt_value(SnowedValue.Float64(1.5)) == SnowedValue.Float64(1.5)


# ── DateTime ──────────────────────────────────────────────────────────────────

class TestDateTime:
    UTC = datetime.timezone.utc

    def test_epoch(self):
        dt = datetime.datetime(1970, 1, 1, tzinfo=self.UTC)
        assert rt(dt) == dt

    def test_modern(self):
        dt = datetime.datetime.fromtimestamp(1_700_000_000, tz=self.UTC)
        result = rt(dt)
        assert result == dt
        assert result.tzinfo == self.UTC

    def test_pre_epoch(self):
        dt = datetime.datetime(1960, 6, 15, tzinfo=self.UTC)
        assert rt(dt) == dt

    def test_non_utc_normalised(self):
        # A non-UTC datetime is normalised to UTC on encode/decode
        tz = datetime.timezone(datetime.timedelta(hours=5))
        dt = datetime.datetime(2024, 1, 1, 12, 0, tzinfo=tz)
        result = rt(dt)
        expected = dt.astimezone(self.UTC).replace(tzinfo=self.UTC)
        assert result == expected

    def test_naive_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps(datetime.datetime(2024, 1, 1))  # no tzinfo

    def test_snowed_datetime_wrapper(self):
        v = SnowedDateTime(1_700_000_000_000.0)
        assert rt(v) == datetime.datetime.fromtimestamp(1_700_000_000.0, tz=self.UTC)

    def test_snowed_value(self):
        v = SnowedValue.DateTime(0.0)
        assert rt_value(v) == v


# ── Binary ────────────────────────────────────────────────────────────────────

class TestBinary:
    def test_empty(self):     assert rt(b"") == b""
    def test_payload(self):   assert rt(b"\xde\xad\xbe\xef") == b"\xde\xad\xbe\xef"
    def test_bytearray(self):
        result = rt(bytearray(b"abc"))
        assert result == b"abc"
        assert isinstance(result, bytes)

    def test_snowed_value(self):
        v = SnowedValue.Binary(b"\x01\x02")
        assert rt_value(v) == v


# ── String ────────────────────────────────────────────────────────────────────

class TestString:
    def test_empty(self):     assert rt("") == ""
    def test_ascii(self):     assert rt("hello") == "hello"
    def test_unicode(self):   assert rt("こんにちは🌨️") == "こんにちは🌨️"

    def test_snowed_value(self):
        assert rt_value(SnowedValue.Str("world")) == SnowedValue.Str("world")


# ── Decimal ───────────────────────────────────────────────────────────────────

class TestDecimal:

    def test_integer(self):
        assert rt(decimal.Decimal("42")) == decimal.Decimal("42")

    def test_fractional(self):
        assert rt(decimal.Decimal("3.14159")) == decimal.Decimal("3.14159")

    def test_negative(self):
        assert rt(decimal.Decimal("-7")) == decimal.Decimal("-7")

    def test_trailing_zeros_stripped(self):
        # 3.10 encodes as "3.1" (canonical); decodes back to Decimal("3.1")
        result = rt(decimal.Decimal("3.10"))
        assert result == decimal.Decimal("3.1")

    def test_zero(self):
        assert rt(decimal.Decimal("0")) == decimal.Decimal("0")

    def test_negative_zero_normalised(self):
        result = rt(decimal.Decimal("-0"))
        assert result == decimal.Decimal("0")
        assert str(result) == "0"

    def test_small(self):
        assert rt(decimal.Decimal("0.001")) == decimal.Decimal("0.001")

    def test_large_integer(self):
        assert rt(decimal.Decimal("1000000")) == decimal.Decimal("1000000")

    def test_nan_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps(decimal.Decimal("NaN"))

    def test_infinity_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps(decimal.Decimal("Infinity"))

    def test_snowed_decimal_wrapper(self):
        data = snowed.dumps(SnowedDecimal("2.71828"))
        result = snowed.loads(data)
        assert result == decimal.Decimal("2.71828")

    def test_snowed_value(self):
        v = SnowedValue.Decimal("99.9")
        assert rt_value(v) == v


# ── UUID ──────────────────────────────────────────────────────────────────────

class TestUUID:

    def test_round_trip(self):
        u = uuid.UUID("550e8400-e29b-41d4-a716-446655440000")
        assert rt(u) == u

    def test_bytes_are_rfc4122(self):
        u = uuid.UUID("550e8400-e29b-41d4-a716-446655440000")
        data = snowed.dumps(u)
        # 0x13 + 8-byte LE length (16) + 16 UUID bytes
        assert data[0] == 0x13
        assert struct.unpack("<Q", data[1:9])[0] == 16
        assert data[9:] == u.bytes  # RFC 4122 order

    def test_snowed_uuid_wrapper(self):
        raw = bytes(range(16))
        data = snowed.dumps(SnowedUUID(raw))
        result = snowed.loads(data)
        assert isinstance(result, uuid.UUID)
        assert result.bytes == raw

    def test_snowed_value(self):
        raw = bytes(range(16))
        v = SnowedValue.Uuid(raw)
        back = rt_value(v)
        assert isinstance(back, SnowedValue.Uuid)
        assert back.bytes_ == raw


# ── Array ─────────────────────────────────────────────────────────────────────

class TestArray:
    def test_empty(self):     assert rt([]) == []
    def test_single(self):    assert rt([42]) == [42]
    def test_mixed(self):
        v = [None, True, 1, 3.14, "hi", b"\xff"]
        assert rt(v) == v

    def test_nested(self):
        v = [[1, 2], [3, [4, 5]]]
        assert rt(v) == v

    def test_tuple_encodes_as_array(self):
        result = rt((1, 2, 3))
        assert result == [1, 2, 3]
        assert isinstance(result, list)

    def test_snowed_value_empty(self):
        assert rt_value(SnowedValue.Array([])) == SnowedValue.Array([])

    def test_snowed_value_mixed(self):
        v = SnowedValue.Array([
            SnowedValue.Null(),
            SnowedValue.Int64(7),
            SnowedValue.Str("x"),
        ])
        assert rt_value(v) == v


# ── Object ────────────────────────────────────────────────────────────────────

class TestObject:
    def test_empty(self):     assert rt({}) == {}
    def test_single_key(self): assert rt({"x": 1}) == {"x": 1}
    def test_nested(self):
        v = {"a": {"b": {"c": None}}}
        assert rt(v) == v

    def test_non_string_key_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps({1: "value"})

    def test_key_too_long_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps({"x" * 256: "value"})

    def test_snowed_value_empty(self):
        assert rt_value(SnowedValue.Object({})) == SnowedValue.Object({})

    def test_snowed_value_single(self):
        v = SnowedValue.Object({"k": SnowedValue.Int64(42)})
        assert rt_value(v) == v


# ── Dataclass ─────────────────────────────────────────────────────────────────

class TestDataclass:

    def test_simple(self):
        @dataclass
        class Point:
            x: int
            y: int

        p = Point(3, -7)
        data = snowed.dumps(p)
        back = snowed.loads_dataclass(data, Point)
        assert back == p

    def test_nested(self):
        @dataclass
        class Inner:
            value: str

        @dataclass
        class Outer:
            name: str
            inner: Inner

        o = Outer("test", Inner("hello"))
        data = snowed.dumps(o)
        raw = snowed.loads(data)
        # loads returns dict; use loads_dataclass for typed result
        assert raw["name"] == "test"
        assert raw["inner"]["value"] == "hello"

    def test_default_field(self):
        @dataclass
        class WithDefault:
            x: int
            y: int = 99

        obj = WithDefault(x=1)
        data = snowed.dumps(obj)
        back = snowed.loads_dataclass(data, WithDefault)
        assert back.x == 1
        assert back.y == 99

    def test_default_factory(self):
        @dataclass
        class WithList:
            items: list = field(default_factory=list)

        obj = WithList(items=[1, 2, 3])
        data = snowed.dumps(obj)
        back = snowed.loads_dataclass(data, WithList)
        assert back.items == [1, 2, 3]


# ── uint64 annotation ─────────────────────────────────────────────────────────

class TestUInt64Annotation:

    def test_encodes_as_uint64(self):
        @dataclass
        class Counter:
            count: Annotated[int, uint64]

        data = snowed.dumps(Counter(count=1))
        # Object: 0x80, key "count" (5 bytes), then 0x05 for UInt64
        assert b"\x05" in data[1:]

    def test_round_trip(self):
        @dataclass
        class Counter:
            count: Annotated[int, uint64]

        obj = Counter(count=2**63)
        data = snowed.dumps(obj)
        back = snowed.loads_dataclass(data, Counter)
        assert back.count == 2**63
        assert isinstance(back.count, int)  # unwrapped to plain int

    def test_plain_int_field_is_int64(self):
        @dataclass
        class Signed:
            value: int

        data = snowed.dumps(Signed(value=1))
        assert b"\x04" in data  # Int64 type byte


# ── SnowedValue tree ──────────────────────────────────────────────────────────

class TestSnowedValue:

    def test_all_scalar_types(self):
        cases = [
            SnowedValue.Null(),
            SnowedValue.Bool(True),
            SnowedValue.Int64(-1),
            SnowedValue.UInt64(2**63),
            SnowedValue.Float64(1.5),
            SnowedValue.DateTime(0.0),
            SnowedValue.Binary(b"\xab\xcd"),
            SnowedValue.Str("hello"),
            SnowedValue.Decimal("3.14"),
            SnowedValue.Uuid(bytes(range(16))),
        ]
        for v in cases:
            assert rt_value(v) == v, f"Round-trip failed for {v!r}"

    def test_nested_array(self):
        inner = SnowedValue.Array([SnowedValue.Int64(1), SnowedValue.Int64(2)])
        outer = SnowedValue.Array([inner, SnowedValue.Null()])
        assert rt_value(outer) == outer

    def test_nested_object(self):
        v = SnowedValue.Object({
            "name": SnowedValue.Str("Alice"),
            "scores": SnowedValue.Array([
                SnowedValue.Int64(10),
                SnowedValue.Int64(20),
            ]),
        })
        assert rt_value(v) == v


# ── Streaming API ─────────────────────────────────────────────────────────────

class TestStreaming:

    def test_dump_load(self):
        buf = io.BytesIO()
        snowed.dump({"x": 1}, buf)
        buf.seek(0)
        assert snowed.load(buf) == {"x": 1}


# ── Forward compatibility ─────────────────────────────────────────────────────

class TestForwardCompat:

    def _decode_raw(self, data: bytes):
        return snowed.loads(data)

    def test_unknown_null_subtype(self):
        # 0x01 is an unknown Null subtype → decoded as None
        assert self._decode_raw(wire("01")) is None

    def test_unknown_int_subtype_06(self):
        # 0x06 is an unknown Int64 subtype → decoded as Int64
        data = wire("06") + struct.pack("<q", 42)
        assert self._decode_raw(data) == 42

    def test_unknown_int_subtype_07(self):
        data = wire("07") + struct.pack("<q", -5)
        assert self._decode_raw(data) == -5

    def test_unknown_float_subtype_0a(self):
        # 0x0A is an unknown Float64 subtype → decoded as Float64
        data = wire("0a") + struct.pack("<d", 2.71828)
        result = self._decode_raw(data)
        assert abs(result - 2.71828) < 1e-10

    def test_unknown_binary_subtype(self):
        # 0x14 is an unknown Binary subtype → decoded as bytes
        payload = b"hello"
        data = wire("14") + struct.pack("<Q", len(payload)) + payload
        assert self._decode_raw(data) == payload

    def test_sentinel_in_value_position(self):
        # Object with sentinel (0xFF) in value position terminates object
        data = wire("80") + b"\x01x" + wire("ff")
        result = self._decode_raw(data)
        assert result == {}


# ── Error cases ───────────────────────────────────────────────────────────────

class TestErrors:

    def test_empty_input_raises(self):
        with pytest.raises(SnowedDecodeError):
            snowed.loads(b"")

    def test_truncated_int_raises(self):
        with pytest.raises(SnowedDecodeError):
            snowed.loads(wire("04") + b"\x00\x00")  # only 2 of 8 bytes

    def test_trailing_bytes_raises(self):
        with pytest.raises(SnowedDecodeError):
            snowed.loads(wire("00 00"))  # Null + extra byte

    def test_unexpected_sentinel_raises(self):
        with pytest.raises(SnowedDecodeError):
            snowed.loads(wire("ff"))

    def test_invalid_utf8_string_raises(self):
        payload = b"\xff\xfe"
        data = wire("11") + struct.pack("<Q", len(payload)) + payload
        with pytest.raises(SnowedDecodeError):
            snowed.loads(data)

    def test_invalid_utf8_key_raises(self):
        key = b"\xff\xfe"
        data = wire("80") + bytes([len(key)]) + key + wire("00 00")
        with pytest.raises(SnowedDecodeError):
            snowed.loads(data)

    def test_max_depth_exceeded(self):
        # Build 65 levels of nesting
        data = b"\x80\x01x" * 65 + b"\x00" * 65
        with pytest.raises(SnowedDecodeError):
            snowed.loads(data, max_depth=64)

    def test_encode_unencodable_type_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps(object())

    def test_encode_non_str_dict_key_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps({42: "value"})

    def test_int64_overflow_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps(2**63)

    def test_naive_datetime_raises(self):
        with pytest.raises(SnowedEncodeError):
            snowed.dumps(datetime.datetime(2024, 1, 1))
