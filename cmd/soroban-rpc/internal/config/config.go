package config

import (
	"time"

	"github.com/sirupsen/logrus"
)

type LocalConfig struct {
	EndPoint                  string
	HorizonURL                string
	StellarCoreBinaryPath     string
	CaptiveCoreConfigPath     string
	CaptiveCoreHTTPPort       uint16
	NetworkPassphrase         string
	HistoryArchiveURLs        []string
	LogLevel                  logrus.Level
	TxConcurrency             int
	TxQueueSize               int
	SQLiteDBPath              string
	LedgerEntryStorageTimeout time.Duration
}
