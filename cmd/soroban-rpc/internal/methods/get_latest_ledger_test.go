package methods

import (
	"context"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

const (
	expectedLatestLedgerSequence        uint32 = 960
	expectedLatestLedgerProtocolVersion uint32 = 20
	expectedLatestLedgerHashBytes       byte   = 42
)

type ConstantLedgerEntryReader struct {
}

type ConstantLedgerEntryReaderTx struct {
}

type ConstantLedgerReader struct {
}

func (entryReader *ConstantLedgerEntryReader) GetLatestLedgerSequence(ctx context.Context) (uint32, error) {
	return expectedLatestLedgerSequence, nil
}

func (entryReader *ConstantLedgerEntryReader) NewTx(ctx context.Context) (db.LedgerEntryReadTx, error) {
	return ConstantLedgerEntryReaderTx{}, nil
}

func (entryReaderTx ConstantLedgerEntryReaderTx) GetLatestLedgerSequence() (uint32, error) {
	return expectedLatestLedgerSequence, nil
}

func (entryReaderTx ConstantLedgerEntryReaderTx) GetLedgerEntry(key xdr.LedgerKey, includeExpired bool) (bool, xdr.LedgerEntry, error) {
	return false, xdr.LedgerEntry{}, nil
}

func (entryReaderTx ConstantLedgerEntryReaderTx) Done() error {
	return nil
}

func (ledgerReader *ConstantLedgerReader) GetLedger(ctx context.Context, sequence uint32) (xdr.LedgerCloseMeta, bool, error) {
	return createLedger(sequence, expectedLatestLedgerProtocolVersion, expectedLatestLedgerHashBytes), true, nil
}

func (ledgerReader *ConstantLedgerReader) GetAllLedgers(ctx context.Context) ([]xdr.LedgerCloseMeta, error) {
	return []xdr.LedgerCloseMeta{createLedger(expectedLatestLedgerSequence, expectedLatestLedgerProtocolVersion, expectedLatestLedgerHashBytes)}, nil
}

func createLedger(ledgerSequence uint32, protocolVersion uint32, hash byte) xdr.LedgerCloseMeta {
	return xdr.LedgerCloseMeta{
		V: 2,
		V2: &xdr.LedgerCloseMetaV2{
			LedgerHeader: xdr.LedgerHeaderHistoryEntry{
				Hash: xdr.Hash{hash},
				Header: xdr.LedgerHeader{
					LedgerSeq:     xdr.Uint32(ledgerSequence),
					LedgerVersion: xdr.Uint32(protocolVersion),
				},
			},
		},
	}
}

func TestGetLatestLedger(t *testing.T) {
	getLatestLedgerHandler := NewGetLatestLedgerHandler(&ConstantLedgerEntryReader{}, &ConstantLedgerReader{})
	latestLedgerRespI, err := getLatestLedgerHandler.Handle(context.Background(), &jrpc2.Request{})
	latestLedgerResp := latestLedgerRespI.(GetLatestLedgerResponse)
	assert.NoError(t, err)

	expectedLatestLedgerHashStr := xdr.Hash{expectedLatestLedgerHashBytes}.HexString()
	assert.Equal(t, expectedLatestLedgerHashStr, latestLedgerResp.Hash)

	assert.Equal(t, expectedLatestLedgerProtocolVersion, latestLedgerResp.ProtocolVersion)
	assert.Equal(t, expectedLatestLedgerSequence, latestLedgerResp.Sequence)
}
