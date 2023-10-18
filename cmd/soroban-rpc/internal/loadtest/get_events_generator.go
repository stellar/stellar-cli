package loadtest

import (
	"github.com/creachadair/jrpc2"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

// Generates specs for getting all events after a target start ledger.
type GetEventsGenerator struct {
	startLedger int32
}

func (generator *GetEventsGenerator) GenerateSpec() (jrpc2.Spec, error) {
	getEventsRequest := methods.GetEventsRequest{
		StartLedger: generator.startLedger,
	}
	return jrpc2.Spec{
		Method: "getEvents",
		Params: getEventsRequest,
	}, nil
}
