package snowed

import (
	"encoding/binary"
	"fmt"
	"io"
	"math"
	"reflect"
	"sort"
	"strings"
	"time"
	"unicode/utf8"
)

// marshalError is returned when a value cannot be encoded.
type marshalError struct{ msg string }

func (e *marshalError) Error() string { return "snowed: " + e.msg }

// ── Low-level write helpers ──────────────────────────────────────────────────

func writeByte(w io.Writer, b byte) error {
	_, err := w.Write([]byte{b})
	return err
}

func writeLE64(w io.Writer, v uint64) error {
	var buf [8]byte
	binary.LittleEndian.PutUint64(buf[:], v)
	_, err := w.Write(buf[:])
	return err
}

func writeBytesWithLength(w io.Writer, b []byte) error {
	if err := writeLE64(w, uint64(len(b))); err != nil {
		return err
	}
	_, err := w.Write(b)
	return err
}

// ── Value encoder ────────────────────────────────────────────────────────────

// encodeValue writes a snowed.Value to w.
func encodeValue(w io.Writer, v Value) error {
	switch v.Type {
	case TypeNull:
		return writeByte(w, 0x00)

	case TypeBool:
		if v.Bool {
			return writeByte(w, 0x03)
		}
		return writeByte(w, 0x02)

	case TypeInt64:
		if err := writeByte(w, 0x04); err != nil {
			return err
		}
		return writeLE64(w, uint64(v.Int64))

	case TypeUInt64:
		if err := writeByte(w, 0x05); err != nil {
			return err
		}
		return writeLE64(w, v.UInt64)

	case TypeFloat64:
		if err := writeByte(w, 0x08); err != nil {
			return err
		}
		return writeLE64(w, math.Float64bits(v.Float64))

	case TypeDateTime:
		if err := writeByte(w, 0x09); err != nil {
			return err
		}
		return writeLE64(w, math.Float64bits(v.DateTime))

	case TypeBinary:
		if err := writeByte(w, 0x10); err != nil {
			return err
		}
		return writeBytesWithLength(w, v.Binary)

	case TypeString:
		if err := writeByte(w, 0x11); err != nil {
			return err
		}
		return writeBytesWithLength(w, []byte(v.String))

	case TypeDecimal:
		if err := writeByte(w, 0x12); err != nil {
			return err
		}
		return writeBytesWithLength(w, []byte(v.Decimal))

	case TypeUUID:
		if err := writeByte(w, 0x13); err != nil {
			return err
		}
		if err := writeLE64(w, 16); err != nil {
			return err
		}
		_, err := w.Write(v.UUID[:])
		return err

	case TypeArray:
		if err := writeByte(w, 0x40); err != nil {
			return err
		}
		for _, elem := range v.Array {
			if err := encodeValue(w, elem); err != nil {
				return err
			}
		}
		return writeByte(w, 0xFF)

	case TypeObject:
		if err := writeByte(w, 0x80); err != nil {
			return err
		}
		// Sort keys for deterministic output.
		keys := make([]string, 0, len(v.Object))
		for k := range v.Object {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			kb := []byte(k)
			if len(kb) > 255 {
				return &marshalError{fmt.Sprintf("object key %q exceeds 255 bytes", k)}
			}
			if err := writeByte(w, byte(len(kb))); err != nil {
				return err
			}
			if _, err := w.Write(kb); err != nil {
				return err
			}
			if err := encodeValue(w, v.Object[k]); err != nil {
				return err
			}
		}
		return writeByte(w, 0x00) // zero-length key terminates

	default:
		return &marshalError{fmt.Sprintf("unknown ValueType 0x%02X", v.Type)}
	}
}

// ── Reflection-based struct encoder ─────────────────────────────────────────

