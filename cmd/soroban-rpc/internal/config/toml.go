package config

import (
	"fmt"
	"io"
	"reflect"
	"strings"

	"github.com/pelletier/go-toml"
)

func parseToml(r io.Reader, strict bool, cfg *Config) error {
	tree, err := toml.LoadReader(r)
	if err != nil {
		return err
	}

	validKeys := map[string]struct{}{}
	for _, option := range cfg.options() {
		key, ok := option.getTomlKey()
		if !ok {
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
				return fmt.Errorf("invalid config: unexpected entry specified in toml file %q", key)
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

	for _, option := range cfg.options() {
		key, ok := option.getTomlKey()
		if !ok {
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
				Comment: strings.ReplaceAll(
					wordWrap(option.Usage, 80-2),
					"\n",
					"\n ",
				),
				// output unset values commented out
				// TODO: Provide commented example values for these
				Commented: reflect.ValueOf(option.ConfigKey).Elem().IsZero(),
			},
			value,
		)
	}

	return tree.Marshal()
}

// From https://gist.github.com/kennwhite/306317d81ab4a885a965e25aa835b8ef
func wordWrap(text string, lineWidth int) string {
	words := strings.Fields(strings.TrimSpace(text))
	if len(words) == 0 {
		return text
	}
	wrapped := words[0]
	spaceLeft := lineWidth - len(wrapped)
	for _, word := range words[1:] {
		if len(word)+1 > spaceLeft {
			wrapped += "\n" + word
			spaceLeft = lineWidth - len(word)
		} else {
			wrapped += " " + word
			spaceLeft -= 1 + len(word)
		}
	}
	return wrapped
}
