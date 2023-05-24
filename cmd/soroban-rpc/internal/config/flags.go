package config

import (
	"fmt"
	"go/types"
	"reflect"

	"github.com/spf13/viper"
	support "github.com/stellar/go/support/config"
)

// Bind binds the config options to viper.
func (cfg *Config) Bind() {
	if cfg.viper == nil {
		cfg.viper = viper.New()
	}
	for _, flag := range cfg.flags() {
		flag.Bind(cfg.viper)
	}
}

func (cfg *Config) flags() support.ConfigOptions {
	if cfg.flagsCache != nil {
		return *cfg.flagsCache
	}
	options := cfg.options()
	flags := make(support.ConfigOptions, 0, len(options))
	for _, option := range options {
		if f := option.flag(cfg); f != nil {
			flags = append(flags, f)
		}
	}
	cfg.flagsCache = &flags
	return flags
}

var optTypes = map[reflect.Kind]types.BasicKind{
	reflect.Bool:    types.Bool,
	reflect.Int:     types.Int,
	reflect.Int8:    types.Int,
	reflect.Int16:   types.Int,
	reflect.Int32:   types.Int,
	reflect.Int64:   types.Int,
	reflect.Uint:    types.Uint,
	reflect.Uint8:   types.Uint,
	reflect.Uint16:  types.Uint,
	reflect.Uint64:  types.Uint,
	reflect.Uint32:  types.Uint32,
	reflect.Float32: types.Float64,
	reflect.Float64: types.Float64,
	reflect.String:  types.String,
}

// Convert our configOption into a CLI flag, if it should be one.
func (o *ConfigOption) flag(cfg *Config) *support.ConfigOption {
	optType := o.OptType
	if optType == types.Invalid {
		// If there was no OptType explicitly set, guess the type based on the
		// target field's type.
		t, found := optTypes[reflect.ValueOf(o.ConfigKey).Elem().Kind()]
		if !found {
			t = types.String
		}
		optType = t
	}

	flagDefault := o.DefaultValue
	if flagDefault != nil && optType == types.String {
		flagDefault = fmt.Sprint(o.DefaultValue)
	}

	f := &support.ConfigOption{
		Name:        o.Name,
		EnvVar:      o.EnvVar,
		OptType:     optType,
		FlagDefault: flagDefault,
		Required:    false,
		Usage:       o.Usage,
		ConfigKey:   o.ConfigKey,
	}
	if o.CustomSetValue != nil {
		f.CustomSetValue = func(co *support.ConfigOption) error {
			return o.CustomSetValue(o, cfg.viper.Get(o.Name))
		}
	}
	return f
}
