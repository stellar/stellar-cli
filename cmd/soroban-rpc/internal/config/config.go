package config

import (
	"time"

	"github.com/sirupsen/logrus"
)

type LocalConfig struct {
	HorizonURL                       string
	StellarCoreURL                   string
	StellarCoreBinaryPath            string
	CaptiveCoreConfigPath            string
	CaptiveCoreStoragePath           string
	CaptiveCoreHTTPPort              uint16
	CaptiveCoreUseDB                 bool
	FriendbotURL                     string
	NetworkPassphrase                string
	HistoryArchiveURLs               []string
	LogLevel                         logrus.Level
	TxConcurrency                    int
	TxQueueSize                      int
	SQLiteDBPath                     string
	LedgerEntryStorageTimeout        time.Duration
	EventLedgerRetentionWindow       int
	TransactionLedgerRetentionWindow int
	CheckpointFrequency              uint32
	MaxEventsLimit                   uint
	DefaultEventsLimit               uint
}
