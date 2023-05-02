package config

import (
	"fmt"
	"go/types"

	"github.com/spf13/viper"
	support "github.com/stellar/go/support/config"
)

// Bind binds the config options to viper.
func (cfg *Config) Bind() {
	for _, flag := range cfg.flags() {
		flag.Bind()
	}
}

func (cfg *Config) flags() support.ConfigOptions {
	if cfg.flagsCache != nil {
		return *cfg.flagsCache
	}
	options := cfg.options()
	flags := make(support.ConfigOptions, 0, len(options))
	for _, option := range options {
		if f := option.flag(); f != nil {
			flags = append(flags, f)
		}
	}
	cfg.flagsCache = &flags
	return flags
}

// Convert our configOption into a CLI flag, if it should be one.
func (o *ConfigOption) flag() *support.ConfigOption {
	flagDefault := o.DefaultValue
	if flagDefault != nil {
		switch o.OptType {
		case types.String:
			flagDefault = fmt.Sprint(flagDefault)
		}
	}
	f := &support.ConfigOption{
		Name:        o.Name,
		EnvVar:      o.EnvVar,
		OptType:     o.OptType,
		FlagDefault: flagDefault,
		Required:    false,
		Usage:       o.Usage,
		ConfigKey:   o.ConfigKey,
	}
	if o.CustomSetValue != nil {
		f.CustomSetValue = func(co *support.ConfigOption) error {
			return o.CustomSetValue(o, viper.Get(o.Name))
		}
	}
	return f
}
