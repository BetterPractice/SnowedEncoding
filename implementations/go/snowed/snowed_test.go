package snowed_test

import (
	"testing"
	"time"

	"snowed/snowed"
)

// ── Helpers ──────────────────────────────────────────────────────────────────

func mustMarshal(t *testing.T, v any) []byte {
	t.Helper()
	b, err := snowed.Marshal(v)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}
	return b
}

func mustUnmarshal(t *testing.T, data []byte, v any) {
	t.Helper()
	if err := snowed.Unmarshal(data, v); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
}

func assertBytes(t *testing.T, got []byte, hexStr string) {
	t.Helper()
	want := parseHex(t, hexStr)
	if string(got) != string(want) {
		t.Errorf("bytes mismatch\n got:  %X\n want: %X", got, want)
	}
}

func parseHex(t *testing.T, s string) []byte {
	t.Helper()
	var out []byte
	for _, tok := range splitTokens(s) {
		var b byte
		if _, err := hexSscanf(tok, &b); err != nil {
			t.Fatalf("bad hex token %q: %v", tok, err)
		}
		out = append(out, b)
	}
	return out
}

func splitTokens(s string) []string {
	var tokens []string
	cur := ""
	for _, c := range s {
		if c == ' ' {
			if cur != "" {
				tokens = append(tokens, cur)
				cur = ""
			}
		} else {
			cur += string(c)
		}
	}
	if cur != "" {
		tokens = append(tokens, cur)
	}
	return tokens
}

func hexSscanf(s string, b *byte) (int, error) {
	var v uint8
	_, err := sscanfByte(s, &v)
	*b = v
	return 1, err
}

func sscanfByte(s string, b *uint8) (int, error) {
	if len(s) != 2 {
		return 0, &parseHexError{s}
	}
	hi := hexNibble(s[0])
	lo := hexNibble(s[1])
	if hi < 0 || lo < 0 {
		return 0, &parseHexError{s}
	}
	*b = byte(hi<<4 | lo)
	return 1, nil
}

func hexNibble(c byte) int {
	switch {
	case c >= '0' && c <= '9':
		return int(c - '0')
	case c >= 'a' && c <= 'f':
		return int(c-'a') + 10
	case c >= 'A' && c <= 'F':
		return int(c-'A') + 10
	}
	return -1
}

type parseHexError struct{ s string }

func (e *parseHexError) Error() string { return "invalid hex: " + e.s }

// ── Wire-byte tests ───────────────────────────────────────────────────────────

func TestWireNull(t *testing.T) {
	assertBytes(t, mustMarshal(t, snowed.NullValue()), "00")
}

func TestWireBoolFalse(t *testing.T) {
	assertBytes(t, mustMarshal(t, snowed.BoolValue(false)), "02")
}

func TestWireBoolTrue(t *testing.T) {
	assertBytes(t, mustMarshal(t, snowed.BoolValue(true)), "03")
}

func TestWireInt64_42(t *testing.T) {
	assertBytes(t, mustMarshal(t, snowed.Int64Value(42)), "04 2A 00 00 00 00 00 00 00")
}

func TestWireInt64_Minus1(t *testing.T) {
	assertBytes(t, mustMarshal(t, snowed.Int64Value(-1)), "04 FF FF FF FF FF FF FF FF")
}

func TestWireUInt64Max(t *testing.T) {
	assertBytes(t, mustMarshal(t, snowed.UInt64Value(^uint64(0))), "05 FF FF FF FF FF FF FF FF")
}

func TestWireFloat64_314(t *testing.T) {
	assertBytes(t, mustMarshal(t, snowed.Float64Value(3.14)), "08 1F 85 EB 51 B8 1E 09 40")
}

func TestWireBinaryDeadBeef(t *testing.T) {
	assertBytes(t,
		mustMarshal(t, snowed.BinaryValue([]byte{0xDE, 0xAD, 0xBE, 0xEF})),
		"10 04 00 00 00 00 00 00 00 DE AD BE EF",
	)
}

func TestWireStringHello(t *testing.T) {
	assertBytes(t,
		mustMarshal(t, snowed.StringValue("hello")),
		"11 05 00 00 00 00 00 00 00 68 65 6C 6C 6F",
	)
}

func TestWireUUID(t *testing.T) {
	u := snowed.UUID{0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00}
	assertBytes(t,
		mustMarshal(t, snowed.UUIDValue(u)),
		"13 10 00 00 00 00 00 00 00 55 0E 84 00 E2 9B 41 D4 A7 16 44 66 55 44 00 00",
	)
}

func TestWireEmptyArray(t *testing.T) {
	assertBytes(t, mustMarshal(t, snowed.ArrayValue(nil)), "40 FF")
}

