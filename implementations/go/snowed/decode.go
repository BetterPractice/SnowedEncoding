package snowed

import (
	"encoding/binary"
	"fmt"
	"math"
	"reflect"
	"strings"
	"time"
)

// unmarshalError is returned when bytes cannot be decoded.
type unmarshalError struct{ msg string }

func (e *unmarshalError) Error() string { return "snowed: " + e.msg }

// ── Low-level reader ─────────────────────────────────────────────────────────

type reader struct {
	data []byte
	pos  int
}

func newReader(data []byte) *reader { return &reader{data: data} }

func (r *reader) remaining() int { return len(r.data) - r.pos }

func (r *reader) readByte() (byte, error) {
	if r.pos >= len(r.data) {
		return 0, &unmarshalError{"unexpected end of input"}
	}
	b := r.data[r.pos]
	r.pos++
	return b, nil
}

func (r *reader) readLE64() (uint64, error) {
	if r.pos+8 > len(r.data) {
		return 0, &unmarshalError{"unexpected end of input"}
	}
	v := binary.LittleEndian.Uint64(r.data[r.pos : r.pos+8])
	r.pos += 8
	return v, nil
}

func (r *reader) readBytes(n int) ([]byte, error) {
	if r.pos+n > len(r.data) {
		return nil, &unmarshalError{"unexpected end of input"}
	}
	b := r.data[r.pos : r.pos+n]
	r.pos += n
	return b, nil
}

func (r *reader) readLengthPrefixed() ([]byte, error) {
	lenVal, err := r.readLE64()
	if err != nil {
		return nil, err
	}
	if lenVal > uint64(len(r.data)-r.pos) {
		return nil, &unmarshalError{"length prefix exceeds available data"}
	}
	return r.readBytes(int(lenVal))
}

// ── Value decoder ─────────────────────────────────────────────────────────────

// decodeValue reads and returns the next Value from r.
func decodeValue(r *reader) (Value, error) {
	tb, err := r.readByte()
	if err != nil {
		return Value{}, err
	}
	return decodeValueWithTypeByte(r, tb)
}

func decodeValueWithTypeByte(r *reader, tb byte) (Value, error) {
	switch {
	// ── Null family (0x00–0x03) ──────────────────────────────────────────────
	case tb == 0x00 || tb == 0x01:
		return NullValue(), nil
	case tb == 0x02:
		return BoolValue(false), nil
	case tb == 0x03:
		return BoolValue(true), nil

	// ── Int family (0x04–0x07) ───────────────────────────────────────────────
	case tb >= 0x04 && tb <= 0x07:
		bits, err := r.readLE64()
		if err != nil {
			return Value{}, err
		}
		if tb == 0x05 {
			return UInt64Value(bits), nil
		}
		return Int64Value(int64(bits)), nil

	// ── Float family (0x08–0x0F) ─────────────────────────────────────────────
	case tb >= 0x08 && tb <= 0x0F:
		bits, err := r.readLE64()
		if err != nil {
			return Value{}, err
		}
		f := math.Float64frombits(bits)
		if tb == 0x09 {
			return DateTimeValue(f), nil
		}
		return Float64Value(f), nil

	// ── Binary family (0x10–0x3F) ────────────────────────────────────────────
	case tb >= 0x10 && tb <= 0x3F:
		payload, err := r.readLengthPrefixed()
		if err != nil {
			return Value{}, err
		}
		switch tb {
		case 0x11:
			if !isValidUTF8(payload) {
				return Value{}, &unmarshalError{"string value contains invalid UTF-8"}
			}
			return StringValue(string(payload)), nil
		case 0x12:
			if !isValidUTF8(payload) {
				return Value{}, &unmarshalError{"decimal value contains invalid UTF-8"}
			}
			return DecimalValue(string(payload)), nil
		case 0x13:
			if len(payload) != 16 {
				return Value{}, &unmarshalError{fmt.Sprintf("UUID payload must be 16 bytes, got %d", len(payload))}
			}
			var u UUID
			copy(u[:], payload)
			return UUIDValue(u), nil
		default:
			return BinaryValue(append([]byte(nil), payload...)), nil
		}

	// ── Array family (0x40–0x7F) ─────────────────────────────────────────────
	case tb >= 0x40 && tb <= 0x7F:
		var elems []Value
		for {
			next, err := r.readByte()
			if err != nil {
				return Value{}, err
			}
			if next == 0xFF {
				break
			}
			v, err := decodeValueWithTypeByte(r, next)
			if err != nil {
				return Value{}, err
			}
			elems = append(elems, v)
		}
		return ArrayValue(elems), nil

	// ── Object family (0x80–0xFE) ────────────────────────────────────────────
	case tb >= 0x80 && tb <= 0xFE:
		m := make(map[string]Value)
		for {
			keyLen, err := r.readByte()
			if err != nil {
				return Value{}, err
			}
			if keyLen == 0x00 {
				break
			}
			keyBytes, err := r.readBytes(int(keyLen))
			if err != nil {
				return Value{}, err
			}
			if !isValidUTF8(keyBytes) {
				return Value{}, &unmarshalError{"object key contains invalid UTF-8"}
			}
			key := string(keyBytes)
			vt, err := r.readByte()
			if err != nil {
				return Value{}, err
			}
			if vt == 0xFF {
				break
			}
			val, err := decodeValueWithTypeByte(r, vt)
			if err != nil {
				return Value{}, err
			}
			m[key] = val // last value wins on duplicate keys
		}
		return ObjectValue(m), nil

	// ── Sentinel (0xFF) ──────────────────────────────────────────────────────
	case tb == 0xFF:
		return Value{}, &unmarshalError{"unexpected sentinel (0xFF) in value position"}
	}

	return Value{}, &unmarshalError{fmt.Sprintf("unhandled type byte 0x%02X", tb)}
}

