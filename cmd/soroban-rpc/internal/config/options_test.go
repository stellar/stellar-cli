package config

import (
	"reflect"
	"testing"
	"unsafe"
)

func TestAllConfigKeysMustBePointers(t *testing.T) {
	// This test is to ensure we've set up all the config keys correctly.
	cfg := Config{}
	for _, option := range cfg.options() {
		kind := reflect.ValueOf(option.ConfigKey).Type().Kind()
		if kind != reflect.Pointer {
			t.Errorf("ConfigOption.ConfigKey must be a pointer, got %s for %s", kind, option.Name)
		}
	}
}

func TestAllConfigFieldsMustHaveASingleOption(t *testing.T) {
	// This test ensures we've documented all the config options, and not missed
	// any when adding new flags (or accidentally added conflicting duplicates).

	// Allow us to explicitly exclude any fields on the Config struct, which are not going to have Options.
	// e.g. "ConfigPath"
	excluded := map[string]bool{}

	cfg := Config{}
	cfgValue := reflect.ValueOf(cfg)
	cfgType := cfgValue.Type()

	options := cfg.options()
	optionsByField := map[uintptr]*ConfigOption{}
	for _, option := range options {
		key := uintptr(reflect.ValueOf(option.ConfigKey).UnsafePointer())
		if existing, ok := optionsByField[key]; ok {
			t.Errorf("Conflicting ConfigOptions %s and %s, point to the same struct field", existing.Name, option.Name)
		}
		optionsByField[key] = option
	}

	// Get the base address of the struct
	cfgPtr := uintptr(unsafe.Pointer(&cfg))
	for _, structField := range reflect.VisibleFields(cfgType) {
		if excluded[structField.Name] {
			continue
		}
		if !structField.IsExported() {
			continue
		}

		// Each field has an offset within that struct
		fieldPointer := cfgPtr + structField.Offset

		// There should be an option which points to this field
		_, ok := optionsByField[fieldPointer]
		if !ok {
			t.Errorf("Missing ConfigOption for field Config.%s", structField.Name)
		}
	}
}

func TestAllOptionsMustHaveAUniqueValidTomlKey(t *testing.T) {
	// This test ensures we've set a toml key for all the config options, and the
	// keys are all unique & valid. Note, we don't need to check that all struct
	// fields on the config have an option, because the test above checks that.

	// Allow us to explicitly exclude any fields on the Config struct, which are
	// not going to be in the toml.
	// e.g. "ConfigPath"
	excluded := map[string]bool{}

	cfg := Config{}

	options := cfg.options()
	optionsByTomlKey := map[string]*ConfigOption{}
	for _, option := range options {
		key, ok := option.getTomlKey()
		if !ok && !excluded[option.Name] {
			t.Errorf("Missing toml key for ConfigOption %s", option.Name)
		}
		if existing, ok := optionsByTomlKey[key]; ok {
			t.Errorf("Conflicting ConfigOptions %s and %s, have the same toml key: %s", existing.Name, option.Name, key)
		}
		optionsByTomlKey[key] = option
	}
}
