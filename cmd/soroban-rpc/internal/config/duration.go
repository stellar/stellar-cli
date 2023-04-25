package config

import (
	"fmt"
	"time"
)

type Duration struct {
	time.Duration `toml:"-" valid:"-"`
}

func (d Duration) MarshalTOML() ([]byte, error) {
	return []byte(d.String()), nil
}

func (d *Duration) UnmarshalTOML(i interface{}) error {
	switch v := i.(type) {
	case string:
		var err error
		d.Duration, err = time.ParseDuration(v)
		return err
	default:
		return fmt.Errorf("invalid duration value. Must be a string, like \"30s\"")
	}
}
