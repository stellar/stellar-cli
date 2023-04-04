package test

import (
	"context"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stretchr/testify/assert"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

func TestGetLatestLedgerSucceeds(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	coreInfo, err := test.coreClient.Info(context.Background())
	assert.NoError(t, err)

	actualLatestSequence := uint32(coreInfo.Info.Ledger.Num)
	actualProtocolVersion := coreInfo.Info.ProtocolVersion

	ledgerReader := db.NewLedgerReader(test.daemon.GetDB())
	actualLatestLedger, found, err := ledgerReader.GetLedger(context.Background(), actualLatestSequence)
	actualLatestLedgerHash := actualLatestLedger.LedgerHash().HexString()
	assert.NoError(t, err)
	assert.True(t, found)

	request := methods.GetLatestLedgerRequest{}
	var result methods.GetLatestLedgerResponse
	err = client.CallResult(context.Background(), "getLatestLedger", request, &result)
	assert.NoError(t, err)
	assert.Equal(t, result.Hash, actualLatestLedgerHash)
	assert.Equal(t, result.Sequence, actualLatestSequence)
	assert.Equal(t, result.ProtocolVersion, actualProtocolVersion)
}
