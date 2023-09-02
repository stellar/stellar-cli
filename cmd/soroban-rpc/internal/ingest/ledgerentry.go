package ingest

import (
	"context"
	"io"
	"strings"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/ingest"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

func (s *Service) ingestLedgerEntryChanges(ctx context.Context, reader ingest.ChangeReader, tx db.WriteTx, progressLogPeriod int) error {
	entryCount := 0
	startTime := time.Now()
	writer := tx.LedgerEntryWriter()

	changeStatsProcessor := ingest.StatsChangeProcessor{}
	for ctx.Err() == nil {
		if change, err := reader.Read(); err == io.EOF {
			return nil
		} else if err != nil {
			return err
		} else if err = ingestLedgerEntryChange(writer, change); err != nil {
			return err
		} else if err = changeStatsProcessor.ProcessChange(ctx, change); err != nil {
			return err
		}
		entryCount++
		if progressLogPeriod > 0 && entryCount%progressLogPeriod == 0 {
			s.logger.Infof("processed %d ledger entry changes", entryCount)
		}
	}

	results := changeStatsProcessor.GetResults()
	for stat, value := range results.Map() {
		stat = strings.Replace(stat, "stats_", "change_", 1)
		s.ledgerStatsMetric.
			With(prometheus.Labels{"type": stat}).Add(float64(value.(int64)))
	}
	s.ingestionDurationMetric.
		With(prometheus.Labels{"type": "ledger_entries"}).Observe(time.Since(startTime).Seconds())
	return ctx.Err()
}

func ingestLedgerEntryChange(writer db.LedgerEntryWriter, change ingest.Change) error {
	if change.Post == nil {
		ledgerKey, err := xdr.GetLedgerKeyFromData(change.Pre.Data)
		if err != nil {
			return err
		}
		return writer.DeleteLedgerEntry(ledgerKey)
	} else {
		return writer.UpsertLedgerEntry(*change.Post)
	}
}
