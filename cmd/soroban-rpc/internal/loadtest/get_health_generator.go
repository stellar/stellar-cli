package loadtest

import "github.com/creachadair/jrpc2"

// Generates simple getHealth requests. Useful as a baseline for load testing.
type GetHealthGenerator struct{}

func (generator *GetHealthGenerator) GenerateSpec() (jrpc2.Spec, error) {
	return jrpc2.Spec{Method: "getHealth"}, nil
}
