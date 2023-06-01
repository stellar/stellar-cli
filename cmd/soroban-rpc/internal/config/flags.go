package config

import (
	"fmt"
	"net"
	"time"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

// Init adds the CLI flags to the command. This lets the command output the
// flags as part of the --help output.
func (cfg *Config) AddFlags(cmd *cobra.Command) error {
	cfg.flagset = cmd.PersistentFlags()
	for _, option := range cfg.options() {
		if err := option.AddFlag(cfg.flagset); err != nil {
			return err
		}
	}
	return nil
}

// AddFlag adds a CLI flag for this option to the given flagset.
func (co *ConfigOption) AddFlag(flagset *pflag.FlagSet) error {
	// Treat any option with a custom parser as a string option.
	if co.CustomSetValue != nil {
		if co.DefaultValue == nil {
			co.DefaultValue = ""
		}
		flagset.String(co.Name, fmt.Sprint(co.DefaultValue), co.UsageText())
		co.flag = flagset.Lookup(co.Name)
		return nil
	}

	// Infer the type of the flag based on the type of the ConfigKey. This list
	// of options is based on the available flag types from pflags
	switch co.ConfigKey.(type) {
	case *bool:
		flagset.Bool(co.Name, co.DefaultValue.(bool), co.UsageText())
	case *time.Duration:
		flagset.Duration(co.Name, co.DefaultValue.(time.Duration), co.UsageText())
	case *float32:
		flagset.Float32(co.Name, co.DefaultValue.(float32), co.UsageText())
	case *float64:
		flagset.Float64(co.Name, co.DefaultValue.(float64), co.UsageText())
	case *net.IP:
		flagset.IP(co.Name, co.DefaultValue.(net.IP), co.UsageText())
	case *net.IPNet:
		flagset.IPNet(co.Name, co.DefaultValue.(net.IPNet), co.UsageText())
	case *int:
		flagset.Int(co.Name, co.DefaultValue.(int), co.UsageText())
	case *int8:
		flagset.Int8(co.Name, co.DefaultValue.(int8), co.UsageText())
	case *int16:
		flagset.Int16(co.Name, co.DefaultValue.(int16), co.UsageText())
	case *int32:
		flagset.Int32(co.Name, co.DefaultValue.(int32), co.UsageText())
	case *int64:
		flagset.Int64(co.Name, co.DefaultValue.(int64), co.UsageText())
	case *[]int:
		flagset.IntSlice(co.Name, co.DefaultValue.([]int), co.UsageText())
	case *[]int32:
		flagset.Int32Slice(co.Name, co.DefaultValue.([]int32), co.UsageText())
	case *[]int64:
		flagset.Int64Slice(co.Name, co.DefaultValue.([]int64), co.UsageText())
	case *string:
		// Set an empty string if no default was provided, since some value is always required for pflags
		if co.DefaultValue == nil {
			co.DefaultValue = ""
		}
		flagset.String(co.Name, co.DefaultValue.(string), co.UsageText())
	case *[]string:
		// Set an empty string if no default was provided, since some value is always required for pflags
		if co.DefaultValue == nil {
			co.DefaultValue = []string{}
		}
		flagset.StringSlice(co.Name, co.DefaultValue.([]string), co.UsageText())
	case *uint:
		flagset.Uint(co.Name, co.DefaultValue.(uint), co.UsageText())
	case *uint8:
		flagset.Uint8(co.Name, co.DefaultValue.(uint8), co.UsageText())
	case *uint16:
		flagset.Uint16(co.Name, co.DefaultValue.(uint16), co.UsageText())
	case *uint32:
		flagset.Uint32(co.Name, co.DefaultValue.(uint32), co.UsageText())
	case *uint64:
		flagset.Uint64(co.Name, co.DefaultValue.(uint64), co.UsageText())
	case *[]uint:
		flagset.UintSlice(co.Name, co.DefaultValue.([]uint), co.UsageText())
	default:
		return fmt.Errorf("unexpected option type: %T", co.ConfigKey)
	}

	co.flag = flagset.Lookup(co.Name)
	return nil
}

func (co *ConfigOption) GetFlag(flagset *pflag.FlagSet) (interface{}, error) {
	// Treat any option with a custom parser as a string option.
	if co.CustomSetValue != nil {
		return flagset.GetString(co.Name)
	}

	// Infer the type of the flag based on the type of the ConfigKey. This list
	// of options is based on the available flag types from pflags, and must
	// match the above in `AddFlag`.
	switch co.ConfigKey.(type) {
	case *bool:
		return flagset.GetBool(co.Name)
	case *time.Duration:
		return flagset.GetDuration(co.Name)
	case *float32:
		return flagset.GetFloat32(co.Name)
	case *float64:
		return flagset.GetFloat64(co.Name)
	case *net.IP:
		return flagset.GetIP(co.Name)
	case *net.IPNet:
		return flagset.GetIPNet(co.Name)
	case *int:
		return flagset.GetInt(co.Name)
	case *int8:
		return flagset.GetInt8(co.Name)
	case *int16:
		return flagset.GetInt16(co.Name)
	case *int32:
		return flagset.GetInt32(co.Name)
	case *int64:
		return flagset.GetInt64(co.Name)
	case *[]int:
		return flagset.GetIntSlice(co.Name)
	case *[]int32:
		return flagset.GetInt32Slice(co.Name)
	case *[]int64:
		return flagset.GetInt64Slice(co.Name)
	case *string:
		return flagset.GetString(co.Name)
	case *[]string:
		return flagset.GetStringSlice(co.Name)
	case *uint:
		return flagset.GetUint(co.Name)
	case *uint8:
		return flagset.GetUint8(co.Name)
	case *uint16:
		return flagset.GetUint16(co.Name)
	case *uint32:
		return flagset.GetUint32(co.Name)
	case *uint64:
		return flagset.GetUint64(co.Name)
	case *[]uint:
		return flagset.GetUintSlice(co.Name)
	default:
		return nil, fmt.Errorf("unexpected option type: %T", co.ConfigKey)
	}
}

// UsageText returns the string to use for the usage text of the option. The
// string returned will be the Usage defined on the ConfigOption, along with
// the environment variable.
func (co *ConfigOption) UsageText() string {
	envVar, hasEnvVar := co.getEnvKey()
	if hasEnvVar {
		return fmt.Sprintf("%s (%s)", co.Usage, envVar)
	} else {
		return co.Usage
	}
}