// encodeReflect encodes an arbitrary Go value via reflection.
func encodeReflect(w io.Writer, v reflect.Value) error {
	// Dereference pointers; nil pointer → Null.
	for v.Kind() == reflect.Ptr || v.Kind() == reflect.Interface {
		if v.IsNil() {
			return writeByte(w, 0x00)
		}
		v = v.Elem()
	}

	// Check for Marshaler interface.
	if v.CanAddr() {
		if m, ok := v.Addr().Interface().(Marshaler); ok {
			b, err := m.MarshalSnowed()
			if err != nil {
				return err
			}
			_, err = w.Write(b)
			return err
		}
	}

	// Check well-known types.
	iface := v.Interface()

	if t, ok := iface.(time.Time); ok {
		return encodeTime(w, t)
	}
	if u, ok := iface.(UUID); ok {
		return encodeUUID(w, u)
	}

	switch v.Kind() {
	case reflect.Bool:
		if v.Bool() {
			return writeByte(w, 0x03)
		}
		return writeByte(w, 0x02)

	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		if err := writeByte(w, 0x04); err != nil {
			return err
		}
		return writeLE64(w, uint64(v.Int()))

	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64, reflect.Uintptr:
		if err := writeByte(w, 0x05); err != nil {
			return err
		}
		return writeLE64(w, v.Uint())

	case reflect.Float32, reflect.Float64:
		if err := writeByte(w, 0x08); err != nil {
			return err
		}
		return writeLE64(w, math.Float64bits(v.Float()))

	case reflect.String:
		return encodeString(w, v.String())

	case reflect.Slice:
		if v.Type().Elem().Kind() == reflect.Uint8 {
			// []byte → Binary
			if v.IsNil() {
				return writeByte(w, 0x00)
			}
			if err := writeByte(w, 0x10); err != nil {
				return err
			}
			return writeBytesWithLength(w, v.Bytes())
		}
		if v.IsNil() {
			return writeByte(w, 0x00)
		}
		return encodeSlice(w, v)

	case reflect.Array:
		// [16]byte treated as UUID handled above via iface check.
		return encodeSlice(w, v)

	case reflect.Map:
		if v.IsNil() {
			return writeByte(w, 0x00)
		}
		return encodeMap(w, v)

	case reflect.Struct:
		return encodeStruct(w, v)

	default:
		return &marshalError{fmt.Sprintf("unsupported type %s", v.Type())}
	}
}

func encodeTime(w io.Writer, t time.Time) error {
	ms := float64(t.UTC().UnixMilli())
	if err := writeByte(w, 0x09); err != nil {
		return err
	}
	return writeLE64(w, math.Float64bits(ms))
}

func encodeUUID(w io.Writer, u UUID) error {
	if err := writeByte(w, 0x13); err != nil {
		return err
	}
	if err := writeLE64(w, 16); err != nil {
		return err
	}
	_, err := w.Write(u[:])
	return err
}

func encodeString(w io.Writer, s string) error {
	if err := writeByte(w, 0x11); err != nil {
		return err
	}
	return writeBytesWithLength(w, []byte(s))
}

func encodeSlice(w io.Writer, v reflect.Value) error {
	if err := writeByte(w, 0x40); err != nil {
		return err
	}
	for i := range v.Len() {
		if err := encodeReflect(w, v.Index(i)); err != nil {
			return err
		}
	}
	return writeByte(w, 0xFF)
}

func encodeMap(w io.Writer, v reflect.Value) error {
	if v.Type().Key().Kind() != reflect.String {
		return &marshalError{fmt.Sprintf("map key type must be string, got %s", v.Type().Key())}
	}
	if err := writeByte(w, 0x80); err != nil {
		return err
	}
	// Sort keys for deterministic output.
	keys := make([]string, 0, v.Len())
	for _, k := range v.MapKeys() {
		keys = append(keys, k.String())
	}
	sort.Strings(keys)
	for _, k := range keys {
		kb := []byte(k)
		if len(kb) > 255 {
			return &marshalError{fmt.Sprintf("map key %q exceeds 255 bytes", k)}
		}
		if err := writeByte(w, byte(len(kb))); err != nil {
			return err
		}
		if _, err := w.Write(kb); err != nil {
			return err
		}
		if err := encodeReflect(w, v.MapIndex(reflect.ValueOf(k))); err != nil {
			return err
		}
	}
	return writeByte(w, 0x00)
}