func TestWireArrayTrueAnd42(t *testing.T) {
	arr := snowed.ArrayValue([]snowed.Value{snowed.BoolValue(true), snowed.Int64Value(42)})
	assertBytes(t, mustMarshal(t, arr), "40 03 04 2A 00 00 00 00 00 00 00 FF")
}

func TestWireEmptyObject(t *testing.T) {
	assertBytes(t, mustMarshal(t, snowed.ObjectValue(map[string]snowed.Value{})), "80 00")
}

func TestWireObjectX1(t *testing.T) {
	obj := snowed.ObjectValue(map[string]snowed.Value{"x": snowed.Int64Value(1)})
	assertBytes(t, mustMarshal(t, obj), "80 01 78 04 01 00 00 00 00 00 00 00 00")
}

// ── Value round-trips ─────────────────────────────────────────────────────────

func roundTripValue(t *testing.T, v snowed.Value) snowed.Value {
	t.Helper()
	b := mustMarshal(t, v)
	var out snowed.Value
	mustUnmarshal(t, b, &out)
	return out
}

func TestRoundTripNull(t *testing.T) {
	rt := roundTripValue(t, snowed.NullValue())
	if rt.Type != snowed.TypeNull {
		t.Fatalf("expected Null, got %v", rt.Type)
	}
}

func TestRoundTripBool(t *testing.T) {
	for _, b := range []bool{true, false} {
		rt := roundTripValue(t, snowed.BoolValue(b))
		if rt.Bool != b {
			t.Errorf("bool %v: got %v", b, rt.Bool)
		}
	}
}

func TestRoundTripInt64(t *testing.T) {
	for _, v := range []int64{0, 42, -1, -9223372036854775808, 9223372036854775807} {
		rt := roundTripValue(t, snowed.Int64Value(v))
		if rt.Int64 != v {
			t.Errorf("int64 %d: got %d", v, rt.Int64)
		}
	}
}

func TestRoundTripUInt64(t *testing.T) {
	for _, v := range []uint64{0, ^uint64(0), uint64(1<<63) + 1} {
		rt := roundTripValue(t, snowed.UInt64Value(v))
		if rt.UInt64 != v {
			t.Errorf("uint64 %d: got %d", v, rt.UInt64)
		}
	}
}

func TestRoundTripFloat64(t *testing.T) {
	rt := roundTripValue(t, snowed.Float64Value(3.14))
	if rt.Float64 != 3.14 {
		t.Errorf("expected 3.14, got %v", rt.Float64)
	}
}

func TestRoundTripDateTime(t *testing.T) {
	rt := roundTripValue(t, snowed.DateTimeValue(1_700_000_000_000))
	if rt.DateTime != 1_700_000_000_000 {
		t.Errorf("expected 1700000000000, got %v", rt.DateTime)
	}
}

func TestRoundTripBinary(t *testing.T) {
	payload := []byte{0xDE, 0xAD, 0xBE, 0xEF}
	rt := roundTripValue(t, snowed.BinaryValue(payload))
	if string(rt.Binary) != string(payload) {
		t.Errorf("binary mismatch: %X vs %X", rt.Binary, payload)
	}
}

func TestRoundTripString(t *testing.T) {
	rt := roundTripValue(t, snowed.StringValue("こんにちは🌨️"))
	if rt.String != "こんにちは🌨️" {
		t.Errorf("string mismatch: %q", rt.String)
	}
}

func TestRoundTripDecimal(t *testing.T) {
	rt := roundTripValue(t, snowed.DecimalValue("3.14159"))
	if rt.Decimal != "3.14159" {
		t.Errorf("decimal mismatch: %q", rt.Decimal)
	}
}

func TestRoundTripUUID(t *testing.T) {
	u := snowed.UUID{0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00}
	rt := roundTripValue(t, snowed.UUIDValue(u))
	if rt.UUID != u {
		t.Errorf("uuid mismatch: %v vs %v", rt.UUID, u)
	}
}

func TestRoundTripArray(t *testing.T) {
	arr := snowed.ArrayValue([]snowed.Value{snowed.BoolValue(true), snowed.Int64Value(42)})
	rt := roundTripValue(t, arr)
	if len(rt.Array) != 2 || rt.Array[0].Bool != true || rt.Array[1].Int64 != 42 {
		t.Errorf("array mismatch: %+v", rt.Array)
	}
}

func TestRoundTripObject(t *testing.T) {
	obj := snowed.ObjectValue(map[string]snowed.Value{"x": snowed.Int64Value(1)})
	rt := roundTripValue(t, obj)
	if rt.Object["x"].Int64 != 1 {
		t.Errorf("object mismatch: %+v", rt.Object)
	}
}

// ── Struct round-trips ────────────────────────────────────────────────────────

type simpleStruct struct {
	Name  string `snowed:"name"`
	Count int    `snowed:"count"`
	Flag  bool   `snowed:"flag"`
}

