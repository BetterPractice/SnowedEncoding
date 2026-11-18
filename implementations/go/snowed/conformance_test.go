package snowed_test

import (
	"bytes"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"testing"

	"snowed/snowed"
)

// Conformance runner for tests/conformance/vectors (see tests/conformance/README.md).

type conformanceSuite struct {
	Suite string            `json:"suite"`
	Cases []conformanceCase `json:"cases"`
}

type conformanceCase struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	Hex         string          `json:"hex"`
	Decoded     json.RawMessage `json:"decoded"`
	Error       bool            `json:"error"`
	Canonical   *bool           `json:"canonical"`
}

// expectedValue is a decoded-value object: {"type": "...", "value": ...}.
// Value holds json.Number for numbers (decoded with UseNumber).
type expectedValue struct {
	Type  string
	Value any
}

func conformanceVectorsDir(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("conformance: runtime.Caller failed; cannot locate vectors directory")
	}
	// implementations/go/snowed/conformance_test.go -> repo root
	root := filepath.Join(filepath.Dir(file), "..", "..", "..")
	return filepath.Join(root, "tests", "conformance", "vectors")
}

func TestConformance(t *testing.T) {
	dir := conformanceVectorsDir(t)
	info, err := os.Stat(dir)
	if err != nil || !info.IsDir() {
		t.Fatalf("conformance: vectors directory %s missing: %v", dir, err)
	}
	files, err := filepath.Glob(filepath.Join(dir, "*.json"))
	if err != nil {
		t.Fatalf("conformance: glob: %v", err)
	}
	if len(files) == 0 {
		t.Fatalf("conformance: no .json vector files in %s", dir)
	}
	sort.Strings(files)

	total := 0
	for _, path := range files {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("conformance: read %s: %v", path, err)
		}
		var suite conformanceSuite
		dec := json.NewDecoder(bytes.NewReader(data))
		dec.UseNumber()
		if err := dec.Decode(&suite); err != nil {
			t.Fatalf("conformance: parse %s: %v", path, err)
		}
		if suite.Suite == "" || len(suite.Cases) == 0 {
			t.Fatalf("conformance: %s has no suite name or no cases", path)
		}
		total += len(suite.Cases)
		t.Run(suite.Suite, func(t *testing.T) {
			for _, c := range suite.Cases {
				c := c
				t.Run(c.Name, func(t *testing.T) {
					runConformanceCase(t, suite.Suite, c)
				})
			}
		})
	}
	if total == 0 {
		t.Fatal("conformance: zero cases found")
	}
}

func runConformanceCase(t *testing.T, suiteName string, c conformanceCase) {
	id := suiteName + "/" + c.Name
	input, err := parseHexBytes(c.Hex)
	if err != nil {
		t.Fatalf("%s: bad hex in vector: %v", id, err)
	}

	if c.Error {
		if len(c.Decoded) != 0 {
			t.Fatalf("%s: vector has both error and decoded", id)
		}
		var v snowed.Value
		if err := snowed.Unmarshal(input, &v); err == nil {
			t.Errorf("%s: expected decode error, got success: %s", id, describeValue(v))
		}
		return
	}

	if len(c.Decoded) == 0 {
		t.Fatalf("%s: vector has neither decoded nor error", id)
	}
	expected, err := parseExpected(c.Decoded)
	if err != nil {
		t.Fatalf("%s: bad decoded in vector: %v", id, err)
	}

	// 1. Decode and compare.
	var got snowed.Value
	if err := snowed.Unmarshal(input, &got); err != nil {
		t.Fatalf("%s: decode failed: %v", id, err)
	}
	if err := compareValue(expected, got, "$"); err != nil {
		t.Fatalf("%s: decoded mismatch: %v", id, err)
	}

	// 2. Semantic round-trip.
	reencoded, err := snowed.Marshal(got)
	if err != nil {
		t.Fatalf("%s: re-encode failed: %v", id, err)
	}
	var again snowed.Value
	if err := snowed.Unmarshal(reencoded, &again); err != nil {
		t.Fatalf("%s: decode of re-encoded bytes failed: %v (bytes % X)", id, err, reencoded)
	}
	if err := compareValue(expected, again, "$"); err != nil {
		t.Errorf("%s: round-trip mismatch: %v (bytes % X)", id, err, reencoded)
	}

	// 3. Canonical byte-exact re-encoding.
	if c.Canonical == nil || *c.Canonical {
		if !bytes.Equal(reencoded, input) {
			t.Errorf("%s: canonical re-encode mismatch\n  got:  % X\n  want: % X", id, reencoded, input)
		}
	}
}

