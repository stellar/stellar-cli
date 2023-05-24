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
	flagset := cmd.PersistentFlags()
	for _, option := range cfg.options() {
		if err := option.AddFlag(flagset); err != nil {
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

	// Infer the type of the flag based on the type of the ConfigKey
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
	case *net.IPMask:
		flagset.IPMask(co.Name, co.DefaultValue.(net.IPMask), co.UsageText())
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

// UsageText returns the string to use for the usage text of the option. The
// string returned will be the Usage defined on the ConfigOption, along with
// the environment variable.
func (co *ConfigOption) UsageText() string {
	return fmt.Sprintf("%s (%s)", co.Usage, co.EnvVar)
}
