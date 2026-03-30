package snowed

// Value is a type-safe representation of any Snowed value. It is used when
// marshalling or unmarshalling without a known Go struct type.
//
// Exactly one of the fields is set; all others are their zero value. Use the
// Type field to determine which one.
type Value struct {
	Type ValueType

	// Set when Type == TypeNull — no additional fields needed.

	// Set when Type == TypeBool.
	Bool bool

	// Set when Type == TypeInt64.
	Int64 int64

	// Set when Type == TypeUInt64.
	UInt64 uint64

	// Set when Type == TypeFloat64.
	Float64 float64

	// Set when Type == TypeDateTime. Milliseconds since Unix epoch (UTC).
	DateTime float64

	// Set when Type == TypeBinary.
	Binary []byte

	// Set when Type == TypeString.
	String string

	// Set when Type == TypeDecimal. Canonical decimal string per §5.9.
	Decimal string

	// Set when Type == TypeUUID.
	UUID UUID

	// Set when Type == TypeArray.
	Array []Value

	// Set when Type == TypeObject. Keys are UTF-8 strings; order is
	// not significant on decode but is deterministic (sorted) on encode.
	Object map[string]Value
}

// ValueType identifies which Snowed type a Value holds.
type ValueType uint8

const (
	TypeNull     ValueType = 0x00
	TypeBool     ValueType = 0x02 // 0x02 false / 0x03 true — we use 0x02 as the canonical kind
	TypeInt64    ValueType = 0x04
	TypeUInt64   ValueType = 0x05
	TypeFloat64  ValueType = 0x08
	TypeDateTime ValueType = 0x09
	TypeBinary   ValueType = 0x10
	TypeString   ValueType = 0x11
	TypeDecimal  ValueType = 0x12
	TypeUUID     ValueType = 0x13
	TypeArray    ValueType = 0x40
	TypeObject   ValueType = 0x80
)

// Convenience constructors.

func NullValue() Value                  { return Value{Type: TypeNull} }
func BoolValue(b bool) Value            { return Value{Type: TypeBool, Bool: b} }
func Int64Value(v int64) Value          { return Value{Type: TypeInt64, Int64: v} }
func UInt64Value(v uint64) Value        { return Value{Type: TypeUInt64, UInt64: v} }
func Float64Value(v float64) Value      { return Value{Type: TypeFloat64, Float64: v} }
func DateTimeValue(ms float64) Value    { return Value{Type: TypeDateTime, DateTime: ms} }
func BinaryValue(b []byte) Value        { return Value{Type: TypeBinary, Binary: b} }
func StringValue(s string) Value        { return Value{Type: TypeString, String: s} }
func DecimalValue(s string) Value       { return Value{Type: TypeDecimal, Decimal: s} }
func UUIDValue(u UUID) Value            { return Value{Type: TypeUUID, UUID: u} }
func ArrayValue(v []Value) Value        { return Value{Type: TypeArray, Array: v} }
func ObjectValue(m map[string]Value) Value { return Value{Type: TypeObject, Object: m} }
