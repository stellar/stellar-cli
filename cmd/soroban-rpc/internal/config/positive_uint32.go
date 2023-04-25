package config

import (
	"fmt"
	"math"
)

type PositiveUint32 struct {
	Value uint32
}

func (u *PositiveUint32) MarshalTOML() ([]byte, error) {
	return []byte(fmt.Sprintf("%d", u.Value)), nil
}

func (u *PositiveUint32) UnmarshalTOML(i interface{}) error {
	switch v := i.(type) {
	case uint32:
		u.Value = v
		return nil
	case int:
		if v <= 0 {
			return fmt.Errorf("value must be positive")
		}
		if v > math.MaxUint32 {
			return fmt.Errorf("value is too large (must be <= %d)", math.MaxUint32)
		}
		u.Value = uint32(v)
		return nil
	default:
		return fmt.Errorf("could not parse positive uint32: %v", v)
	}
}
