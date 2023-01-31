package ingest

import (
	"context"
	"io"

	"github.com/stellar/go/ingest"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

func (s *Service) ingestLedgerEntryChanges(ctx context.Context, reader ingest.ChangeReader, tx db.WriteTx) error {
	// Make sure we finish the updating transaction
	entryCount := 0
	writer := tx.LedgerEntryWriter()

	for ctx.Err() == nil {
		if change, err := reader.Read(); err == io.EOF {
			return nil
		} else if err != nil {
			return err
		} else if err = ingestLedgerEntryChange(writer, change); err != nil {
			return err
		}
		entryCount++
		if entryCount%changePrintOutFreq == 0 {
			s.logger.Infof("processed %d ledger entry changes", entryCount)
		}
	}
	return ctx.Err()
}

func ingestLedgerEntryChange(writer db.LedgerEntryWriter, change ingest.Change) error {
	if change.Post == nil {
		return writer.DeleteLedgerEntry(change.Pre.LedgerKey())
	} else {
		return writer.UpsertLedgerEntry(change.Post.LedgerKey(), *change.Post)
	}
}