func TestRoundTripStruct(t *testing.T) {
	orig := simpleStruct{Name: "hello", Count: 42, Flag: true}
	b := mustMarshal(t, orig)
	var out simpleStruct
	mustUnmarshal(t, b, &out)
	if out != orig {
		t.Errorf("struct mismatch: %+v vs %+v", out, orig)
	}
}

type omitStruct struct {
	Name  string `snowed:"name"`
	Empty string `snowed:"empty,omitempty"`
}

func TestOmitemptyEmptyString(t *testing.T) {
	orig := omitStruct{Name: "hi", Empty: ""}
	b := mustMarshal(t, orig)
	// "empty" key must not be present.
	var out omitStruct
	mustUnmarshal(t, b, &out)
	if out.Empty != "" {
		t.Errorf("expected empty string, got %q", out.Empty)
	}
	// Verify wire bytes don't contain "empty" key.
	if containsBytes(b, []byte("empty")) {
		t.Error("encoded bytes should not contain the 'empty' key")
	}
}

func containsBytes(haystack, needle []byte) bool {
	for i := range len(haystack) - len(needle) + 1 {
		if string(haystack[i:i+len(needle)]) == string(needle) {
			return true
		}
	}
	return false
}

type timeStruct struct {
	T time.Time `snowed:"t"`
}

func TestRoundTripTime(t *testing.T) {
	orig := timeStruct{T: time.UnixMilli(1_700_000_000_000).UTC()}
	b := mustMarshal(t, orig)
	var out timeStruct
	mustUnmarshal(t, b, &out)
	if !orig.T.Equal(out.T) {
		t.Errorf("time mismatch: %v vs %v", out.T, orig.T)
	}
	if out.T.Location() != time.UTC {
		t.Errorf("decoded time should be UTC, got %v", out.T.Location())
	}
}

type uuidStructNative struct {
	ID snowed.UUID `snowed:"id"`
}

func TestRoundTripUUIDNative(t *testing.T) {
	u := snowed.UUID{0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00}
	orig := uuidStructNative{ID: u}
	b := mustMarshal(t, orig)
	var out uuidStructNative
	mustUnmarshal(t, b, &out)
	if out.ID != u {
		t.Errorf("uuid mismatch: %v vs %v", out.ID, u)
	}
}

type uuidStructString struct {
	ID string `snowed:"id,uuid"`
}

func TestRoundTripUUIDStringTag(t *testing.T) {
	orig := uuidStructString{ID: "550e8400-e29b-41d4-a716-446655440000"}
	b := mustMarshal(t, orig)
	// Wire layout: 0x80 [key_len=2] 'i' 'd' [value_type] ...
	// value type byte is at index 4.
	if b[4] != 0x13 {
		t.Errorf("expected type byte 0x13, got 0x%02X", b[4])
	}
	var out uuidStructString
	mustUnmarshal(t, b, &out)
	if out.ID != "550e8400-e29b-41d4-a716-446655440000" {
		t.Errorf("uuid string mismatch: %q", out.ID)
	}
}

func TestUUIDStringTagCaseInsensitive(t *testing.T) {
	// Uppercase input should parse, lowercase output.
	orig := uuidStructString{ID: "550E8400-E29B-41D4-A716-446655440000"}
	b := mustMarshal(t, orig)
	var out uuidStructString
	mustUnmarshal(t, b, &out)
	if out.ID != "550e8400-e29b-41d4-a716-446655440000" {
		t.Errorf("expected lowercase output, got %q", out.ID)
	}
}

type decimalStruct struct {
	Amount string `snowed:"amount,decimal"`
}

func TestRoundTripDecimalTag(t *testing.T) {
	orig := decimalStruct{Amount: "3.14"}
	b := mustMarshal(t, orig)
	// Find the type byte for the value — after the object header and key.
	// Key is "amount" (6 bytes): 80 06 61 6d 6f 75 6e 74 [type byte]
	// Just verify it decodes correctly.
	var out decimalStruct
	mustUnmarshal(t, b, &out)
	if out.Amount != "3.14" {
		t.Errorf("decimal mismatch: %q", out.Amount)
	}
}

// ── UUID parsing ──────────────────────────────────────────────────────────────

func TestParseUUIDValid(t *testing.T) {
	cases := []struct {
		in   string
		want string
	}{
		{"550e8400-e29b-41d4-a716-446655440000", "550e8400-e29b-41d4-a716-446655440000"},
		{"550E8400-E29B-41D4-A716-446655440000", "550e8400-e29b-41d4-a716-446655440000"},
		{"550e8400e29b41d4a716446655440000", "550e8400-e29b-41d4-a716-446655440000"},
	}
	for _, c := range cases {
		u, err := snowed.ParseUUID(c.in)
		if err != nil {
			t.Errorf("ParseUUID(%q): unexpected error %v", c.in, err)
			continue
		}
		if got := u.String(); got != c.want {
			t.Errorf("ParseUUID(%q).String() = %q, want %q", c.in, got, c.want)
		}
	}
}

