package config

import (
	"go/types"
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
		OptType:   types.String,
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
		OptType:   types.Uint32,
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
		OptType:   types.Uint32,
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
func TestUnassignableFiels(t *testing.T) {
	var co ConfigOption
	var b bool
	co.Name = "mykey"
	co.ConfigKey = &b
	err := co.setValue("abc")
	require.Error(t, err)
	require.Contains(t, err.Error(), co.Name)
}
