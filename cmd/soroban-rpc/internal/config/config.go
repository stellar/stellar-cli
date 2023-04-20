package config

import (
	"time"

	"github.com/sirupsen/logrus"
)

type LogFormat int

const (
	LogFormatText = iota
	LogFormatJSON
)

type LocalConfig struct {
	Endpoint                         string        `toml:"endpoint" valid:"optional"`
	AdminEndpoint                    string        `toml:"admin_endpoint" valid:"optional"`
	IngestionTimeoutMinutes          uint          `toml:"ingestion_timeout_minutes" valid:"optional"`
	CoreTimeoutSeconds               uint          `toml:"core_timeout_seconds" valid:"optional"`
	MaxHealthyLedgerLatencySeconds   uint          `toml:"max_healthy_ledger_latency_seconds" valid:"optional"`
	CaptiveCoreConfigPath            string        `toml:"captive_core_config_path" valid:"required"`
	CaptiveCoreHTTPPort              uint16        `toml:"captive_core_http_port" valid:"optional"`
	CaptiveCoreStoragePath           string        `toml:"captive_core_storage_path" valid:"optional"`
	CaptiveCoreUseDB                 bool          `toml:"captive_core_use_db" valid:"optional"`
	CheckpointFrequency              uint32        `toml:"checkpoint_frequency" valid:"optional"`
	CoreRequestTimeout               time.Duration `toml:"core_request_timeout" valid:"optional"`
	DefaultEventsLimit               uint          `toml:"default_events_limit" valid:"optional"`
	EventLedgerRetentionWindow       uint32        `toml:"event_ledger_retention_window" valid:"optional"`
	FriendbotURL                     string        `toml:"friendbot_url" valid:"optional"`
	HistoryArchiveURLs               []string      `toml:"history_archive_urls" valid:"required"`
	IngestionTimeout                 time.Duration `toml:"ingestion_timeout" valid:"optional"`
	LogFormat                        LogFormat     `toml:"log_format" valid:"optional"`
	LogLevel                         logrus.Level  `toml:"log_level" valid:"optional"`
	MaxEventsLimit                   uint          `toml:"max_events_limit" valid:"optional"`
	MaxHealthyLedgerLatency          time.Duration `toml:"max_healthy_ledger_latency" valid:"optional"`
	NetworkPassphrase                string        `toml:"network_passphrase" valid:"required"`
	PreflightWorkerCount             uint          `toml:"preflight_worker_count" valid:"optional"`
	PreflightWorkerQueueSize         uint          `toml:"preflight_worker_queue_size" valid:"optional"`
	SQLiteDBPath                     string        `toml:"sqlite_db_path" valid:"optional"`
	StellarCoreBinaryPath            string        `toml:"stellar_core_binary_path" valid:"required"`
	StellarCoreURL                   string        `toml:"stellar_core_url" valid:"optional"`
	TransactionLedgerRetentionWindow uint32        `toml:"transaction_ledger_retention_window" valid:"optional"`
}