func encodeStruct(w io.Writer, v reflect.Value) error {
	t := v.Type()

	// Collect fields that will be written.
	type fieldEntry struct {
		key string
		val reflect.Value
		tag fieldTag
	}
	var fields []fieldEntry

	for i := range t.NumField() {
		f := t.Field(i)
		if !f.IsExported() {
			continue
		}
		tag := parseFieldTag(f)
		if tag.skip {
			continue
		}
		fv := v.Field(i)
		if tag.omitempty && isEmptyValue(fv) {
			continue
		}
		fields = append(fields, fieldEntry{key: tag.name, val: fv, tag: tag})
	}

	// Sort for deterministic output.
	sort.Slice(fields, func(i, j int) bool { return fields[i].key < fields[j].key })

	if err := writeByte(w, 0x80); err != nil {
		return err
	}
	for _, fe := range fields {
		kb := []byte(fe.key)
		if len(kb) > 255 {
			return &marshalError{fmt.Sprintf("field key %q exceeds 255 bytes", fe.key)}
		}
		if err := writeByte(w, byte(len(kb))); err != nil {
			return err
		}
		if _, err := w.Write(kb); err != nil {
			return err
		}
		if err := encodeFieldValue(w, fe.val, fe.tag); err != nil {
			return err
		}
	}
	return writeByte(w, 0x00)
}

// encodeFieldValue applies tag modifiers (decimal, uuid) before encoding.
func encodeFieldValue(w io.Writer, v reflect.Value, tag fieldTag) error {
	// Dereference pointers.
	for v.Kind() == reflect.Ptr {
		if v.IsNil() {
			return writeByte(w, 0x00)
		}
		v = v.Elem()
	}

	if tag.decimal {
		if v.Kind() != reflect.String {
			return &marshalError{"decimal tag requires a string field"}
		}
		s := v.String()
		if err := validateDecimalString(s); err != nil {
			return err
		}
		if err := writeByte(w, 0x12); err != nil {
			return err
		}
		return writeBytesWithLength(w, []byte(s))
	}

	if tag.uuid {
		if v.Kind() != reflect.String {
			return &marshalError{"uuid tag requires a string field"}
		}
		u, err := ParseUUID(v.String())
		if err != nil {
			return &marshalError{fmt.Sprintf("field %q: %v", tag.name, err)}
		}
		return encodeUUID(w, u)
	}

	return encodeReflect(w, v)
}

// ── Tag parsing ──────────────────────────────────────────────────────────────

type fieldTag struct {
	name      string
	skip      bool
	omitempty bool
	decimal   bool
	uuid      bool
}

func parseFieldTag(f reflect.StructField) fieldTag {
	raw := f.Tag.Get("snowed")
	if raw == "-" {
		return fieldTag{skip: true}
	}
	parts := strings.Split(raw, ",")
	name := parts[0]
	if name == "" {
		name = f.Name
	}
	tag := fieldTag{name: name}
	for _, opt := range parts[1:] {
		switch opt {
		case "omitempty":
			tag.omitempty = true
		case "decimal":
			tag.decimal = true
			tag.omitempty = true // empty string decimal makes no sense to encode
		case "uuid":
			tag.uuid = true
		}
	}
	return tag
}

// ── Helpers ──────────────────────────────────────────────────────────────────

func isEmptyValue(v reflect.Value) bool {
	switch v.Kind() {
	case reflect.String:
		return v.Len() == 0
	case reflect.Bool:
		return !v.Bool()
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return v.Int() == 0
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64, reflect.Uintptr:
		return v.Uint() == 0
	case reflect.Float32, reflect.Float64:
		return v.Float() == 0
	case reflect.Slice, reflect.Map:
		return v.IsNil()
	case reflect.Ptr, reflect.Interface:
		return v.IsNil()
	}
	return false
}

// validateDecimalString checks that s conforms to the canonical decimal rules
// in §5.9. It does not parse the number — it validates the string form only.
func validateDecimalString(s string) error {
	if s == "" {
		return &marshalError{"decimal string must not be empty"}
	}
	if !utf8.ValidString(s) {
		return &marshalError{"decimal string must be valid UTF-8"}
	}
	// Reject exponent notation.
	if strings.ContainsAny(s, "eE") {
		return &marshalError{fmt.Sprintf("decimal string %q must not use exponent notation", s)}
	}
	return nil
}
