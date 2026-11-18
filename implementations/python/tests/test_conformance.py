"""
Cross-implementation conformance tests.

Runs every vector in ``tests/conformance/vectors/*.json`` (repository root)
against the Python implementation, as described in
``tests/conformance/README.md``.

Decoding uses :func:`snowed.loads_value`, which returns the type-preserving
:class:`snowed.SnowedValue` tree (distinguishes Int64/UInt64, Binary/String/
Decimal/UUID, Float64/DateTime). Re-encoding uses :func:`snowed.dumps` on that
tree.
"""
from __future__ import annotations

import base64
import json
import math
import uuid
from pathlib import Path
from typing import Any

import pytest

import snowed
from snowed import SnowedDecodeError, SnowedValue

VECTORS_DIR = (
    Path(__file__).resolve().parent.parent.parent.parent
    / "tests"
    / "conformance"
    / "vectors"
)


# ── Vector loading ────────────────────────────────────────────────────────────

def _vector_files() -> list[Path]:
    if not VECTORS_DIR.is_dir():
        return []
    return sorted(VECTORS_DIR.glob("*.json"))


def _load_cases() -> list[Any]:
    params = []
    for path in _vector_files():
        doc = json.loads(path.read_text(encoding="utf-8"))
        suite = doc["suite"]
        for case in doc["cases"]:
            params.append(
                pytest.param(suite, case, id=f"{suite}/{case['name']}")
            )
    return params


_CASES = _load_cases()


def test_vectors_present() -> None:
    """The runner must fail, not pass vacuously, when vectors are missing."""
    assert VECTORS_DIR.is_dir(), f"Conformance vector directory not found: {VECTORS_DIR}"
    assert _vector_files(), f"No .json vector files in {VECTORS_DIR}"
    assert _CASES, f"Vector files in {VECTORS_DIR} contain no cases"


# ── Expected-value comparison ─────────────────────────────────────────────────

def _parse_int(value: Any, path: str) -> int:
    """Parse an integer vector value exactly (never via float)."""
    if isinstance(value, bool):
        raise AssertionError(f"{path}: malformed vector integer {value!r}")
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        return int(value, 10)
    if isinstance(value, float) and value.is_integer():
        return int(value)
    raise AssertionError(f"{path}: malformed vector integer {value!r}")


def _parse_float(value: Any) -> float:
    if isinstance(value, str):
        return {
            "Infinity": math.inf,
            "-Infinity": -math.inf,
            "NaN": math.nan,
            "-0": -0.0,
        }[value]
    return float(value)


def _float_matches(actual: float, expected: float) -> bool:
    if math.isnan(expected):
        return math.isnan(actual)
    if expected == 0.0:
        # Zero sign is significant: "-0" must be negative zero, 0 must not be.
        return actual == 0.0 and math.copysign(1.0, actual) == math.copysign(1.0, expected)
    return actual == expected


def _assert_matches(actual: SnowedValue, expected: dict[str, Any], path: str = "$") -> None:
    etype = expected["type"]
    evalue = expected.get("value")

    def fail(detail: str) -> None:
        raise AssertionError(f"{path}: expected {etype} {evalue!r}, {detail}")

    if etype == "null":
        if not isinstance(actual, SnowedValue.Null):
            fail(f"got {actual!r}")

    elif etype == "bool":
        if not isinstance(actual, SnowedValue.Bool) or actual.value is not evalue:
            fail(f"got {actual!r}")

    elif etype == "int64":
        if not isinstance(actual, SnowedValue.Int64) or actual.value != _parse_int(evalue, path):
            fail(f"got {actual!r}")

    elif etype == "uint64":
        if not isinstance(actual, SnowedValue.UInt64) or actual.value != _parse_int(evalue, path):
            fail(f"got {actual!r}")

    elif etype == "float64":
        if not isinstance(actual, SnowedValue.Float64) or not _float_matches(
            actual.value, _parse_float(evalue)
        ):
            fail(f"got {actual!r}")

    elif etype == "datetime":
        if not isinstance(actual, SnowedValue.DateTime) or not _float_matches(
            actual.millis, _parse_float(evalue)
        ):
            fail(f"got {actual!r}")

    elif etype == "binary":
        if not isinstance(actual, SnowedValue.Binary) or actual.data != base64.b64decode(
            evalue, validate=True
        ):
            fail(f"got {actual!r}")

    elif etype == "string":
        if not isinstance(actual, SnowedValue.Str) or actual.value != evalue:
            fail(f"got {actual!r}")

    elif etype == "decimal":
        if not isinstance(actual, SnowedValue.Decimal) or actual.value != evalue:
            fail(f"got {actual!r}")

    elif etype == "uuid":
        if not isinstance(actual, SnowedValue.Uuid) or str(
            uuid.UUID(bytes=actual.bytes_)
        ) != evalue:
            fail(f"got {actual!r}")

    elif etype == "array":
        if not isinstance(actual, SnowedValue.Array):
            fail(f"got {actual!r}")
        if len(actual.elements) != len(evalue):
            fail(f"got array of length {len(actual.elements)}: {actual!r}")
        for i, (a, e) in enumerate(zip(actual.elements, evalue)):
            _assert_matches(a, e, f"{path}[{i}]")

    elif etype == "object":
        if not isinstance(actual, SnowedValue.Object):
            fail(f"got {actual!r}")
        if set(actual.entries) != set(evalue):
            fail(f"got keys {sorted(actual.entries)!r}")
        for key, e in evalue.items():
            _assert_matches(actual.entries[key], e, f"{path}[{key!r}]")

    else:
        raise AssertionError(f"{path}: unknown vector type {etype!r}")


# ── Runner ────────────────────────────────────────────────────────────────────

def _hex_bytes(hex_str: str) -> bytes:
    return bytes.fromhex(hex_str)


@pytest.mark.parametrize(("suite", "case"), _CASES)
def test_conformance(suite: str, case: dict[str, Any]) -> None:
    data = _hex_bytes(case["hex"])
    label = f"[{suite}/{case['name']}]"

    if case.get("error") is True:
        with pytest.raises(SnowedDecodeError):
            result = snowed.loads_value(data)
            pytest.fail(f"{label} expected SnowedDecodeError, decoded {result!r}")
        return

    assert "decoded" in case, f"{label} vector has neither 'decoded' nor 'error'"
    expected = case["decoded"]

    # 1. Decode and compare.
    decoded = snowed.loads_value(data)
    try:
        _assert_matches(decoded, expected)
    except AssertionError as e:
        raise AssertionError(f"{label} decode: {e}") from None

    # 2. Semantic round-trip.
    reencoded = snowed.dumps(decoded)
    redecoded = snowed.loads_value(reencoded)
    try:
        _assert_matches(redecoded, expected)
    except AssertionError as e:
        raise AssertionError(
            f"{label} round-trip (re-encoded {reencoded.hex(' ').upper()}): {e}"
        ) from None

    # 3. Byte-exact canonical re-encoding.
    if case.get("canonical", True):
        assert reencoded == data, (
            f"{label} canonical re-encoding mismatch: "
            f"expected {data.hex(' ').upper()}, got {reencoded.hex(' ').upper()}"
        )
