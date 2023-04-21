package config

import (
	"fmt"
	"os"
	"runtime"
	"time"

	"github.com/sirupsen/logrus"

	"github.com/stellar/go/ingest/ledgerbackend"
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

type CaptiveCoreConfig = ledgerbackend.CaptiveCoreToml

// Config represents the configuration of a friendbot server
type Config struct {
	// Optional: The path to the config file. Not in the toml, as wouldn't make sense.
	ConfigPath string `toml:"-" valid:"-"`

	CaptiveCoreConfig `toml:"STELLAR_CORE" valid:"required"`

	CaptiveCoreStoragePath           string        `toml:"CAPTIVE_CORE_STORAGE_PATH" valid:"optional"`
	Endpoint                         string        `toml:"ENDPOINT" valid:"optional"`
	AdminEndpoint                    string        `toml:"ADMIN_ENDPOINT" valid:"optional"`
	CheckpointFrequency              uint32        `toml:"CHECKPOINT_FREQUENCY" valid:"optional"`
	CoreRequestTimeout               time.Duration `toml:"CORE_REQUEST_TIMEOUT" valid:"optional"`
	DefaultEventsLimit               uint          `toml:"DEFAULT_EVENTS_LIMIT" valid:"optional"`
	EventLedgerRetentionWindow       uint32        `toml:"EVENT_LEDGER_RETENTION_WINDOW" valid:"optional"`
	FriendbotURL                     string        `toml:"FRIENDBOT_URL" valid:"optional"`
	HistoryArchiveURLs               []string      `toml:"HISTORY_ARCHIVE_URLS" valid:"required"`
	IngestionTimeout                 time.Duration `toml:"INGESTION_TIMEOUT" valid:"optional"`
	LogFormat                        LogFormat     `toml:"LOG_FORMAT" valid:"optional"`
	LogLevel                         logrus.Level  `toml:"LOG_LEVEL" valid:"optional"`
	MaxEventsLimit                   uint          `toml:"MAX_EVENTS_LIMIT" valid:"optional"`
	MaxHealthyLedgerLatency          time.Duration `toml:"MAX_HEALTHY_LEDGER_LATENCY" valid:"optional"`
	NetworkPassphrase                string        `toml:"NETWORK_PASSPHRASE" valid:"required"`
	PreflightWorkerCount             uint          `toml:"PREFLIGHT_WORKER_COUNT" valid:"optional"`
	PreflightWorkerQueueSize         uint          `toml:"PREFLIGHT_WORKER_QUEUE_SIZE" valid:"optional"`
	SQLiteDBPath                     string        `toml:"SQLITE_DB_PATH" valid:"optional"`
	TransactionLedgerRetentionWindow uint32        `toml:"TRANSACTION_LEDGER_RETENTION_WINDOW" valid:"optional"`
}

func (cfg *Config) SetDefaults() {
	cfg.CaptiveCoreConfig.HTTPPort = 11626
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
		return cannotBeBlank(
			"history-archive-urls",
			"HISTORY_ARCHIVE_URLS",
		)
	}

	if cfg.NetworkPassphrase == "" {
		return cannotBeBlank(
			"network-passphrase",
			"NETWORK_PASSPHRASE",
		)
	}

	// if cfg.CaptiveCoreConfigPath == "" {
	// 	return cannotBeBlank(
	// 		"captive-core-config-path",
	// 		"CAPTIVE_CORE_CONFIG_PATH",
	// 	)
	// }

	if cfg.StellarCoreBinaryPath == "" {
		return cannotBeBlank(
			"stellar-core-binary-path",
			"STELLAR_CORE_BINARY_PATH",
		)
	}

	return nil
}

func cannotBeBlank(name, envVar string) error {
	return fmt.Errorf("Invalid config: %s is blank. Please specify --%s on the command line or set the %s environment variable.", name, name, envVar)
}

