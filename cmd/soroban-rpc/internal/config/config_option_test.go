package config

import (
	"fmt"
	"math"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestConfigOptionGetTomlKey(t *testing.T) {
	// Explicitly set toml key
	key, ok := ConfigOption{TomlKey: "TOML_KEY"}.getTomlKey()
	assert.Equal(t, "TOML_KEY", key)
	assert.True(t, ok)

	// Explicitly disabled toml key via `-`
	key, ok = ConfigOption{TomlKey: "-"}.getTomlKey()
	assert.Equal(t, "", key)
	assert.False(t, ok)

	// Explicitly disabled toml key via `_`
	key, ok = ConfigOption{TomlKey: "_"}.getTomlKey()
	assert.Equal(t, "", key)
	assert.False(t, ok)

	// Fallback to env var
	key, ok = ConfigOption{EnvVar: "ENV_VAR"}.getTomlKey()
	assert.Equal(t, "ENV_VAR", key)
	assert.True(t, ok)

	// Env-var disabled, autogenerate from name
	key, ok = ConfigOption{Name: "test-flag", EnvVar: "-"}.getTomlKey()
	assert.Equal(t, "TEST_FLAG", key)
	assert.True(t, ok)

	// Env-var not set, autogenerate from name
	key, ok = ConfigOption{Name: "test-flag"}.getTomlKey()
	assert.Equal(t, "TEST_FLAG", key)
	assert.True(t, ok)
}

func TestValidateRequired(t *testing.T) {
	var strVal string
	o := &ConfigOption{
		Name:      "required-option",
		ConfigKey: &strVal,
		Validate:  required,
	}

	// unset
	assert.ErrorContains(t, o.Validate(o), "required-option is required")

	// set with blank value
	require.NoError(t, o.setValue(""))
	assert.ErrorContains(t, o.Validate(o), "required-option is required")

	// set with valid value
	require.NoError(t, o.setValue("not-blank"))
	assert.NoError(t, o.Validate(o))
}

func TestValidatePositiveUint32(t *testing.T) {
	var val uint32
	o := &ConfigOption{
		Name:      "positive-option",
		ConfigKey: &val,
		Validate:  positive,
	}

	// unset
	assert.ErrorContains(t, o.Validate(o), "positive-option must be positive")

	// set with 0 value
	require.NoError(t, o.setValue(uint32(0)))
	assert.ErrorContains(t, o.Validate(o), "positive-option must be positive")

	// set with valid value
	require.NoError(t, o.setValue(uint32(1)))
	assert.NoError(t, o.Validate(o))
}

func TestValidatePositiveInt(t *testing.T) {
	var val int
	o := &ConfigOption{
		Name:      "positive-option",
		ConfigKey: &val,
		Validate:  positive,
	}

	// unset
	assert.ErrorContains(t, o.Validate(o), "positive-option must be positive")

	// set with 0 value
	require.NoError(t, o.setValue(0))
	assert.ErrorContains(t, o.Validate(o), "positive-option must be positive")

	// set with negative value
	require.NoError(t, o.setValue(-1))
	assert.ErrorContains(t, o.Validate(o), "positive-option must be positive")

	// set with valid value
	require.NoError(t, o.setValue(1))
	assert.NoError(t, o.Validate(o))
}

func TestUnassignableField(t *testing.T) {
	var co ConfigOption
	var b bool
	co.Name = "mykey"
	co.ConfigKey = &b
	err := co.setValue("abc")
	require.Error(t, err)
	require.Contains(t, err.Error(), co.Name)
}

func TestSetValue(t *testing.T) {
	var b bool
	var i int
	var u32 uint32
	var u64 uint64
	var f64 float64
	var s string

	for _, scenario := range []struct {
		name  string
		key   interface{}
		value interface{}
		err   error
	}{
		{
			name:  "valid-bool",
			key:   &b,
			value: true,
			err:   nil,
		},
		{
			name:  "valid-bool-string",
			key:   &b,
			value: "true",
			err:   nil,
		},
		{
			name:  "valid-bool-string-false",
			key:   &b,
			value: "false",
			err:   nil,
		},
		{
			name:  "valid-bool-string-uppercase",
			key:   &b,
			value: "TRUE",
			err:   nil,
		},
		{
			name:  "invalid-bool-string",
			key:   &b,
			value: "foobar",
			err:   fmt.Errorf("invalid boolean value invalid-bool-string: foobar"),
		},
		{
			name:  "invalid-bool-string",
			key:   &b,
			value: "foobar",
			err:   fmt.Errorf("invalid boolean value invalid-bool-string: foobar"),
		},
		{
			name:  "valid-int",
			key:   &i,
			value: 1,
			err:   nil,
		},
		{
			name:  "valid-int-string",
			key:   &i,
			value: "1",
			err:   nil,
		},
		{
			name:  "invalid-int-string",
			key:   &i,
			value: "abcd",
			err:   fmt.Errorf("strconv.ParseInt: parsing \"abcd\": invalid syntax"),
		},
		{
			name:  "valid-uint32",
			key:   &u32,
			value: 1,
			err:   nil,
		},
		{
			name:  "overflow-uint32",
			key:   &u32,
			value: uint64(math.MaxUint32) + 1,
			err:   fmt.Errorf("overflow-uint32 overflows uint32"),
		},
		{
			name:  "negative-uint32",
			key:   &u32,
			value: -1,
			err:   fmt.Errorf("negative-uint32 cannot be negative"),
		},
		{
			name:  "valid-uint",
			key:   &u64,
			value: 1,
			err:   nil,
		},
		{
			name:  "negative-uint",
			key:   &u64,
			value: -1,
			err:   fmt.Errorf("negative-uint cannot be negative"),
		},
		{
			name:  "valid-float",
			key:   &f64,
			value: 1.05,
			err:   nil,
		},
		{
			name:  "valid-float-int",
			key:   &f64,
			value: int64(1234),
			err:   nil,
		},
		{
			name:  "valid-float-string",
			key:   &f64,
			value: "1.05",
			err:   nil,
		},
		{
			name:  "invalid-float-string",
			key:   &f64,
			value: "foobar",
			err:   fmt.Errorf("strconv.ParseFloat: parsing \"foobar\": invalid syntax"),
		},
		{
			name:  "valid-string",
			key:   &s,
			value: "foobar",
			err:   nil,
		},
	} {
		t.Run(scenario.name, func(t *testing.T) {
			co := ConfigOption{
				Name:      scenario.name,
				ConfigKey: scenario.key,
			}
			err := co.setValue(scenario.value)
			if scenario.err != nil {
				require.EqualError(t, err, scenario.err.Error())
			} else {
				require.NoError(t, err)
			}
		})
	}
}