func parseHexBytes(s string) ([]byte, error) {
	s = strings.Join(strings.Fields(s), "")
	if s == "" {
		return []byte{}, nil
	}
	return hex.DecodeString(s)
}

func parseExpected(raw json.RawMessage) (expectedValue, error) {
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber()
	var m map[string]any
	if err := dec.Decode(&m); err != nil {
		return expectedValue{}, err
	}
	return expectedFromMap(m)
}

func expectedFromMap(m map[string]any) (expectedValue, error) {
	typ, ok := m["type"].(string)
	if !ok {
		return expectedValue{}, fmt.Errorf("decoded value missing string \"type\": %v", m)
	}
	val, ok := m["value"]
	if !ok {
		return expectedValue{}, fmt.Errorf("decoded value of type %q missing \"value\"", typ)
	}
	return expectedValue{Type: typ, Value: val}, nil
}

var conformanceTypeNames = map[snowed.ValueType]string{
	snowed.TypeNull:     "null",
	snowed.TypeBool:     "bool",
	snowed.TypeInt64:    "int64",
	snowed.TypeUInt64:   "uint64",
	snowed.TypeFloat64:  "float64",
	snowed.TypeDateTime: "datetime",
	snowed.TypeBinary:   "binary",
	snowed.TypeString:   "string",
	snowed.TypeDecimal:  "decimal",
	snowed.TypeUUID:     "uuid",
	snowed.TypeArray:    "array",
	snowed.TypeObject:   "object",
}

func typeName(t snowed.ValueType) string {
	if n, ok := conformanceTypeNames[t]; ok {
		return n
	}
	return fmt.Sprintf("unknown(0x%02X)", uint8(t))
}

func describeValue(v snowed.Value) string {
	return fmt.Sprintf("%s %+v", typeName(v.Type), v)
}

func compareValue(exp expectedValue, got snowed.Value, path string) error {
	if typeName(got.Type) != exp.Type {
		return fmt.Errorf("%s: type: got %s, want %s", path, typeName(got.Type), exp.Type)
	}
	switch exp.Type {
	case "null":
		if exp.Value != nil {
			return fmt.Errorf("%s: vector null value must be JSON null, got %v", path, exp.Value)
		}
		return nil

	case "bool":
		b, ok := exp.Value.(bool)
		if !ok {
			return fmt.Errorf("%s: vector bool value is %T", path, exp.Value)
		}
		if got.Bool != b {
			return fmt.Errorf("%s: bool: got %v, want %v", path, got.Bool, b)
		}
		return nil

	case "int64":
		s, err := integerText(exp.Value)
		if err != nil {
			return fmt.Errorf("%s: %v", path, err)
		}
		want, err := strconv.ParseInt(s, 10, 64)
		if err != nil {
			return fmt.Errorf("%s: vector int64 %q: %v", path, s, err)
		}
		if got.Int64 != want {
			return fmt.Errorf("%s: int64: got %d, want %d", path, got.Int64, want)
		}
		return nil

	case "uint64":
		s, err := integerText(exp.Value)
		if err != nil {
			return fmt.Errorf("%s: %v", path, err)
		}
		want, err := strconv.ParseUint(s, 10, 64)
		if err != nil {
			return fmt.Errorf("%s: vector uint64 %q: %v", path, s, err)
		}
		if got.UInt64 != want {
			return fmt.Errorf("%s: uint64: got %d, want %d", path, got.UInt64, want)
		}
		return nil

	case "float64":
		return compareFloat(exp.Value, got.Float64, path)

	case "datetime":
		return compareFloat(exp.Value, got.DateTime, path)

	case "binary":
		s, ok := exp.Value.(string)
		if !ok {
			return fmt.Errorf("%s: vector binary value is %T", path, exp.Value)
		}
		want, err := base64.StdEncoding.DecodeString(s)
		if err != nil {
			return fmt.Errorf("%s: vector binary base64 %q: %v", path, s, err)
		}
		if !bytes.Equal(got.Binary, want) {
			return fmt.Errorf("%s: binary: got % X, want % X", path, got.Binary, want)
		}
		return nil

	case "string":
		s, ok := exp.Value.(string)
		if !ok {
			return fmt.Errorf("%s: vector string value is %T", path, exp.Value)
		}
		if got.String != s {
			return fmt.Errorf("%s: string: got %q, want %q", path, got.String, s)
		}
		return nil

	case "decimal":
		s, ok := exp.Value.(string)
		if !ok {
			return fmt.Errorf("%s: vector decimal value is %T", path, exp.Value)
		}
		if got.Decimal != s {
			return fmt.Errorf("%s: decimal: got %q, want %q", path, got.Decimal, s)
		}
		return nil

	case "uuid":
		s, ok := exp.Value.(string)
		if !ok {
			return fmt.Errorf("%s: vector uuid value is %T", path, exp.Value)
		}
		if g := got.UUID.String(); g != s {
			return fmt.Errorf("%s: uuid: got %s, want %s", path, g, s)
		}
		return nil

	case "array":
		items, ok := exp.Value.([]any)
		if !ok {
			return fmt.Errorf("%s: vector array value is %T", path, exp.Value)
		}
		if len(got.Array) != len(items) {
			return fmt.Errorf("%s: array length: got %d, want %d", path, len(got.Array), len(items))
		}
		for i, item := range items {
			m, ok := item.(map[string]any)
			if !ok {
				return fmt.Errorf("%s[%d]: vector element is %T", path, i, item)
			}
			e, err := expectedFromMap(m)
			if err != nil {
				return fmt.Errorf("%s[%d]: %v", path, i, err)
			}
			if err := compareValue(e, got.Array[i], fmt.Sprintf("%s[%d]", path, i)); err != nil {
				return err
			}
		}
		return nil

	case "object":
		fields, ok := exp.Value.(map[string]any)
		if !ok {
			return fmt.Errorf("%s: vector object value is %T", path, exp.Value)
		}
		if len(got.Object) != len(fields) {
			return fmt.Errorf("%s: object size: got %d keys %v, want %d keys %v",
				path, len(got.Object), sortedKeys(got.Object), len(fields), sortedKeysAny(fields))
		}
		for k, item := range fields {
			sub := fmt.Sprintf("%s[%q]", path, k)
			gv, ok := got.Object[k]
			if !ok {
				return fmt.Errorf("%s: missing key", sub)
			}
			m, ok := item.(map[string]any)
			if !ok {
				return fmt.Errorf("%s: vector field is %T", sub, item)
			}
			e, err := expectedFromMap(m)
			if err != nil {
				return fmt.Errorf("%s: %v", sub, err)
			}
			if err := compareValue(e, gv, sub); err != nil {
				return err
			}
		}
		return nil
	}
	return fmt.Errorf("%s: unknown vector type %q", path, exp.Type)
}