// Merge a and b, preferring values from b. Neither config is modified, instead
// a new config is returned.
// TODO: Unit-test this
// TODO: Find a less hacky and horrible way to do this.
func (a *Config) Merge(b *Config) Config {
	merged := Config{
		CaptiveCoreConfig:                a.CaptiveCoreConfig.Merge(&b.CaptiveCoreConfig),
		ConfigPath:                       b.ConfigPath,
		Endpoint:                         b.Endpoint,
		AdminEndpoint:                    b.AdminEndpoint,
		CheckpointFrequency:              b.CheckpointFrequency,
		CoreRequestTimeout:               b.CoreRequestTimeout,
		DefaultEventsLimit:               b.DefaultEventsLimit,
		EventLedgerRetentionWindow:       b.EventLedgerRetentionWindow,
		FriendbotURL:                     b.FriendbotURL,
		HistoryArchiveURLs:               b.HistoryArchiveURLs,
		IngestionTimeout:                 b.IngestionTimeout,
		LogFormat:                        b.LogFormat,
		LogLevel:                         b.LogLevel,
		MaxEventsLimit:                   b.MaxEventsLimit,
		MaxHealthyLedgerLatency:          b.MaxHealthyLedgerLatency,
		NetworkPassphrase:                b.NetworkPassphrase,
		PreflightWorkerCount:             b.PreflightWorkerCount,
		PreflightWorkerQueueSize:         b.PreflightWorkerQueueSize,
		SQLiteDBPath:                     b.SQLiteDBPath,
		TransactionLedgerRetentionWindow: b.TransactionLedgerRetentionWindow,
	}
	if merged.ConfigPath == "" {
		merged.ConfigPath = a.ConfigPath
	}
	if merged.Endpoint == "" {
		merged.Endpoint = a.Endpoint
	}
	if merged.AdminEndpoint == "" {
		merged.AdminEndpoint = a.AdminEndpoint
	}
	if merged.CheckpointFrequency == 0 {
		merged.CheckpointFrequency = a.CheckpointFrequency
	}
	if merged.CoreRequestTimeout == 0 {
		merged.CoreRequestTimeout = a.CoreRequestTimeout
	}
	if merged.DefaultEventsLimit == 0 {
		merged.DefaultEventsLimit = a.DefaultEventsLimit
	}
	if merged.EventLedgerRetentionWindow == 0 {
		merged.EventLedgerRetentionWindow = a.EventLedgerRetentionWindow
	}
	if merged.FriendbotURL == "" {
		merged.FriendbotURL = a.FriendbotURL
	}
	if len(merged.HistoryArchiveURLs) == 0 {
		merged.HistoryArchiveURLs = a.HistoryArchiveURLs
	}
	if merged.IngestionTimeout == 0 {
		merged.IngestionTimeout = a.IngestionTimeout
	}
	if merged.LogFormat == 0 {
		merged.LogFormat = a.LogFormat
	}
	if merged.MaxEventsLimit == 0 {
		merged.MaxEventsLimit = a.MaxEventsLimit
	}
	if merged.MaxHealthyLedgerLatency == 0 {
		merged.MaxHealthyLedgerLatency = a.MaxHealthyLedgerLatency
	}
	if merged.LogLevel == logrus.Level(0) {
		merged.LogLevel = b.LogLevel
	}
	if merged.PreflightWorkerCount == 0 {
		merged.PreflightWorkerCount = a.PreflightWorkerCount
	}
	if merged.PreflightWorkerQueueSize == 0 {
		merged.PreflightWorkerQueueSize = a.PreflightWorkerQueueSize
	}
	if merged.SQLiteDBPath == "" {
		merged.SQLiteDBPath = a.SQLiteDBPath
	}
	if merged.TransactionLedgerRetentionWindow == 0 {
		merged.TransactionLedgerRetentionWindow = a.TransactionLedgerRetentionWindow
	}
	if merged.NetworkPassphrase == "" {
		merged.NetworkPassphrase = a.NetworkPassphrase
	}
	return merged
}

func (a *CaptiveCoreConfig) Merge(b *CaptiveCoreConfig) CaptiveCoreConfig {
	merged := CaptiveCoreConfig{
		CaptiveCoreHTTPPort:    b.CaptiveCoreHTTPPort,
		CaptiveCorePeerPort:    b.CaptiveCorePeerPort,
		CaptiveCoreStoragePath: b.CaptiveCoreStoragePath,
		CaptiveCoreUseDB:       b.CaptiveCoreUseDB,
		StellarCoreBinaryPath:  b.StellarCoreBinaryPath,
		StellarCoreURL:         b.StellarCoreURL,
	}
	if merged.CaptiveCoreHTTPPort == 0 {
		merged.CaptiveCoreHTTPPort = a.CaptiveCoreHTTPPort
	}
	if merged.CaptiveCorePeerPort == 0 {
		merged.CaptiveCorePeerPort = a.CaptiveCorePeerPort
	}
	if merged.CaptiveCoreStoragePath == "" {
		merged.CaptiveCoreStoragePath = a.CaptiveCoreStoragePath
	}
	if !merged.CaptiveCoreUseDB {
		merged.CaptiveCoreUseDB = a.CaptiveCoreUseDB
	}
	if merged.StellarCoreBinaryPath == "" {
		merged.StellarCoreBinaryPath = a.StellarCoreBinaryPath
	}
	if merged.StellarCoreURL == "" {
		merged.StellarCoreURL = a.StellarCoreURL
	}
	return merged
}
