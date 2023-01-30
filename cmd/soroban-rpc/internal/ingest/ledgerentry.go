package ingest

import (
	"context"
	"io"

	"github.com/stellar/go/ingest"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

func (r *Runner) ingestLedgerEntryChanges(ctx context.Context, reader ingest.ChangeReader, tx db.WriteTx) error {
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
			r.logger.Infof("processed %d ledger entry changes", entryCount)
		}
	}
	return ctx.Err()
}

func ingestLedgerEntryChange(writer db.LedgerEntryWriter, change ingest.Change) error {
	if change.Post == nil {
		ledgerKey, relevant, err := getRelevantLedgerKeyFromData(change.Post.Data)
		if err != nil {
			return err
		}
		if !relevant {
			return nil
		}

		return writer.UpsertLedgerEntry(ledgerKey, *change.Post)
	} else {
		ledgerKey, relevant, err := getRelevantLedgerKeyFromData(change.Pre.Data)
		if err != nil {
			return err
		}
		if !relevant {
			return nil
		}

		return writer.DeleteLedgerEntry(ledgerKey)
	}
}

func getRelevantLedgerKeyFromData(data xdr.LedgerEntryData) (xdr.LedgerKey, bool, error) {
	var key xdr.LedgerKey
	switch data.Type {
	case xdr.LedgerEntryTypeAccount:
		if err := key.SetAccount(data.Account.AccountId); err != nil {
			return key, false, err
		}
	case xdr.LedgerEntryTypeTrustline:
		if err := key.SetTrustline(data.TrustLine.AccountId, data.TrustLine.Asset); err != nil {
			return key, false, err
		}
	case xdr.LedgerEntryTypeContractData:
		if err := key.SetContractData(data.ContractData.ContractId, data.ContractData.Key); err != nil {
			return key, false, err
		}
	case xdr.LedgerEntryTypeContractCode:
		if err := key.SetContractCode(data.ContractCode.Hash); err != nil {
			return key, false, err
		}
	default:
		// we don't care about any other entry types for now
		return key, false, nil
	}
	return key, true, nil
}