// integerText returns the exact decimal text of a JSON number or string-form integer.
func integerText(v any) (string, error) {
	switch x := v.(type) {
	case json.Number:
		return x.String(), nil
	case string:
		return x, nil
	}
	return "", fmt.Errorf("vector integer value is %T", v)
}

func compareFloat(v any, got float64, path string) error {
	switch x := v.(type) {
	case string:
		switch x {
		case "NaN":
			if !math.IsNaN(got) {
				return fmt.Errorf("%s: float: got %v, want NaN", path, got)
			}
			return nil
		case "Infinity":
			if !math.IsInf(got, 1) {
				return fmt.Errorf("%s: float: got %v, want +Inf", path, got)
			}
			return nil
		case "-Infinity":
			if !math.IsInf(got, -1) {
				return fmt.Errorf("%s: float: got %v, want -Inf", path, got)
			}
			return nil
		case "-0":
			if got != 0 || !math.Signbit(got) {
				return fmt.Errorf("%s: float: got %v (signbit=%v), want -0", path, got, math.Signbit(got))
			}
			return nil
		}
		return fmt.Errorf("%s: unknown float string %q in vector", path, x)
	case json.Number:
		want, err := strconv.ParseFloat(x.String(), 64)
		if err != nil {
			return fmt.Errorf("%s: vector float %q: %v", path, x, err)
		}
		if math.Float64bits(got) != math.Float64bits(want) {
			return fmt.Errorf("%s: float: got %v (0x%016X), want %v (0x%016X)",
				path, got, math.Float64bits(got), want, math.Float64bits(want))
		}
		return nil
	}
	return fmt.Errorf("%s: vector float value is %T", path, v)
}

func sortedKeys(m map[string]snowed.Value) []string {
	ks := make([]string, 0, len(m))
	for k := range m {
		ks = append(ks, k)
	}
	sort.Strings(ks)
	return ks
}

func sortedKeysAny(m map[string]any) []string {
	ks := make([]string, 0, len(m))
	for k := range m {
		ks = append(ks, k)
	}
	sort.Strings(ks)
	return ks
}
