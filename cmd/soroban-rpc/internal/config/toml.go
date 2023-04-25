package config

import (
	"io"

	"github.com/pelletier/go-toml"
)

func parseToml(r io.Reader, strict bool, cfg *Config) error {
	tree, err := toml.LoadReader(r)
	if err != nil {
		return err
	}

	for _, option := range cfg.options() {
		key := option.getTomlKey()
		if key == "-" {
			continue
		}
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
		// TODO: Enforce strict mode here
	}

	return nil
}

func (cfg *Config) marshalToml(w io.Writer) error {
	var tree toml.Tree
	tree.Set("FOO", "bar")
	return toml.NewEncoder(w).Encode(tree)
}
