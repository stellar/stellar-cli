package config

import (
	"github.com/spf13/viper"
	support "github.com/stellar/go/support/config"
	"github.com/stellar/go/support/errors"
)

func (cfg *Config) flags() support.ConfigOptions {
	options := cfg.options()
	flags := make([]*support.ConfigOption, 0, len(options))
	for _, option := range options {
		if f := option.flag(); f != nil {
			flags = append(flags, f)
		}
	}
	return flags
}

// Convert our configOption into a CLI flag, if it should be one.
func (o *ConfigOption) flag() *support.ConfigOption {
	f := &support.ConfigOption{
		Name:        o.Name,
		EnvVar:      o.EnvVar,
		OptType:     o.OptType,
		FlagDefault: o.DefaultValue,
		Required:    false,
		Usage:       o.Usage,
		ConfigKey:   o.ConfigKey,
	}
	if o.CustomSetValue != nil {
		f.CustomSetValue = func(co *support.ConfigOption) error {
			return errors.Wrapf(
				o.CustomSetValue(viper.Get(co.Name)),
				"unable to parse %s", co.Name,
			)
		}
	}
	return f
}
