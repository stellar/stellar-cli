package methods

import (
	"context"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
)

type GetNetworkRequest struct{}

type GetNetworkResponse struct {
	FriendbotURL    string `json:"friendbotUrl,omitempty"`
	Passphrase      string `json:"passphrase"`
	ProtocolVersion int    `json:"protocolVersion,string"`
}

// NewGetNetworkHandler returns a json rpc handler to for the getNetwork method
func NewGetNetworkHandler(daemon interfaces.Daemon, networkPassphrase, friendbotURL string) jrpc2.Handler {
	coreClient := daemon.CoreClient()
	return handler.New(func(ctx context.Context, request GetNetworkRequest) (GetNetworkResponse, error) {
		info, err := coreClient.Info(ctx)
		if err != nil {
			return GetNetworkResponse{}, (&jrpc2.Error{
				Code:    code.InternalError,
				Message: err.Error(),
			})
		}
		return GetNetworkResponse{
			FriendbotURL:    friendbotURL,
			Passphrase:      networkPassphrase,
			ProtocolVersion: info.Info.ProtocolVersion,
		}, nil
	})
}
