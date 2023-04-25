package config

import (
	"fmt"
)

type PositiveUint struct {
	Value uint
}

func (u *PositiveUint) MarshalTOML() ([]byte, error) {
	return []byte(fmt.Sprintf("%d", u.Value)), nil
}

func (u *PositiveUint) UnmarshalTOML(i interface{}) error {
	switch v := i.(type) {
	case uint:
		if v <= 0 {
			return fmt.Errorf("value must be positive")
		}
		u.Value = v
		return nil
	case int:
		if v <= 0 {
			return fmt.Errorf("value must be positive")
		}
		u.Value = uint(v)
		return nil
	default:
		return fmt.Errorf("could not parse positive uint: %v", v)
	}
}
