package config

import "github.com/sirupsen/logrus"

type LocalConfig struct {
	EndPoint          string
	HorizonURL        string
	StellarCoreURL    string
	NetworkPassphrase string
	LogLevel          logrus.Level
	TxConcurrency     int
	TxQueueSize       int
}
