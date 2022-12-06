package daemon

import (
	"net/http"
	"time"

	"github.com/stellar/go/clients/horizonclient"
	"github.com/stellar/go/clients/stellarcore"
	supporthttp "github.com/stellar/go/support/http"
	supportlog "github.com/stellar/go/support/log"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

func Start(cfg config.LocalConfig) (exitCode int) {
	logger := supportlog.New()
	logger.SetLevel(cfg.LogLevel)

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
		5*time.Minute,
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
