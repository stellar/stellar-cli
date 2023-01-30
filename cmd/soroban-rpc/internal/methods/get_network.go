package methods

import (
	"context"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
)

type GetNetworkResponse struct {
	FriendbotURL    string `json:"friendbotUrl,omitempty"`
	Passphrase      string `json:"passphrase"`
	ProtocolVersion string `json:"protocolVersion"`
}

// NewGetNetworkHandler returns a json rpc handler to for the getNetwork method
func NewGetNetworkHandler(networkPassphrase, friendbotURL string) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request SimulateTransactionRequest) GetNetworkResponse {
		return GetNetworkResponse{
			FriendbotURL:    friendbotURL,
			Passphrase:      networkPassphrase,
			ProtocolVersion: "0",
		}
	})
}
