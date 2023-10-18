package loadtest

import "github.com/creachadair/jrpc2"

// Implement SpecGenerator to test different types request load.
type SpecGenerator interface {
	GenerateSpec() (jrpc2.Spec, error)
}
