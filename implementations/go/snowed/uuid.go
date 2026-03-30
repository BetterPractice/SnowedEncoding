package snowed

import (
	"encoding/hex"
	"errors"
	"strings"
)

// UUID is a 128-bit UUID stored in RFC 4122 byte order (big-endian throughout).
// The 16 bytes correspond directly to the canonical xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
// string representation read left to right with hyphens removed.
//
// Third-party UUID library values that are backed by [16]byte can be converted
// with a direct cast: snowed.UUID(myLib.UUID).
type UUID [16]byte

// ErrInvalidUUID is returned when a string cannot be parsed as a UUID.
var ErrInvalidUUID = errors.New("snowed: invalid UUID string")

// Parse parses a UUID from the canonical xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
// format. Both uppercase and lowercase hex digits are accepted.
func ParseUUID(s string) (UUID, error) {
	// Accept both with and without hyphens, normalise to no-hyphen for parsing.
	s = strings.ToLower(s)
	switch len(s) {
	case 36:
		// xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
		if s[8] != '-' || s[13] != '-' || s[18] != '-' || s[23] != '-' {
			return UUID{}, ErrInvalidUUID
		}
		s = s[0:8] + s[9:13] + s[14:18] + s[19:23] + s[24:36]
	case 32:
		// already no hyphens
	default:
		return UUID{}, ErrInvalidUUID
	}

	b, err := hex.DecodeString(s)
	if err != nil || len(b) != 16 {
		return UUID{}, ErrInvalidUUID
	}
	var u UUID
	copy(u[:], b)
	return u, nil
}

// String returns the canonical lowercase xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
// representation of the UUID.
func (u UUID) String() string {
	b := u[:]
	return hex.EncodeToString(b[0:4]) + "-" +
		hex.EncodeToString(b[4:6]) + "-" +
		hex.EncodeToString(b[6:8]) + "-" +
		hex.EncodeToString(b[8:10]) + "-" +
		hex.EncodeToString(b[10:16])
}
