package config

import (
	"fmt"
	"math"
	"reflect"
	"strconv"
	"strings"
	"time"

	"github.com/stellar/go/support/errors"
)

func parseBool(option *ConfigOption, i interface{}) error {
	switch v := i.(type) {
	case nil:
		return nil
	case bool:
		*option.ConfigKey.(*bool) = v
	case string:
		lower := strings.ToLower(v)
		if lower == "true" {
			*option.ConfigKey.(*bool) = true
		} else if lower == "false" {
			*option.ConfigKey.(*bool) = false
		} else {
			return fmt.Errorf("invalid boolean value %s: %s", option.Name, v)
		}
	default:
		return fmt.Errorf("could not parse boolean %s: %v", option.Name, i)
	}
	return nil
}

func parseInt(option *ConfigOption, i interface{}) error {
	switch v := i.(type) {
	case nil:
		return nil
	case string:
		parsed, err := strconv.ParseInt(v, 10, 64)
		if err != nil {
			return err
		}
		reflect.ValueOf(option.ConfigKey).Elem().SetInt(parsed)
	case int, int8, int16, int32, int64, uint, uint8, uint16, uint32, uint64:
		return parseInt(option, fmt.Sprint(v))
	default:
		return fmt.Errorf("could not parse int %s: %v", option.Name, i)
	}
	return nil
}

func parseUint(option *ConfigOption, i interface{}) error {
	switch v := i.(type) {
	case nil:
		return nil
	case string:
		parsed, err := strconv.ParseUint(v, 10, 64)
		if err != nil {
			return err
		}
		reflect.ValueOf(option.ConfigKey).Elem().SetUint(parsed)
	case int, int8, int16, int32, int64:
		if reflect.ValueOf(v).Int() < 0 {
			return fmt.Errorf("%s cannot be negative", option.Name)
		}
		return parseUint(option, fmt.Sprint(v))
	case uint, uint8, uint16, uint32, uint64:
		return parseUint(option, fmt.Sprint(v))
	default:
		return fmt.Errorf("could not parse uint %s: %v", option.Name, i)
	}
	return nil
}

func parseFloat(option *ConfigOption, i interface{}) error {
	switch v := i.(type) {
	case nil:
		return nil
	case string:
		parsed, err := strconv.ParseFloat(v, 64)
		if err != nil {
			return err
		}
		reflect.ValueOf(option.ConfigKey).Elem().SetFloat(parsed)
	case uint, uint8, uint16, uint32, uint64, int, int8, int16, int32, int64, float32, float64:
		return parseFloat(option, fmt.Sprint(v))
	default:
		return fmt.Errorf("could not parse float %s: %v", option.Name, i)
	}
	return nil
}

func parseString(option *ConfigOption, i interface{}) error {
	switch v := i.(type) {
	case nil:
		return nil
	case string:
		*option.ConfigKey.(*string) = v
	default:
		return fmt.Errorf("could not parse string %s: %v", option.Name, i)
	}
	return nil
}

func parseUint32(option *ConfigOption, i interface{}) error {
	switch v := i.(type) {
	case nil:
		return nil
	case string:
		parsed, err := strconv.ParseUint(v, 10, 64)
		if err != nil {
			return err
		}
		if parsed > math.MaxUint32 {
			return fmt.Errorf("%s overflows uint32", option.Name)
		}
		reflect.ValueOf(option.ConfigKey).Elem().SetUint(parsed)
	case int, int8, int16, int32, int64:
		if reflect.ValueOf(v).Int() < 0 {
			return fmt.Errorf("%s cannot be negative", option.Name)
		}
		return parseUint32(option, fmt.Sprint(v))
	case uint, uint8, uint16, uint32, uint64:
		return parseUint32(option, fmt.Sprint(v))
	default:
		return fmt.Errorf("could not parse uint32 %s: %v", option.Name, i)
	}
	return nil
}

func parseDuration(option *ConfigOption, i interface{}) error {
	switch v := i.(type) {
	case nil:
		return nil
	case string:
		d, err := time.ParseDuration(v)
		if err != nil {
			return errors.Wrapf(err, "could not parse duration: %q", v)
		}
		*option.ConfigKey.(*time.Duration) = d
	case time.Duration:
		*option.ConfigKey.(*time.Duration) = v
	case *time.Duration:
		*option.ConfigKey.(*time.Duration) = *v
	default:
		return fmt.Errorf("%s is not a duration", option.Name)
	}
	return nil
}

func parseStringSlice(option *ConfigOption, i interface{}) error {
	switch v := i.(type) {
	case nil:
		return nil
	case string:
		if v == "" {
			*option.ConfigKey.(*[]string) = nil
		} else {
			*option.ConfigKey.(*[]string) = strings.Split(v, ",")
		}
		return nil
	case []string:
		*option.ConfigKey.(*[]string) = v
		return nil
	case []interface{}:
		*option.ConfigKey.(*[]string) = make([]string, len(v))
		for i, s := range v {
			switch s := s.(type) {
			case string:
				(*option.ConfigKey.(*[]string))[i] = s
			default:
				return fmt.Errorf("could not parse %s: %v", option.Name, v)
			}
		}
		return nil
	default:
		return fmt.Errorf("could not parse %s: %v", option.Name, v)
	}
}