// ── Reflection-based struct decoder ──────────────────────────────────────────

// decodeInto decodes the next value from r into the Go value pointed to by dst.
func decodeInto(r *reader, dst reflect.Value) error {
	// Read the raw value first.
	v, err := decodeValue(r)
	if err != nil {
		return err
	}
	return assignValue(v, dst)
}

// assignValue assigns a decoded Value into the Go value dst.
func assignValue(v Value, dst reflect.Value) error {
	// Check for Unmarshaler interface.
	if dst.CanAddr() {
		if u, ok := dst.Addr().Interface().(Unmarshaler); ok {
			// Re-encode to []byte for the interface.
			// We pass the Value via a small helper.
			return unmarshalIntoUnmarshaler(v, u)
		}
	}

	// Dereference / allocate pointers.
	for dst.Kind() == reflect.Ptr {
		if v.Type == TypeNull {
			// nil out the pointer
			dst.Set(reflect.Zero(dst.Type()))
			return nil
		}
		if dst.IsNil() {
			dst.Set(reflect.New(dst.Type().Elem()))
		}
		dst = dst.Elem()
	}

	// Check well-known target types.
	if dst.Type() == reflect.TypeOf(time.Time{}) {
		if v.Type != TypeDateTime {
			return typeMismatch("time.Time", v.Type)
		}
		ms := int64(v.DateTime)
		dst.Set(reflect.ValueOf(time.UnixMilli(ms).UTC()))
		return nil
	}

	if dst.Type() == reflect.TypeOf(UUID{}) {
		if v.Type != TypeUUID {
			return typeMismatch("snowed.UUID", v.Type)
		}
		dst.Set(reflect.ValueOf(v.UUID))
		return nil
	}

	// Check for Value target.
	if dst.Type() == reflect.TypeOf(Value{}) {
		dst.Set(reflect.ValueOf(v))
		return nil
	}

	switch dst.Kind() {
	case reflect.Bool:
		if v.Type != TypeBool {
			return typeMismatch("bool", v.Type)
		}
		dst.SetBool(v.Bool)

	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		switch v.Type {
		case TypeInt64:
			dst.SetInt(v.Int64)
		case TypeUInt64:
			dst.SetInt(int64(v.UInt64))
		default:
			return typeMismatch(dst.Kind().String(), v.Type)
		}

	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64, reflect.Uintptr:
		switch v.Type {
		case TypeUInt64:
			dst.SetUint(v.UInt64)
		case TypeInt64:
			dst.SetUint(uint64(v.Int64))
		default:
			return typeMismatch(dst.Kind().String(), v.Type)
		}

	case reflect.Float32, reflect.Float64:
		if v.Type != TypeFloat64 && v.Type != TypeDateTime {
			return typeMismatch(dst.Kind().String(), v.Type)
		}
		dst.SetFloat(v.Float64)

	case reflect.String:
		switch v.Type {
		case TypeString:
			dst.SetString(v.String)
		case TypeDecimal:
			dst.SetString(v.Decimal)
		case TypeUUID:
			// UUID arriving on an untagged string field is an error.
			return &unmarshalError{"cannot decode UUID (0x13) into untagged string field; add `snowed:\",uuid\"` tag"}
		default:
			return typeMismatch("string", v.Type)
		}

	case reflect.Slice:
		if dst.Type().Elem().Kind() == reflect.Uint8 {
			// []byte ← Binary
			if v.Type == TypeNull {
				dst.Set(reflect.Zero(dst.Type()))
				return nil
			}
			if v.Type != TypeBinary {
				return typeMismatch("[]byte", v.Type)
			}
			dst.Set(reflect.ValueOf(append([]byte(nil), v.Binary...)))
			return nil
		}
		if v.Type == TypeNull {
			dst.Set(reflect.Zero(dst.Type()))
			return nil
		}
		if v.Type != TypeArray {
			return typeMismatch("slice", v.Type)
		}
		sl := reflect.MakeSlice(dst.Type(), len(v.Array), len(v.Array))
		for i, elem := range v.Array {
			if err := assignValue(elem, sl.Index(i)); err != nil {
				return err
			}
		}
		dst.Set(sl)

	case reflect.Map:
		if v.Type == TypeNull {
			dst.Set(reflect.Zero(dst.Type()))
			return nil
		}
		if v.Type != TypeObject {
			return typeMismatch("map", v.Type)
		}
		if dst.Type().Key().Kind() != reflect.String {
			return &unmarshalError{fmt.Sprintf("cannot decode Object into map with non-string key type %s", dst.Type().Key())}
		}
		m := reflect.MakeMap(dst.Type())
		for k, val := range v.Object {
			ev := reflect.New(dst.Type().Elem()).Elem()
			if err := assignValue(val, ev); err != nil {
				return err
			}
			m.SetMapIndex(reflect.ValueOf(k), ev)
		}
		dst.Set(m)

	case reflect.Struct:
		if v.Type == TypeNull {
			dst.Set(reflect.Zero(dst.Type()))
			return nil
		}
		if v.Type != TypeObject {
			return typeMismatch("struct", v.Type)
		}
		return assignObject(v.Object, dst)

	default:
		return &unmarshalError{fmt.Sprintf("unsupported target type %s", dst.Type())}
	}
	return nil
}

