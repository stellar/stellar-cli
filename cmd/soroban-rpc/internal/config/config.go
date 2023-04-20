package config

import (
	"bytes"
	"fmt"
	"os"
	"runtime"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/sirupsen/logrus"

	"github.com/stellar/go/network"
	support "github.com/stellar/go/support/config"
	"github.com/stellar/go/support/errors"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
)

type LogFormat int

const (
	LogFormatText = iota
	LogFormatJSON
)

func (f LogFormat) String() string {
	switch f {
	case LogFormatText:
		return "text"
	case LogFormatJSON:
		return "json"
	default:
		panic(fmt.Sprintf("unknown log format: %d", f))
	}
}

type CaptiveCoreConfig struct {
	CaptiveCoreConfigPath  string `toml:"config-path" valid:"optional"`
	CaptiveCoreHTTPPort    uint   `toml:"http-port" valid:"optional"`
	CaptiveCoreStoragePath string `toml:"storage-path" valid:"optional"`
	CaptiveCoreUseDB       bool   `toml:"use-db" valid:"optional"`
	StellarCoreBinaryPath  string `toml:"binary-path" valid:"required"`
	StellarCoreURL         string `toml:"url" valid:"optional"`
}

// Config represents the configuration of a friendbot server
type Config struct {
	// Optional: The path to the config file. Not in the toml, as wouldn't make sense.
	ConfigPath string `toml:"-" valid:"-"`

	CaptiveCoreConfig `toml:"stellar-core" valid:"required"`

	Endpoint                         string        `toml:"endpoint" valid:"optional"`
	AdminEndpoint                    string        `toml:"admin-endpoint" valid:"optional"`
	CheckpointFrequency              uint32        `toml:"checkpoint-frequency" valid:"optional"`
	CoreRequestTimeout               time.Duration `toml:"core-request-timeout" valid:"optional"`
	DefaultEventsLimit               uint          `toml:"default-events-limit" valid:"optional"`
	EventLedgerRetentionWindow       uint32        `toml:"event-ledger-retention-window" valid:"optional"`
	FriendbotURL                     string        `toml:"friendbot-url" valid:"optional"`
	HistoryArchiveURLs               []string      `toml:"history-archive-urls" valid:"required"`
	IngestionTimeout                 time.Duration `toml:"ingestion-timeout" valid:"optional"`
	LogFormat                        LogFormat     `toml:"log-format" valid:"optional"`
	LogLevel                         logrus.Level  `toml:"log-level" valid:"optional"`
	MaxEventsLimit                   uint          `toml:"max-events-limit" valid:"optional"`
	MaxHealthyLedgerLatency          time.Duration `toml:"max-healthy-ledger-latency" valid:"optional"`
	NetworkPassphrase                string        `toml:"network-passphrase" valid:"required"`
	PreflightWorkerCount             uint          `toml:"preflight-worker-count" valid:"optional"`
	PreflightWorkerQueueSize         uint          `toml:"preflight-worker-queue-size" valid:"optional"`
	SQLiteDBPath                     string        `toml:"sqlite-db-path" valid:"optional"`
	TransactionLedgerRetentionWindow uint32        `toml:"transaction-ledger-retention-window" valid:"optional"`
}

func (cfg *Config) SetDefaults() {
	cfg.CaptiveCoreHTTPPort = 11626
	cfg.CheckpointFrequency = 64
	cfg.CoreRequestTimeout = 2 * time.Second
	cfg.DefaultEventsLimit = 100
	cfg.Endpoint = "localhost:8000"
	cfg.EventLedgerRetentionWindow = uint32(ledgerbucketwindow.DefaultEventLedgerRetentionWindow)
	cfg.IngestionTimeout = 30 * time.Minute
	cfg.LogFormat = LogFormatText
	cfg.LogLevel = logrus.InfoLevel
	cfg.MaxEventsLimit = 10000
	cfg.MaxHealthyLedgerLatency = 30 * time.Second
	cfg.NetworkPassphrase = network.FutureNetworkPassphrase
	cfg.PreflightWorkerCount = uint(runtime.NumCPU())
	cfg.PreflightWorkerQueueSize = uint(runtime.NumCPU())
	cfg.SQLiteDBPath = "soroban_rpc.sqlite"
	cfg.StellarCoreURL = fmt.Sprintf("http://localhost:%d", cfg.CaptiveCoreHTTPPort)
	cfg.TransactionLedgerRetentionWindow = 1440

	cwd, err := os.Getwd()
	if err != nil {
		panic(fmt.Errorf("unable to determine the current directory: %s", err))
	}
	cfg.CaptiveCoreStoragePath = cwd
}

func Read(path string) (*Config, error) {
	cfg := &Config{}
	err := support.Read(path, cfg)
	if err != nil {
		switch cause := errors.Cause(err).(type) {
		case *support.InvalidConfigError:
			return nil, errors.Wrap(cause, "config file")
		default:
			return nil, err
		}
	}
	return cfg, nil
}

func (cfg *Config) Validate() error {
	if cfg.DefaultEventsLimit > cfg.MaxEventsLimit {
		return fmt.Errorf(
			"default-events-limit (%v) cannot exceed max-events-limit (%v)\n",
			cfg.DefaultEventsLimit,
			cfg.MaxEventsLimit,
		)
	}

	if len(cfg.HistoryArchiveURLs) == 0 {
		return fmt.Errorf("history-archive-urls is required")
	}

	if cfg.NetworkPassphrase == "" {
		return fmt.Errorf("network-passphrase is required")
	}

	// if cfg.CaptiveCoreConfigPath == "" {
	// 	return fmt.Errorf("captive-core-config-path is required")
	// }

	if cfg.StellarCoreBinaryPath == "" {
		return fmt.Errorf("stellar-core-binary-path is required")
	}

	return nil
}

// Merge a and b, preferring values from b. Neither config is modified, instead
// a new config is returned.
// TODO: Unit-test this
// TODO: Find a less hacky and horrible way to do this.
func Merge(a, b *Config) (Config, error) {
	fmt.Printf("Merging configs: %+v, %+v", a, b)
	var buf bytes.Buffer
	aMap := map[string]interface{}{}
	err := toml.NewEncoder(&buf).Encode(a)
	if err != nil {
		return Config{}, errors.Wrap(err, "encoding config")
	}
	_, err = toml.Decode(buf.String(), &aMap)
	if err != nil {
		return Config{}, errors.Wrap(err, "encoding config")
	}
	buf.Reset()
	bMap := map[string]interface{}{}
	err = toml.NewEncoder(&buf).Encode(b)
	if err != nil {
		return Config{}, errors.Wrap(err, "encoding config")
	}
	_, err = toml.Decode(buf.String(), &bMap)
	if err != nil {
		return Config{}, errors.Wrap(err, "encoding config")
	}
	buf.Reset()

	for k, v := range bMap {
		aMap[k] = v
	}

	err = toml.NewEncoder(&buf).Encode(aMap)
	if err != nil {
		return Config{}, errors.Wrap(err, "encoding config")
	}
	var merged Config
	_, err = toml.Decode(buf.String(), &merged)
	if err != nil {
		return Config{}, errors.Wrap(err, "decoding config")
	}

	merged.ConfigPath = a.ConfigPath
	if b.ConfigPath != "" {
		merged.ConfigPath = b.ConfigPath
	}

	fmt.Printf("Merged: %+v", merged)
	return merged, nil
}
