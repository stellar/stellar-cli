package config

import (
	"fmt"
	"go/types"
	"reflect"
	"strconv"
	"strings"

	"github.com/stellar/go/support/errors"
)

// ConfigOptions is a group of ConfigOptions that can be for convenience
// initialized and set at the same time.
type ConfigOptions []*ConfigOption

// Validate all the config options.
func (options ConfigOptions) Validate() error {
	for _, option := range options {
		if option.Validate != nil {
			err := option.Validate(option)
			if err != nil {
				return errors.Wrap(err, fmt.Sprintf("Invalid config value for %s", option.Name))
			}
		}
	}
	return nil
}

// ConfigOption is a complete description of the configuration of a command line option
type ConfigOption struct {
	Name           string                                 // e.g. "database-url"
	EnvVar         string                                 // e.g. "DATABASE_URL". Defaults to uppercase/underscore representation of name
	TomlKey        string                                 // e.g. "DATABASE_URL". Defaults to uppercase/underscore representation of name. - to omit from toml
	Usage          string                                 // Help text
	OptType        types.BasicKind                        // The type of this option, e.g. types.Bool
	DefaultValue   interface{}                            // A default if no option is provided. Omit or set to `nil` if no default
	ConfigKey      interface{}                            // Pointer to the final key in the linked Config struct
	CustomSetValue func(*ConfigOption, interface{}) error // Optional function for custom validation/transformation
	Validate       func(*ConfigOption) error              // Function called after loading all options, to validate the configuration
	MarshalTOML    func(*ConfigOption) (interface{}, error)
}

func (o ConfigOption) getTomlKey() (string, bool) {
	if o.TomlKey != "" {
		return o.TomlKey, true
	}
	if o.EnvVar != "" && o.EnvVar != "-" {
		return o.EnvVar, true
	}
	return strings.ToUpper(strings.ReplaceAll(o.Name, "-", "_")), true
}

// TODO: See if we can combine OptType and CustomSetValue into just SetValue/ParseValue
func (o *ConfigOption) setValue(i interface{}) error {
	if o.CustomSetValue != nil {
		return o.CustomSetValue(o, i)
	}
	reflect.ValueOf(o.ConfigKey).Elem().Set(reflect.ValueOf(i))
	return nil
}

func (o *ConfigOption) marshalTOML() (interface{}, error) {
	if o.MarshalTOML != nil {
		return o.MarshalTOML(o)
	}
	// go-toml doesn't handle ints other than `int`, so we have to do that ourselves.
	switch v := o.ConfigKey.(type) {
	case *int, *int8, *int16, *int32, *int64:
		return []byte(strconv.FormatInt(reflect.ValueOf(v).Elem().Int(), 10)), nil
	case *uint, *uint8, *uint16, *uint32, *uint64:
		return []byte(strconv.FormatUint(reflect.ValueOf(v).Elem().Uint(), 10)), nil
	default:
		// Unknown, hopefully go-toml knows what to do with it! :crossed_fingers:
		return reflect.ValueOf(o.ConfigKey).Elem().Interface(), nil
	}
}
