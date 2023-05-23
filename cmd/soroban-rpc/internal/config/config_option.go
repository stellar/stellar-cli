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

// Returns false if this option is omitted in the toml
func (o ConfigOption) getTomlKey() (string, bool) {
	if o.TomlKey == "-" || o.TomlKey == "_" {
		return "", false
	}
	if o.TomlKey != "" {
		return o.TomlKey, true
	}
	if o.EnvVar != "" && o.EnvVar != "-" {
		return o.EnvVar, true
	}
	return strings.ToUpper(strings.ReplaceAll(o.Name, "-", "_")), true
}

// TODO: See if we can remove CustomSetValue into just SetValue/ParseValue
func (o *ConfigOption) setValue(i interface{}) (err error) {
	if o.CustomSetValue != nil {
		return o.CustomSetValue(o, i)
	}
	// it's unfortunate that Set below panics when it cannot set the value..
	// we'll want to catch this so that we can alert the user nicely.
	defer func() {
		if recoverRes := recover(); recoverRes != nil {
			var ok bool
			if err, ok = recoverRes.(error); ok {
				return
			}

			err = errors.Errorf("config option setting error ('%s') %v", o.Name, recoverRes)
		}
	}()
	parser := func(option *ConfigOption, i interface{}) error {
		panic(fmt.Sprintf("no parser for flag %s", o.Name))
	}
	switch reflect.ValueOf(o.ConfigKey).Elem().Kind() {
	case reflect.Bool:
		parser = parseBool
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		parser = parseInt
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32:
		parser = parseUint32
	case reflect.Uint64:
		parser = parseUint
	case reflect.Float32, reflect.Float64:
		parser = parseFloat
	case reflect.String:
		parser = parseString
	}

	return parser(o, i)
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