func TestParseUUIDInvalid(t *testing.T) {
	cases := []string{
		"",
		"not-a-uuid",
		"550e8400-e29b-41d4-a716-44665544000",  // too short
		"550e8400-e29b-41d4-a716-4466554400000", // too long
		"550e8400-e29b-41d4-a716-44665544000z",  // invalid char
	}
	for _, c := range cases {
		_, err := snowed.ParseUUID(c)
		if err == nil {
			t.Errorf("ParseUUID(%q): expected error, got nil", c)
		}
	}
}

// ── Error cases ───────────────────────────────────────────────────────────────

func TestDecodeUnexpectedSentinel(t *testing.T) {
	var v snowed.Value
	if err := snowed.Unmarshal([]byte{0xFF}, &v); err == nil {
		t.Error("expected error for sentinel as root value")
	}
}

func TestDecodeTrailingBytes(t *testing.T) {
	var v snowed.Value
	if err := snowed.Unmarshal([]byte{0x00, 0x00}, &v); err == nil {
		t.Error("expected error for trailing bytes")
	}
}

func TestDecodeUnexpectedEOF(t *testing.T) {
	var v snowed.Value
	// Int64 type byte with only 4 payload bytes (need 8).
	if err := snowed.Unmarshal([]byte{0x04, 0x00, 0x00, 0x00, 0x00}, &v); err == nil {
		t.Error("expected error for truncated input")
	}
}

func TestDecodeUUIDOnUntaggedStringErrors(t *testing.T) {
	// Encode a UUID value, try to decode into a plain string field.
	type wrong struct {
		ID string `snowed:"id"`
	}
	u := snowed.UUID{0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00}
	b := mustMarshal(t, snowed.ObjectValue(map[string]snowed.Value{"id": snowed.UUIDValue(u)}))
	var out wrong
	if err := snowed.Unmarshal(b, &out); err == nil {
		t.Error("expected error decoding UUID into untagged string field")
	}
}

// ── Forward compatibility ─────────────────────────────────────────────────────

func TestForwardCompatUnknownNullSubtype(t *testing.T) {
	var v snowed.Value
	mustUnmarshal(t, []byte{0x01}, &v)
	if v.Type != snowed.TypeNull {
		t.Errorf("expected Null, got %v", v.Type)
	}
}

func TestForwardCompatUnknownIntSubtype(t *testing.T) {
	// 0x06 → Int64
	data := []byte{0x06, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00}
	var v snowed.Value
	mustUnmarshal(t, data, &v)
	if v.Type != snowed.TypeInt64 || v.Int64 != 42 {
		t.Errorf("unexpected value: %+v", v)
	}
}

func TestForwardCompatUnknownFloatSubtype(t *testing.T) {
	// 0x0A → Float64
	import_bits := floatBitsLE(3.14)
	data := append([]byte{0x0A}, import_bits...)
	var v snowed.Value
	mustUnmarshal(t, data, &v)
	if v.Type != snowed.TypeFloat64 || v.Float64 != 3.14 {
		t.Errorf("unexpected value: %+v", v)
	}
}

func TestForwardCompatUnknownBinarySubtype(t *testing.T) {
	// 0x14 → Binary
	payload := []byte("test")
	data := []byte{0x14, byte(len(payload)), 0, 0, 0, 0, 0, 0, 0}
	data = append(data, payload...)
	var v snowed.Value
	mustUnmarshal(t, data, &v)
	if v.Type != snowed.TypeBinary || string(v.Binary) != "test" {
		t.Errorf("unexpected value: %+v", v)
	}
}

func floatBitsLE(f float64) []byte {
	import_math := mathFloat64bits(f)
	b := make([]byte, 8)
	b[0] = byte(import_math)
	b[1] = byte(import_math >> 8)
	b[2] = byte(import_math >> 16)
	b[3] = byte(import_math >> 24)
	b[4] = byte(import_math >> 32)
	b[5] = byte(import_math >> 40)
	b[6] = byte(import_math >> 48)
	b[7] = byte(import_math >> 56)
	return b
}

func mathFloat64bits(f float64) uint64 {
	// Avoid importing math in test file by reusing snowed's encoding.
	b, _ := snowed.Marshal(snowed.Float64Value(f))
	// b = [0x08, b0..b7] — skip type byte.
	return uint64(b[1]) | uint64(b[2])<<8 | uint64(b[3])<<16 | uint64(b[4])<<24 |
		uint64(b[5])<<32 | uint64(b[6])<<40 | uint64(b[7])<<48 | uint64(b[8])<<56
}
