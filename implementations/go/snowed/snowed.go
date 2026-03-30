// Package snowed implements the Snowed binary serialization format.
//
// # Encoding
//
// Use [Marshal] to encode a Go value to Snowed bytes, or [NewEncoder] to write
// directly to an [io.Writer].
//
//	data, err := snowed.Marshal(myStruct)
//
// # Decoding
//
// Use [Unmarshal] to decode Snowed bytes into a Go value, or [NewDecoder] to
// read from an [io.Reader].
//
//	var out MyStruct
//	err := snowed.Unmarshal(data, &out)
//
// # Struct tags
//
// Struct fields are mapped to Snowed object keys using the `snowed` tag:
//
//	type Record struct {
//	    ID     string    `snowed:"id,uuid"`        // encode/decode as UUID (0x13)
//	    Amount string    `snowed:"amount,decimal"` // encode/decode as Decimal (0x12)
//	    Name   string    `snowed:"name"`           // encode/decode as String (0x11)
//	    Note   string    `snowed:"note,omitempty"` // omit if empty string
//	    Hidden string    `snowed:"-"`              // always skip
//	}
//
// # Type mapping
//
//	Go type          → Snowed type
//	bool             → Bool (0x02/0x03)
//	int*             → Int64 (0x04)
//	uint*            → UInt64 (0x05)
//	float32/float64  → Float64 (0x08)
//	time.Time        → DateTime (0x09) — always UTC
//	[]byte           → Binary (0x10)
//	string           → String (0x11)
//	string + decimal → Decimal (0x12)
//	snowed.UUID      → UUID (0x13)
//	string + uuid    → UUID (0x13)
//	slice/array      → Array (0x40)
//	struct/map       → Object (0x80)
//	nil pointer      → Null (0x00)
package snowed

import (
	"bytes"
	"io"
	"reflect"
)

// Marshaler is the interface implemented by types that can encode themselves
// into Snowed binary format. The returned bytes must be a complete, valid
// Snowed value.
type Marshaler interface {
	MarshalSnowed() ([]byte, error)
}

// Unmarshaler is the interface implemented by types that can decode themselves
// from Snowed binary format.
type Unmarshaler interface {
	UnmarshalSnowed([]byte) error
}

// Marshal encodes v into Snowed binary format and returns the bytes.
func Marshal(v any) ([]byte, error) {
	var buf bytes.Buffer
	enc := NewEncoder(&buf)
	if err := enc.Encode(v); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// Unmarshal decodes Snowed binary data into v, which must be a non-nil pointer.
// Trailing bytes after the root value are rejected.
func Unmarshal(data []byte, v any) error {
	dec := NewDecoder(bytes.NewReader(data))
	if err := dec.Decode(v); err != nil {
		return err
	}
	// Ensure no trailing bytes remain.
	var probe [1]byte
	if n, _ := dec.r.Read(probe[:]); n > 0 {
		return &unmarshalError{"trailing bytes after root value"}
	}
	return nil
}

// Encoder writes Snowed-encoded values to an output stream.
type Encoder struct {
	w io.Writer
}

// NewEncoder returns a new Encoder that writes to w.
func NewEncoder(w io.Writer) *Encoder { return &Encoder{w: w} }

// Encode encodes v and writes the Snowed encoding to the stream.
func (e *Encoder) Encode(v any) error {
	// snowed.Value goes through the value path directly.
	if sv, ok := v.(Value); ok {
		return encodeValue(e.w, sv)
	}
	return encodeReflect(e.w, reflect.ValueOf(v))
}

// Decoder reads and decodes Snowed-encoded values from an input stream.
type Decoder struct {
	r io.Reader
}

// NewDecoder returns a new Decoder that reads from r.
func NewDecoder(r io.Reader) *Decoder { return &Decoder{r: r} }

// Decode reads one Snowed-encoded value from the stream and stores the result
// in v, which must be a non-nil pointer.
func (d *Decoder) Decode(v any) error {
	// Buffer the entire remaining input so we can use the slice-based reader.
	// This matches the approach of encoding/json and keeps the decoder simple.
	data, err := io.ReadAll(d.r)
	if err != nil {
		return err
	}
	// Wrap unread bytes back so the caller can detect trailing data.
	r := newReader(data)

	if sv, ok := v.(*Value); ok {
		val, err := decodeValue(r)
		if err != nil {
			return err
		}
		*sv = val
		// Wrap remaining bytes back into d.r.
		d.r = bytes.NewReader(data[r.pos:])
		return nil
	}

	rv := reflect.ValueOf(v)
	if rv.Kind() != reflect.Ptr || rv.IsNil() {
		return &unmarshalError{"Decode requires a non-nil pointer"}
	}
	if err := decodeInto(r, rv.Elem()); err != nil {
		return err
	}
	d.r = bytes.NewReader(data[r.pos:])
	return nil
}
