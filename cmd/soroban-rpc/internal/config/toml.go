package config

import (
	"fmt"
	"io"
	"reflect"
	"strings"
	"unicode"
	"unicode/utf8"

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
				return fmt.Errorf("Invalid config: unexpected entry specified in toml file %q", key)
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

func wordWrap(text string, lineWidth int) string {
	wrap := make([]byte, 0, len(text)+2*len(text)/lineWidth)
	eoLine := lineWidth
	inWord := false
	for i, j := 0, 0; ; {
		r, size := utf8.DecodeRuneInString(text[i:])
		if size == 0 && r == utf8.RuneError {
			r = ' '
		}
		if unicode.IsSpace(r) {
			if inWord {
				if i >= eoLine {
					wrap = append(wrap, '\n')
					eoLine = len(wrap) + lineWidth
				} else if len(wrap) > 0 {
					wrap = append(wrap, ' ')
				}
				wrap = append(wrap, text[j:i]...)
			}
			inWord = false
		} else if !inWord {
			inWord = true
			j = i
		}
		if size == 0 && r == ' ' {
			break
		}
		i += size
	}
	return string(wrap)
}
