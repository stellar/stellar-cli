package methods

import (
	"context"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"
	"github.com/stellar/go/clients/horizonclient"
)

type AccountInfo struct {
	ID       string `json:"id"`
	Sequence int64  `json:"sequence,string"`
}

type AccountRequest struct {
	Address string ` json:"address"`
}

type AccountStore struct {
	Client *horizonclient.Client
}

func (a AccountStore) GetAccount(request AccountRequest) (AccountInfo, error) {
	details, err := a.Client.AccountDetail(horizonclient.AccountRequest{AccountID: request.Address})
	if err != nil {
		return AccountInfo{}, err
	}

	return AccountInfo{
		ID:       details.AccountID,
		Sequence: details.Sequence,
	}, nil
}

// NewAccountHandler returns a json rpc handler to fetch account info
func NewAccountHandler(store AccountStore) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request AccountRequest) (AccountInfo, error) {
		response, err := store.GetAccount(request)
		if err != nil {
			if herr, ok := err.(*horizonclient.Error); ok {
				return response, (&jrpc2.Error{
					Code:    code.InvalidRequest,
					Message: herr.Problem.Title,
				}).WithData(herr.Problem.Extras)
			}
			return response, err
		}
		return response, nil
	})
}
