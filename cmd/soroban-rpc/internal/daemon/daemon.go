package daemon

import (
	"context"
	"errors"
	"net/http"
	"net/http/pprof" //nolint:gosec
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"github.com/stellar/go/clients/stellarcore"
	"github.com/stellar/go/historyarchive"
	"github.com/stellar/go/ingest/ledgerbackend"
	dbsession "github.com/stellar/go/support/db"
	supporthttp "github.com/stellar/go/support/http"
	supportlog "github.com/stellar/go/support/log"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/events"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ingest"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/preflight"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

const (
	prometheusNamespace          = "soroban_rpc"
	maxLedgerEntryWriteBatchSize = 150
	defaultReadTimeout           = 5 * time.Second
	defaultShutdownGracePeriod   = 10 * time.Second
)

type Daemon struct {
	core                *ledgerbackend.CaptiveStellarCore
	coreClient          *CoreClientWithMetrics
	ingestService       *ingest.Service
	db                  dbsession.SessionInterface
	jsonRPCHandler      *internal.Handler
	logger              *supportlog.Entry
	preflightWorkerPool *preflight.PreflightWorkerPool
	server              *http.Server
	adminServer         *http.Server
	closeOnce           sync.Once
	closeError          error
	done                chan struct{}
	metricsRegistry     *prometheus.Registry
}

func (d *Daemon) GetDB() dbsession.SessionInterface {
	return d.db
}

func (d *Daemon) close() {
	shutdownCtx, shutdownRelease := context.WithTimeout(context.Background(), defaultShutdownGracePeriod)
	defer shutdownRelease()
	var closeErrors []error

	if err := d.server.Shutdown(shutdownCtx); err != nil {
		d.logger.WithError(err).Error("error during Soroban JSON RPC server Shutdown")
		closeErrors = append(closeErrors, err)
	}
	if d.adminServer != nil {
		if err := d.adminServer.Shutdown(shutdownCtx); err != nil {
			d.logger.WithError(err).Error("error during Soroban JSON admin server Shutdown")
			closeErrors = append(closeErrors, err)
		}
	}

	if err := d.ingestService.Close(); err != nil {
		d.logger.WithError(err).Error("error closing ingestion service")
		closeErrors = append(closeErrors, err)
	}
	if err := d.core.Close(); err != nil {
		d.logger.WithError(err).Error("error closing captive core")
		closeErrors = append(closeErrors, err)
	}
	d.jsonRPCHandler.Close()
	if err := d.db.Close(); err != nil {
		d.logger.WithError(err).Error("Error closing db")
		closeErrors = append(closeErrors, err)
	}
	d.preflightWorkerPool.Close()
	d.closeError = errors.Join(closeErrors...)
	close(d.done)
}

func (d *Daemon) Close() error {
	d.closeOnce.Do(d.close)
	return d.closeError
}

// newCaptiveCore creates a new captive core backend instance and returns it.
func newCaptiveCore(cfg *config.Config, logger *supportlog.Entry) (*ledgerbackend.CaptiveStellarCore, error) {
	httpPortUint := uint(cfg.CaptiveCoreHTTPPort)
	var captiveCoreTomlParams ledgerbackend.CaptiveCoreTomlParams
	captiveCoreTomlParams.HTTPPort = &httpPortUint
	captiveCoreTomlParams.HistoryArchiveURLs = cfg.HistoryArchiveURLs
	captiveCoreTomlParams.NetworkPassphrase = cfg.NetworkPassphrase
	captiveCoreTomlParams.Strict = true
	captiveCoreTomlParams.UseDB = cfg.CaptiveCoreUseDB
	captiveCoreToml, err := ledgerbackend.NewCaptiveCoreTomlFromFile(cfg.CaptiveCoreConfigPath, captiveCoreTomlParams)
	if err != nil {
		logger.WithError(err).Fatal("Invalid captive core toml")
	}

	captiveConfig := ledgerbackend.CaptiveCoreConfig{
		BinaryPath:          cfg.StellarCoreBinaryPath,
		StoragePath:         cfg.CaptiveCoreStoragePath,
		NetworkPassphrase:   cfg.NetworkPassphrase,
		HistoryArchiveURLs:  cfg.HistoryArchiveURLs,
		CheckpointFrequency: cfg.CheckpointFrequency,
		Log:                 logger.WithField("subservice", "stellar-core"),
		Toml:                captiveCoreToml,
		UserAgent:           "captivecore",
		UseDB:               cfg.CaptiveCoreUseDB,
	}
	return ledgerbackend.NewCaptive(captiveConfig)

}

func MustNew(cfg *config.Config) *Daemon {
	logger := supportlog.New()
	logger.SetLevel(cfg.LogLevel)

	if cfg.LogFormat == config.LogFormatJSON {
		logger.UseJSONFormatter()
	}

	core, err := newCaptiveCore(cfg, logger)
	if err != nil {
		logger.WithError(err).Fatal("could not create captive core")
	}

	if len(cfg.HistoryArchiveURLs) == 0 {
		logger.Fatal("no history archives url were provided")
	}
	historyArchive, err := historyarchive.Connect(
		cfg.HistoryArchiveURLs[0],
		historyarchive.ConnectOptions{
			CheckpointFrequency: cfg.CheckpointFrequency,
		},
	)
	if err != nil {
		logger.WithError(err).Fatal("could not connect to history archive")
	}

	session, err := db.OpenSQLiteDB(cfg.SQLiteDBPath)
	if err != nil {
		logger.WithError(err).Fatal("could not open database")
	}
	metricsRegistry := prometheus.NewRegistry()

	dbConn := dbsession.RegisterMetrics(session, prometheusNamespace, "db", metricsRegistry)

	daemon := &Daemon{
		logger:          logger,
		core:            core,
		db:              dbConn,
		done:            make(chan struct{}),
		metricsRegistry: metricsRegistry,
		coreClient: newCoreClientWithMetrics(stellarcore.Client{
			URL:  cfg.StellarCoreURL,
			HTTP: &http.Client{Timeout: cfg.CoreRequestTimeout},
		}, metricsRegistry),
	}

	eventStore := events.NewMemoryStore(
		daemon,
		cfg.NetworkPassphrase,
		cfg.EventLedgerRetentionWindow,
	)
	transactionStore := transactions.NewMemoryStore(
		daemon,
		cfg.NetworkPassphrase,
		cfg.TransactionLedgerRetentionWindow,
	)

	maxRetentionWindow := cfg.EventLedgerRetentionWindow
	if cfg.TransactionLedgerRetentionWindow > maxRetentionWindow {
		maxRetentionWindow = cfg.TransactionLedgerRetentionWindow
	} else if cfg.EventLedgerRetentionWindow == 0 && cfg.TransactionLedgerRetentionWindow > ledgerbucketwindow.DefaultEventLedgerRetentionWindow {
		maxRetentionWindow = ledgerbucketwindow.DefaultEventLedgerRetentionWindow
	}

	// initialize the stores using what was on the DB
	readTxMetaCtx, cancelReadTxMeta := context.WithTimeout(context.Background(), cfg.IngestionTimeout)
	defer cancelReadTxMeta()
	txmetas, err := db.NewLedgerReader(dbConn).GetAllLedgers(readTxMetaCtx)
	if err != nil {
		logger.WithError(err).Fatal("could obtain txmeta cache from the database")
	}
	for _, txmeta := range txmetas {
		// NOTE: We could optimize this to avoid unnecessary ingestion calls
		//       (len(txmetas) can be larger than the store retention windows)
		//       but it's probably not worth the pain.
		if err := eventStore.IngestEvents(txmeta); err != nil {
			logger.WithError(err).Fatal("could initialize event memory store")
		}
		if err := transactionStore.IngestTransactions(txmeta); err != nil {
			logger.WithError(err).Fatal("could initialize transaction memory store")
		}
	}

	onIngestionRetry := func(err error, dur time.Duration) {
		logger.WithError(err).Error("could not run ingestion. Retrying")
	}
	ingestService := ingest.NewService(ingest.Config{
		Logger:            logger,
		DB:                db.NewReadWriter(dbConn, maxLedgerEntryWriteBatchSize, maxRetentionWindow),
		EventStore:        eventStore,
		TransactionStore:  transactionStore,
		NetworkPassPhrase: cfg.NetworkPassphrase,
		Archive:           historyArchive,
		LedgerBackend:     core,
		Timeout:           cfg.IngestionTimeout,
		OnIngestionRetry:  onIngestionRetry,
		Daemon:            daemon,
	})

	ledgerEntryReader := db.NewLedgerEntryReader(dbConn)
	preflightWorkerPool := preflight.NewPreflightWorkerPool(
		daemon,
		cfg.PreflightWorkerCount,
		cfg.PreflightWorkerQueueSize,
		ledgerEntryReader,
		cfg.NetworkPassphrase,
		logger,
	)

	jsonRPCHandler := internal.NewJSONRPCHandler(cfg, internal.HandlerParams{
		Daemon:            daemon,
		EventStore:        eventStore,
		TransactionStore:  transactionStore,
		Logger:            logger,
		LedgerReader:      db.NewLedgerReader(dbConn),
		LedgerEntryReader: db.NewLedgerEntryReader(dbConn),
		PreflightGetter:   preflightWorkerPool,
	})

	httpHandler := supporthttp.NewAPIMux(logger)
	httpHandler.Handle("/", jsonRPCHandler)

	daemon.preflightWorkerPool = preflightWorkerPool
	daemon.ingestService = ingestService
	daemon.jsonRPCHandler = &jsonRPCHandler

	daemon.server = &http.Server{
		Addr:        cfg.Endpoint,
		Handler:     httpHandler,
		ReadTimeout: defaultReadTimeout,
	}
	if cfg.AdminEndpoint != "" {
		adminMux := supporthttp.NewMux(logger)
		adminMux.HandleFunc("/debug/pprof/", pprof.Index)
		adminMux.HandleFunc("/debug/pprof/cmdline", pprof.Cmdline)
		adminMux.HandleFunc("/debug/pprof/profile", pprof.Profile)
		adminMux.HandleFunc("/debug/pprof/symbol", pprof.Symbol)
		adminMux.HandleFunc("/debug/pprof/trace", pprof.Trace)
		adminMux.Handle("/metrics", promhttp.HandlerFor(metricsRegistry, promhttp.HandlerOpts{}))
		daemon.adminServer = &http.Server{Addr: cfg.AdminEndpoint, Handler: adminMux}
	}
	daemon.registerMetrics()
	return daemon
}

func (d *Daemon) Run() {
	d.logger.WithFields(supportlog.F{
		"version": config.Version,
		"commit":  config.CommitHash,
		"addr":    d.server.Addr,
	}).Info("starting Soroban JSON RPC server")

	go func() {
		if err := d.server.ListenAndServe(); !errors.Is(err, http.ErrServerClosed) {
			// Error starting or closing listener:
			d.logger.WithError(err).Fatal("soroban JSON RPC server encountered fatal error")
		}
	}()

	if d.adminServer != nil {
		go func() {
			if err := d.adminServer.ListenAndServe(); !errors.Is(err, http.ErrServerClosed) {
				d.logger.WithError(err).Error("soroban admin server encountered fatal error")
			}
		}()
	}

	// Shutdown gracefully when we receive an interrupt signal.
	// First server.Shutdown closes all open listeners, then closes all idle connections.
	// Finally, it waits a grace period (10s here) for connections to return to idle and then shut down.
	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGINT, syscall.SIGTERM)

	select {
	case <-signals:
		d.Close()
	case <-d.done:
		return
	}
}