func assignObject(m map[string]Value, dst reflect.Value) error {
	t := dst.Type()
	// Build name→field index map.
	fieldMap := make(map[string]int, t.NumField())
	for i := range t.NumField() {
		f := t.Field(i)
		if !f.IsExported() {
			continue
		}
		tag := parseFieldTag(f)
		if tag.skip {
			continue
		}
		fieldMap[tag.name] = i
	}

	for key, val := range m {
		idx, ok := fieldMap[key]
		if !ok {
			continue // unknown fields are silently ignored
		}
		f := t.Field(idx)
		tag := parseFieldTag(f)
		fv := dst.Field(idx)

		if err := assignFieldValue(val, fv, tag); err != nil {
			return fmt.Errorf("field %q: %w", key, err)
		}
	}
	return nil
}

// assignFieldValue handles tag modifiers (decimal, uuid) on decode.
func assignFieldValue(v Value, dst reflect.Value, tag fieldTag) error {
	// Dereference / allocate pointers.
	for dst.Kind() == reflect.Ptr {
		if v.Type == TypeNull {
			dst.Set(reflect.Zero(dst.Type()))
			return nil
		}
		if dst.IsNil() {
			dst.Set(reflect.New(dst.Type().Elem()))
		}
		dst = dst.Elem()
	}

	if tag.decimal {
		if dst.Kind() != reflect.String {
			return &unmarshalError{"decimal tag requires a string field"}
		}
		if v.Type != TypeDecimal {
			return typeMismatch("decimal string", v.Type)
		}
		dst.SetString(v.Decimal)
		return nil
	}

	if tag.uuid {
		if dst.Kind() != reflect.String {
			return &unmarshalError{"uuid tag requires a string field"}
		}
		if v.Type != TypeUUID {
			return typeMismatch("UUID string", v.Type)
		}
		dst.SetString(v.UUID.String())
		return nil
	}

	return assignValue(v, dst)
}

// unmarshalIntoUnmarshaler re-encodes a Value and calls UnmarshalSnowed.
func unmarshalIntoUnmarshaler(v Value, u Unmarshaler) error {
	// Use an in-memory buffer to produce the bytes.
	buf := &bytesBuffer{}
	if err := encodeValue(buf, v); err != nil {
		return err
	}
	return u.UnmarshalSnowed(buf.b)
}

// bytesBuffer is a minimal io.Writer backed by a []byte.
type bytesBuffer struct{ b []byte }

func (bb *bytesBuffer) Write(p []byte) (int, error) {
	bb.b = append(bb.b, p...)
	return len(p), nil
}

// ── Helpers ───────────────────────────────────────────────────────────────────

func typeMismatch(want string, got ValueType) error {
	return &unmarshalError{fmt.Sprintf("cannot decode %s (0x%02X) into %s", valueTypeName(got), got, want)}
}

func valueTypeName(t ValueType) string {
	names := map[ValueType]string{
		TypeNull: "Null", TypeBool: "Bool", TypeInt64: "Int64",
		TypeUInt64: "UInt64", TypeFloat64: "Float64", TypeDateTime: "DateTime",
		TypeBinary: "Binary", TypeString: "String", TypeDecimal: "Decimal",
		TypeUUID: "UUID", TypeArray: "Array", TypeObject: "Object",
	}
	if n, ok := names[t]; ok {
		return n
	}
	return fmt.Sprintf("0x%02X", uint8(t))
}

func isValidUTF8(b []byte) bool {
	s := string(b)
	return strings.ToValidUTF8(s, "\uFFFD") == s
}
