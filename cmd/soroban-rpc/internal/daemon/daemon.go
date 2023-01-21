package daemon

import (
	"net/http"
	"time"

	"github.com/stellar/go/clients/horizonclient"
	"github.com/stellar/go/clients/stellarcore"
	"github.com/stellar/go/historyarchive"
	"github.com/stellar/go/ingest/ledgerbackend"
	supporthttp "github.com/stellar/go/support/http"
	supportlog "github.com/stellar/go/support/log"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerentry_storage"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

const transactionProxyTTL = 5 * time.Minute

func Start(cfg config.LocalConfig) (exitCode int) {
	logger := supportlog.New()
	logger.SetLevel(cfg.LogLevel)

	httpPortUint := uint(cfg.CaptiveCoreHTTPPort)
	var captiveCoreTomlParams ledgerbackend.CaptiveCoreTomlParams
	captiveCoreTomlParams.HTTPPort = &httpPortUint
	captiveCoreTomlParams.HistoryArchiveURLs = cfg.HistoryArchiveURLs
	captiveCoreTomlParams.NetworkPassphrase = cfg.NetworkPassphrase
	captiveCoreTomlParams.Strict = true
	captiveCoreToml, err := ledgerbackend.NewCaptiveCoreTomlFromFile(cfg.CaptiveCoreConfigPath, captiveCoreTomlParams)
	if err != nil {
		logger.WithError(err).Fatal("Invalid captive core toml")
	}

	captiveConfig := ledgerbackend.CaptiveCoreConfig{
		BinaryPath:         cfg.StellarCoreBinaryPath,
		NetworkPassphrase:  cfg.NetworkPassphrase,
		HistoryArchiveURLs: cfg.HistoryArchiveURLs,
		// TODO: set for testing
		// CheckpointFrequency: checkpointFrequency,
		Log:       logger.WithField("subservice", "stellar-core"),
		Toml:      captiveCoreToml,
		UserAgent: "captivecore",
	}
	core, err := ledgerbackend.NewCaptive(captiveConfig)
	if err != nil {
		logger.Fatalf("could not create captive core: %v", err)
	}

	defer core.Close()

	historyArchive, err := historyarchive.Connect(
		cfg.HistoryArchiveURLs[0],
		historyarchive.ConnectOptions{},
	)
	if err != nil {
		logger.Fatalf("could not connect to history archive: %v", err)
	}

	db, err := ledgerentry_storage.OpenSQLiteDB(cfg.SQLiteDBPath)
	if err != nil {
		logger.Fatalf("could not open database: %v", err)
	}

	storage, err := ledgerentry_storage.NewLedgerEntryStorage(ledgerentry_storage.LedgerEntryStorageCfg{
		Logger:            logger,
		DB:                db,
		NetworkPassPhrase: cfg.NetworkPassphrase,
		Archive:           historyArchive,
		LedgerBackend:     core,
		Timeout:           cfg.LedgerEntryStorageTimeout,
	})
	if err != nil {
		logger.Fatalf("could not initialize ledger entry storage: %v", err)
	}
	defer storage.Close()

	hc := &horizonclient.Client{
		HorizonURL: cfg.HorizonURL,
		HTTP: &http.Client{
			Timeout: horizonclient.HorizonTimeout,
		},
		AppName: "Soroban RPC",
	}
	hc.SetHorizonTimeout(horizonclient.HorizonTimeout)

	transactionProxy := methods.NewTransactionProxy(
		hc,
		cfg.TxConcurrency,
		cfg.TxQueueSize,
		cfg.NetworkPassphrase,
		transactionProxyTTL,
	)

	handler, err := internal.NewJSONRPCHandler(internal.HandlerParams{
		AccountStore:     methods.AccountStore{Client: hc},
		EventStore:       methods.EventStore{Client: hc},
		Logger:           logger,
		TransactionProxy: transactionProxy,
		CoreClient:       &stellarcore.Client{URL: cfg.StellarCoreURL},
	})
	if err != nil {
		logger.Fatalf("could not create handler: %v", err)
	}
	supporthttp.Run(supporthttp.Config{
		ListenAddr: cfg.EndPoint,
		Handler:    handler,
		OnStarting: func() {
			logger.Infof("Starting Soroban JSON RPC server on %v", cfg.EndPoint)
			handler.Start()
		},
		OnStopping: func() {
			handler.Close()
		},
	})
	return 0
}
