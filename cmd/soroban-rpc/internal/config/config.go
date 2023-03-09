package config

import (
	"time"

	"github.com/sirupsen/logrus"
)

type LocalConfig struct {
	StellarCoreURL         string
	CoreRequestTimeout     time.Duration
	StellarCoreBinaryPath  string
	CaptiveCoreConfigPath  string
	CaptiveCoreStoragePath string
	CaptiveCoreHTTPPort    uint16
	CaptiveCoreUseDB       bool
	FriendbotURL           string
	NetworkPassphrase      string
	HistoryArchiveURLs     []string
	LogLevel               logrus.Level
	SQLiteDBPath           string
	// TODO: this unused but we should timeout the DB fillup
	LedgerEntryStorageTimeout        time.Duration
	EventLedgerRetentionWindow       uint32
	TransactionLedgerRetentionWindow uint32
	CheckpointFrequency              uint32
	MaxEventsLimit                   uint
	DefaultEventsLimit               uint
	MaxHealthyLedgerLatency          time.Duration
	PreflightWorkerCount             uint
}
