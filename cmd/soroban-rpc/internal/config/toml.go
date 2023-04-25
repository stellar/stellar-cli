package config

import (
	"fmt"
	"io"
	"reflect"

	"github.com/pelletier/go-toml"
)

func parseToml(r io.Reader, strict bool, cfg *Config) error {
	tree, err := toml.LoadReader(r)
	if err != nil {
		return err
	}

	validKeys := map[string]struct{}{}
	for _, option := range cfg.options() {
		key := option.getTomlKey()
		if key == "-" {
			continue
		}
		validKeys[key] = struct{}{}
		value := tree.Get(key)
		if value == nil {
			// not found
			continue
		}
		if err := option.setValue(value); err != nil {
			return err
		}
	}

	if cfg.Strict || strict {
		for _, key := range tree.Keys() {
			if _, ok := validKeys[key]; !ok {
				return fmt.Errorf("Invalid config: unknown field %q", key)
			}
		}
	}

	return nil
}

func (cfg *Config) MarshalTOML() ([]byte, error) {
	tree, err := toml.TreeFromMap(map[string]interface{}{})
	if err != nil {
		return nil, err
	}

	// tomlMarshalerType := reflect.TypeOf((*toml.Marshaler)(nil)).Elem()
	for _, option := range cfg.options() {
		key := option.getTomlKey()
		if key == "-" {
			continue
		}

		// Downcast a couple primitive types which are not directly supported by the toml encoder
		// For non-primitives, you should implement toml.Marshaler instead.
		value, err := option.marshalTOML()
		if err != nil {
			return nil, err
		}

		if m, ok := value.(toml.Marshaler); ok {
			value, err = m.MarshalTOML()
			if err != nil {
				return nil, err
			}
		}

		tree.SetWithOptions(
			key,
			toml.SetOptions{
				// TODO: line-wrap this, the toml library will auto-comment it, we just
				// need to split it on whitespace every x chars
				Comment: option.Usage,
				// output unset values commented out
				// TODO: Provide commented example values for these
				Commented: reflect.ValueOf(option.ConfigKey).Elem().IsZero(),
			},
			value,
		)
	}

	return tree.Marshal()
}
