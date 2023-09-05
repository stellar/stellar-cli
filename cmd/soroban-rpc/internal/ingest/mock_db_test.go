package ingest

import (
	"context"

	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/mock"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

var (
	_ db.ReadWriter        = (*MockDB)(nil)
	_ db.WriteTx           = (*MockTx)(nil)
	_ db.LedgerEntryWriter = (*MockLedgerEntryWriter)(nil)
	_ db.LedgerWriter      = (*MockLedgerWriter)(nil)
)

type MockDB struct {
	mock.Mock
}

func (m MockDB) NewTx(ctx context.Context) (db.WriteTx, error) {
	args := m.Called(ctx)
	return args.Get(0).(db.WriteTx), args.Error(1)
}

func (m MockDB) GetLatestLedgerSequence(ctx context.Context) (uint32, error) {
	args := m.Called(ctx)
	return args.Get(0).(uint32), args.Error(1)
}

type MockTx struct {
	mock.Mock
}

func (m MockTx) LedgerEntryWriter() db.LedgerEntryWriter {
	args := m.Called()
	return args.Get(0).(db.LedgerEntryWriter)
}

func (m MockTx) LedgerWriter() db.LedgerWriter {
	args := m.Called()
	return args.Get(0).(db.LedgerWriter)
}

func (m MockTx) Commit(ledgerSeq uint32) error {
	args := m.Called(ledgerSeq)
	return args.Error(0)
}

func (m MockTx) Rollback() error {
	args := m.Called()
	return args.Error(0)
}

type MockLedgerEntryWriter struct {
	mock.Mock
}

func (m MockLedgerEntryWriter) UpsertLedgerEntry(entry xdr.LedgerEntry) error {
	args := m.Called(entry)
	return args.Error(0)
}

func (m MockLedgerEntryWriter) DeleteLedgerEntry(key xdr.LedgerKey) error {
	args := m.Called(key)
	return args.Error(0)
}

type MockLedgerWriter struct {
	mock.Mock
}

func (m MockLedgerWriter) InsertLedger(ledger xdr.LedgerCloseMeta) error {
	args := m.Called(ledger)
	return args.Error(0)
}
