package config

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestConfigOptionGetTomlKey(t *testing.T) {
	// Explicitly set toml key
	assert.Equal(t, "TOML_KEY", ConfigOption{TomlKey: "TOML_KEY"}.getTomlKey())

	// Explicitly disabled toml key
	assert.Equal(t, "-", ConfigOption{TomlKey: "-"}.getTomlKey())

	// Fallback to env var
	assert.Equal(t, "ENV_VAR", ConfigOption{EnvVar: "ENV_VAR"}.getTomlKey())

	// Env-var disabled, autogenerate from name
	assert.Equal(t, "TEST_FLAG", ConfigOption{Name: "test-flag", EnvVar: "-"}.getTomlKey())

	// Env-var not set, autogenerate from name
	assert.Equal(t, "TEST_FLAG", ConfigOption{Name: "test-flag"}.getTomlKey())
}
