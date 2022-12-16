package methods

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"testing"
	"time"

	"github.com/stellar/go/clients/horizonclient"
	"github.com/stellar/go/protocols/horizon"
	"github.com/stellar/go/support/render/problem"
	"github.com/stellar/go/toid"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/mock"
)

func TestTopicFilterMatches(t *testing.T) {
	transferSym := xdr.ScSymbol("transfer")
	transfer := xdr.ScVal{
		Type: xdr.ScValTypeScvSymbol,
		Sym:  &transferSym,
	}
	sixtyfour := xdr.Int64(64)
	number := xdr.ScVal{
		Type: xdr.ScValTypeScvU63,
		U63:  &sixtyfour,
	}
	star := "*"
	for _, tc := range []struct {
		name     string
		filter   TopicFilter
		includes []xdr.ScVec
		excludes []xdr.ScVec
	}{
		{
			name:   "<empty>",
			filter: nil,
			includes: []xdr.ScVec{
				{},
			},
			excludes: []xdr.ScVec{
				{transfer},
			},
		},

		// Exact matching
		{
			name: "ScSymbol(transfer)",
			filter: []SegmentFilter{
				{scval: &transfer},
			},
			includes: []xdr.ScVec{
				{transfer},
			},
			excludes: []xdr.ScVec{
				{number},
				{transfer, transfer},
			},
		},

		// Star
		{
			name: "*",
			filter: []SegmentFilter{
				{wildcard: &star},
			},
			includes: []xdr.ScVec{
				{transfer},
			},
			excludes: []xdr.ScVec{
				{transfer, transfer},
			},
		},
		{
			name: "*/transfer",
			filter: []SegmentFilter{
				{wildcard: &star},
				{scval: &transfer},
			},
			includes: []xdr.ScVec{
				{number, transfer},
				{transfer, transfer},
			},
			excludes: []xdr.ScVec{
				{number},
				{number, number},
				{number, transfer, number},
				{transfer},
				{transfer, number},
				{transfer, transfer, transfer},
			},
		},
		{
			name: "transfer/*",
			filter: []SegmentFilter{
				{scval: &transfer},
				{wildcard: &star},
			},
			includes: []xdr.ScVec{
				{transfer, number},
				{transfer, transfer},
			},
			excludes: []xdr.ScVec{
				{number},
				{number, number},
				{number, transfer, number},
				{transfer},
				{number, transfer},
				{transfer, transfer, transfer},
			},
		},
		{
			name: "transfer/*/*",
			filter: []SegmentFilter{
				{scval: &transfer},
				{wildcard: &star},
				{wildcard: &star},
			},
			includes: []xdr.ScVec{
				{transfer, number, number},
				{transfer, transfer, transfer},
			},
			excludes: []xdr.ScVec{
				{number},
				{number, number},
				{number, transfer},
				{number, transfer, number, number},
				{transfer},
				{transfer, transfer, transfer, transfer},
			},
		},
		{
			name: "transfer/*/number",
			filter: []SegmentFilter{
				{scval: &transfer},
				{wildcard: &star},
				{scval: &number},
			},
			includes: []xdr.ScVec{
				{transfer, number, number},
				{transfer, transfer, number},
			},
			excludes: []xdr.ScVec{
				{number},
				{number, number},
				{number, number, number},
				{number, transfer, number},
				{transfer},
				{number, transfer},
				{transfer, transfer, transfer},
				{transfer, number, transfer},
			},
		},
	} {
		name := tc.name
		if name == "" {
			name = topicFilterToString(tc.filter)
		}
		t.Run(name, func(t *testing.T) {
			for _, include := range tc.includes {
				assert.True(
					t,
					tc.filter.Matches(include),
					"Expected %v filter to include %v",
					name,
					include,
				)
			}
			for _, exclude := range tc.excludes {
				assert.False(
					t,
					tc.filter.Matches(exclude),
					"Expected %v filter to exclude %v",
					name,
					exclude,
				)
			}

		})
	}
}

func TestTopicFilterJSON(t *testing.T) {
	var got TopicFilter

	assert.NoError(t, json.Unmarshal([]byte("[]"), &got))
	assert.Equal(t, TopicFilter{}, got)

	star := "*"
	assert.NoError(t, json.Unmarshal([]byte("[\"*\"]"), &got))
	assert.Equal(t, TopicFilter{{wildcard: &star}}, got)

	sixtyfour := xdr.Int64(64)
	scval := xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &sixtyfour}
	scvalstr, err := xdr.MarshalBase64(scval)
	assert.NoError(t, err)
	assert.NoError(t, json.Unmarshal([]byte(fmt.Sprintf("[%q]", scvalstr)), &got))
	assert.Equal(t, TopicFilter{{scval: &scval}}, got)
}

func topicFilterToString(t TopicFilter) string {
	var s []string
	for _, segment := range t {
		if segment.wildcard != nil {
			s = append(s, *segment.wildcard)
		} else if segment.scval != nil {
			out, err := xdr.MarshalBase64(*segment.scval)
			if err != nil {
				panic(err)
			}
			s = append(s, out)
		} else {
			panic("Invalid topic filter")
		}
	}
	if len(s) == 0 {
		s = append(s, "<empty>")
	}
	return strings.Join(s, "/")
}

func TestGetEventsRequestValid(t *testing.T) {
	assert.NoError(t, (&GetEventsRequest{
		StartLedger: 0,
		EndLedger:   0,
		Filters:     []EventFilter{},
		Pagination:  nil,
	}).Valid())

	assert.EqualError(t, (&GetEventsRequest{
		StartLedger: 1,
		EndLedger:   0,
		Filters:     []EventFilter{},
		Pagination:  nil,
	}).Valid(), "endLedger must be after or the same as startLedger")

	assert.EqualError(t, (&GetEventsRequest{
		StartLedger: 0,
		EndLedger:   4321,
		Filters:     []EventFilter{},
		Pagination:  nil,
	}).Valid(), "endLedger must be less than 4320 ledgers after startLedger")

	assert.EqualError(t, (&GetEventsRequest{
		StartLedger: 0,
		EndLedger:   0,
		Filters: []EventFilter{
			{}, {}, {}, {}, {}, {},
		},
		Pagination: nil,
	}).Valid(), "maximum 5 filters per request")

	assert.EqualError(t, (&GetEventsRequest{
		StartLedger: 0,
		EndLedger:   0,
		Filters: []EventFilter{
			{EventType: "foo"},
		},
		Pagination: nil,
	}).Valid(), "filter 1 invalid: if set, type must be either 'system' or 'contract'")

	assert.EqualError(t, (&GetEventsRequest{
		StartLedger: 0,
		EndLedger:   0,
		Filters: []EventFilter{
			{ContractIDs: []string{
				"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
				"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
				"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
				"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
				"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
				"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
			}},
		},
		Pagination: nil,
	}).Valid(), "filter 1 invalid: maximum 5 contract IDs per filter")

	assert.EqualError(t, (&GetEventsRequest{
		StartLedger: 0,
		EndLedger:   0,
		Filters: []EventFilter{
			{ContractIDs: []string{"a"}},
		},
		Pagination: nil,
	}).Valid(), "filter 1 invalid: contract ID 1 invalid")

	assert.EqualError(t, (&GetEventsRequest{
		StartLedger: 0,
		EndLedger:   0,
		Filters: []EventFilter{
			{
				Topics: []TopicFilter{
					{}, {}, {}, {}, {}, {},
				},
			},
		},
		Pagination: nil,
	}).Valid(), "filter 1 invalid: maximum 5 topics per filter")

	assert.EqualError(t, (&GetEventsRequest{
		StartLedger: 0,
		EndLedger:   0,
		Filters: []EventFilter{
			{Topics: []TopicFilter{
				{},
			}},
		},
		Pagination: nil,
	}).Valid(), "filter 1 invalid: topic 1 invalid: topic must have at least one segment")

	assert.EqualError(t, (&GetEventsRequest{
		StartLedger: 0,
		EndLedger:   0,
		Filters: []EventFilter{
			{Topics: []TopicFilter{
				{
					{},
					{},
					{},
					{},
					{},
				},
			}},
		},
		Pagination: nil,
	}).Valid(), "filter 1 invalid: topic 1 invalid: topic cannot have more than 4 segments")
}

func TestEventStoreForEachTransaction(t *testing.T) {
	t.Run("empty", func(t *testing.T) {
		client := &mockTransactionClient{}
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(1, 0, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(horizon.TransactionsPage{}, nil).Once()

		start := toid.New(1, 0, 0)
		finish := toid.New(2, 0, 0)
		found := []horizon.Transaction{}

		err := EventStore{Client: client}.ForEachTransaction(start, finish, func(tx horizon.Transaction) error {
			found = append(found, tx)
			return nil
		})
		assert.NoError(t, err)

		client.AssertExpectations(t)
		assert.Equal(t, []horizon.Transaction{}, found)
	})

	t.Run("one", func(t *testing.T) {
		client := &mockTransactionClient{}
		var result horizon.TransactionsPage
		result.Embedded.Records = []horizon.Transaction{
			{ID: "1", PT: toid.New(1, 0, 0).String()},
		}
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        result.Embedded.Records[0].PagingToken(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(result, nil).Once()

		start := toid.New(1, 0, 0)
		finish := toid.New(2, 0, 0)

		found := []horizon.Transaction{}
		called := 0
		err := EventStore{Client: client}.ForEachTransaction(start, finish, func(tx horizon.Transaction) error {
			called++
			found = append(found, tx)
			return nil
		})
		assert.NoError(t, err)

		assert.Equal(t, 1, called)
		client.AssertExpectations(t)
		// Should find all transactions
		assert.Equal(t, result.Embedded.Records, found)
	})

	t.Run("retry", func(t *testing.T) {
		client := &mockTransactionClient{}
		var result horizon.TransactionsPage
		result.Embedded.Records = []horizon.Transaction{
			{ID: "1", PT: toid.New(1, 0, 0).String()},
		}

		// Return an error the first time
		client.On("Transactions", mock.Anything).Return(horizon.TransactionsPage{}, horizonclient.Error{
			Response: &http.Response{
				StatusCode: http.StatusTooManyRequests,
			},
		}).Once()

		// Then success on the retry
		client.On("Transactions", mock.Anything).Return(result, nil).Once()

		start := toid.New(1, 0, 0)
		finish := toid.New(2, 0, 0)
		found := []horizon.Transaction{}
		called := 0
		err := EventStore{Client: client}.ForEachTransaction(start, finish, func(tx horizon.Transaction) error {
			called++
			found = append(found, tx)
			return nil
		})
		assert.NoError(t, err)

		assert.Equal(t, 1, called)
		client.AssertExpectations(t)
		// Should find all transactions
		assert.Equal(t, result.Embedded.Records, found)
	})

	t.Run("timeout", func(t *testing.T) {
		client := &mockTransactionClient{}
		var result horizon.TransactionsPage
		result.Embedded.Records = []horizon.Transaction{
			{ID: "1", PT: toid.New(1, 0, 0).String()},
		}

		// Return an error every time
		client.On("Transactions", mock.Anything).Return(horizon.TransactionsPage{}, horizonclient.Error{
			Response: &http.Response{StatusCode: http.StatusTooManyRequests},
			Problem:  problem.P{Title: "too many requests"},
		}).Times(7)

		start := toid.New(1, 0, 0)
		finish := toid.New(2, 0, 0)
		called := 0
		err := EventStore{Client: client}.ForEachTransaction(start, finish, func(tx horizon.Transaction) error {
			called++
			return nil
		})
		assert.EqualError(t, err, "horizon error: \"too many requests\" - check horizon.Error.Problem for more information")

		assert.Equal(t, 0, called)
		client.AssertExpectations(t)
	})

	t.Run("cuts off after f error", func(t *testing.T) {
		client := &mockTransactionClient{}
		var result horizon.TransactionsPage
		result.Embedded.Records = []horizon.Transaction{
			{ID: "1", PT: toid.New(1, 0, 0).String()},
			{ID: "2", PT: toid.New(2, 0, 0).String()},
		}
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        result.Embedded.Records[0].PagingToken(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(result, nil).Once()

		start := toid.New(1, 0, 0)
		finish := toid.New(2, 0, 0)

		found := []horizon.Transaction{}
		called := 0
		err := EventStore{Client: client}.ForEachTransaction(start, finish, func(tx horizon.Transaction) error {
			called++
			found = append(found, tx)
			return io.EOF
		})
		assert.EqualError(t, err, "EOF")

		assert.Equal(t, 1, called)
		client.AssertExpectations(t)
		// Should find just the first record
		assert.Equal(t, result.Embedded.Records[:1], found)
	})

	t.Run("pagination within a ledger", func(t *testing.T) {
		client := &mockTransactionClient{}
		result := []horizon.Transaction{}
		for i := 0; i < 210; i++ {
			result = append(
				result,
				horizon.Transaction{ID: fmt.Sprintf("%d", i), PT: toid.New(1, int32(i), 0).String()},
			)
		}
		pages := []horizon.TransactionsPage{{}, {}}
		pages[0].Embedded.Records = result[:200]
		pages[1].Embedded.Records = result[200:]
		start := toid.New(1, 0, 0)
		finish := toid.New(2, 0, 0)

		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        start.String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(pages[0], nil).Once()
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        pages[0].Embedded.Records[199].PagingToken(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(pages[1], nil).Once()

		found := []horizon.Transaction{}
		called := 0
		err := EventStore{Client: client}.ForEachTransaction(start, finish, func(tx horizon.Transaction) error {
			called++
			found = append(found, tx)
			return nil
		})
		assert.NoError(t, err)

		assert.Equal(t, 210, called)
		client.AssertExpectations(t)
		// Should find all transactions
		assert.Equal(t, result, found)
	})

	t.Run("pagination across ledgers", func(t *testing.T) {
		client := &mockTransactionClient{}
		result := []horizon.Transaction{}
		// Create two full pages, split across two ledgers
		for ledger := 1; ledger < 3; ledger++ {
			for i := 0; i < 180; i++ {
				result = append(
					result,
					horizon.Transaction{
						ID: fmt.Sprintf("%d-%d", ledger, i),
						PT: toid.New(int32(ledger), int32(i), 0).String(),
					},
				)
			}
		}
		pages := []horizon.TransactionsPage{{}, {}}
		pages[0].Embedded.Records = result[:200]
		pages[1].Embedded.Records = result[200:]
		start := toid.New(1, 0, 0)
		finish := toid.New(3, 0, 0)

		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        start.String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(pages[0], nil).Once()
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        pages[0].Embedded.Records[199].PagingToken(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(pages[1], nil).Once()

		found := []horizon.Transaction{}
		called := 0
		err := EventStore{Client: client}.ForEachTransaction(start, finish, func(tx horizon.Transaction) error {
			called++
			found = append(found, tx)
			return nil
		})
		assert.NoError(t, err)

		assert.Equal(t, 360, called)
		client.AssertExpectations(t)
		// Should find all transactions
		assert.Equal(t, result, found)
	})
}

func TestEventStoreGetEvents(t *testing.T) {
	now := time.Now()
	counter := xdr.ScSymbol("COUNTER")
	counterScVal := xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter}
	counterXdr, err := xdr.MarshalBase64(counterScVal)
	assert.NoError(t, err)

	t.Run("empty", func(t *testing.T) {
		client := &mockTransactionClient{}
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(1, 0, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(horizon.TransactionsPage{}, nil).Once()

		events, err := EventStore{Client: client}.GetEvents(GetEventsRequest{
			StartLedger: 1,
			EndLedger:   2,
		})
		assert.NoError(t, err)

		client.AssertExpectations(t)
		assert.Equal(t, []EventInfo(nil), events)
	})

	t.Run("no filtering returns all", func(t *testing.T) {
		client := &mockTransactionClient{}
		results := []horizon.Transaction{}
		contractId := xdr.Hash([32]byte{})
		for i := 0; i < 10; i++ {
			meta := transactionMetaWithEvents(t,
				[]xdr.ContractEvent{
					contractEvent(
						contractId,
						xdr.ScVec{xdr.ScVal{
							Type: xdr.ScValTypeScvSymbol,
							Sym:  &counter,
						}},
						xdr.ScVal{
							Type: xdr.ScValTypeScvSymbol,
							Sym:  &counter,
						},
					),
				},
			)
			results = append(results, horizon.Transaction{
				ID:              fmt.Sprintf("%d", i),
				PT:              toid.New(1, int32(i), 0).String(),
				Ledger:          1,
				LedgerCloseTime: now,
				ResultMetaXdr:   meta,
			})
		}
		page := horizon.TransactionsPage{}
		page.Embedded.Records = results
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(1, 0, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(page, nil).Once()

		events, err := EventStore{Client: client}.GetEvents(GetEventsRequest{
			StartLedger: 1,
			EndLedger:   2,
		})
		assert.NoError(t, err)

		client.AssertExpectations(t)
		expected := []EventInfo{}
		for i, tx := range results {
			id := EventID{ID: toid.New(tx.Ledger, int32(i), 0), EventOrder: 0}.String()
			value, err := xdr.MarshalBase64(xdr.ScVal{
				Type: xdr.ScValTypeScvSymbol,
				Sym:  &counter,
			})
			assert.NoError(t, err)
			expected = append(expected, EventInfo{
				EventType:      "contract",
				Ledger:         tx.Ledger,
				LedgerClosedAt: tx.LedgerCloseTime.Format(time.RFC3339),
				ContractID:     "0000000000000000000000000000000000000000000000000000000000000000",
				ID:             id,
				PagingToken:    id,
				Topic:          []string{value},
				Value: EventInfoValue{
					XDR: value,
				},
			})
		}
		assert.Equal(t, expected, events)
	})

	t.Run("filtering by contract id", func(t *testing.T) {
		client := &mockTransactionClient{}
		results := []horizon.Transaction{}
		contractIds := []xdr.Hash{
			xdr.Hash([32]byte{}),
			xdr.Hash([32]byte{1}),
		}
		for i := 0; i < 5; i++ {
			meta := transactionMetaWithEvents(t,
				[]xdr.ContractEvent{
					contractEvent(
						contractIds[i%len(contractIds)],
						xdr.ScVec{xdr.ScVal{
							Type: xdr.ScValTypeScvSymbol,
							Sym:  &counter,
						}},
						xdr.ScVal{
							Type: xdr.ScValTypeScvSymbol,
							Sym:  &counter,
						},
					),
				},
			)
			results = append(results, horizon.Transaction{
				ID:              fmt.Sprintf("%d", i),
				PT:              toid.New(1, int32(i), 0).String(),
				Ledger:          1,
				LedgerCloseTime: now,
				ResultMetaXdr:   meta,
			})
		}
		page := horizon.TransactionsPage{}
		page.Embedded.Records = results
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(1, 0, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(page, nil).Once()

		events, err := EventStore{Client: client}.GetEvents(GetEventsRequest{
			StartLedger: 1,
			EndLedger:   2,
			Filters: []EventFilter{
				{ContractIDs: []string{contractIds[0].HexString()}},
			},
		})
		assert.NoError(t, err)

		client.AssertExpectations(t)
		expectedIds := []string{
			EventID{ID: toid.New(1, int32(0), 0), EventOrder: 0}.String(),
			EventID{ID: toid.New(1, int32(2), 0), EventOrder: 0}.String(),
			EventID{ID: toid.New(1, int32(4), 0), EventOrder: 0}.String(),
		}
		eventIds := []string{}
		for _, event := range events {
			eventIds = append(eventIds, event.ID)
		}
		assert.Equal(t, expectedIds, eventIds)
	})

	t.Run("filtering by topic", func(t *testing.T) {
		client := &mockTransactionClient{}
		results := []horizon.Transaction{}
		contractId := xdr.Hash([32]byte{})
		for i := 0; i < 10; i++ {
			number := xdr.Int64(i)
			meta := transactionMetaWithEvents(t,
				[]xdr.ContractEvent{
					// Generate a unique topic like /counter/4 for each event so we can check
					contractEvent(
						contractId,
						xdr.ScVec{
							xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter},
							xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
						},
						xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
					),
				},
			)
			results = append(results, horizon.Transaction{
				ID:              fmt.Sprintf("%d", i),
				PT:              toid.New(1, int32(i), 0).String(),
				Ledger:          1,
				LedgerCloseTime: now,
				ResultMetaXdr:   meta,
			})
		}
		page := horizon.TransactionsPage{}
		page.Embedded.Records = results
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(1, 0, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(page, nil).Once()

		number := xdr.Int64(4)
		events, err := EventStore{Client: client}.GetEvents(GetEventsRequest{
			StartLedger: 1,
			EndLedger:   2,
			Filters: []EventFilter{
				{Topics: []TopicFilter{
					[]SegmentFilter{
						{scval: &xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter}},
						{scval: &xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number}},
					},
				}},
			},
		})
		assert.NoError(t, err)

		client.AssertExpectations(t)
		tx := results[4]
		id := EventID{ID: toid.New(tx.Ledger, int32(4), 0), EventOrder: 0}.String()
		assert.NoError(t, err)
		value, err := xdr.MarshalBase64(xdr.ScVal{
			Type: xdr.ScValTypeScvU63,
			U63:  &number,
		})
		assert.NoError(t, err)
		expected := []EventInfo{
			{
				EventType:      "contract",
				Ledger:         tx.Ledger,
				LedgerClosedAt: tx.LedgerCloseTime.Format(time.RFC3339),
				ContractID:     "0000000000000000000000000000000000000000000000000000000000000000",
				ID:             id,
				PagingToken:    id,
				Topic:          []string{counterXdr, value},
				Value:          EventInfoValue{XDR: value},
			},
		}
		assert.Equal(t, expected, events)
	})

	t.Run("filtering by both contract id and topic", func(t *testing.T) {
		client := &mockTransactionClient{}
		contractId := xdr.Hash([32]byte{})
		otherContractId := xdr.Hash([32]byte{1})
		number := xdr.Int64(1)
		results := []horizon.Transaction{
			// This matches neither the contract id nor the topic
			{
				ID:              fmt.Sprintf("%d", 0),
				PT:              toid.New(1, int32(0), 0).String(),
				Ledger:          1,
				LedgerCloseTime: now,
				ResultMetaXdr: transactionMetaWithEvents(t,
					[]xdr.ContractEvent{
						contractEvent(
							otherContractId,
							xdr.ScVec{
								xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter},
							},
							xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
						),
					},
				),
			},
			// This matches the contract id but not the topic
			{
				ID:              fmt.Sprintf("%d", 1),
				PT:              toid.New(1, int32(1), 0).String(),
				Ledger:          1,
				LedgerCloseTime: now,
				ResultMetaXdr: transactionMetaWithEvents(t,
					[]xdr.ContractEvent{
						contractEvent(
							contractId,
							xdr.ScVec{
								xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter},
							},
							xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
						),
					},
				),
			},
			// This matches the topic but not the contract id
			{
				ID:              fmt.Sprintf("%d", 2),
				PT:              toid.New(1, int32(2), 0).String(),
				Ledger:          1,
				LedgerCloseTime: now,
				ResultMetaXdr: transactionMetaWithEvents(t,
					[]xdr.ContractEvent{
						contractEvent(
							otherContractId,
							xdr.ScVec{
								xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter},
								xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
							},
							xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
						),
					},
				),
			},
			// This matches both the contract id and the topic
			{
				ID:              fmt.Sprintf("%d", 3),
				PT:              toid.New(1, int32(3), 0).String(),
				Ledger:          1,
				LedgerCloseTime: now,
				ResultMetaXdr: transactionMetaWithEvents(t,
					[]xdr.ContractEvent{
						contractEvent(
							contractId,
							xdr.ScVec{
								xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter},
								xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
							},
							xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
						),
					},
				),
			},
		}
		page := horizon.TransactionsPage{}
		page.Embedded.Records = results
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(1, 0, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(page, nil).Once()

		events, err := EventStore{Client: client}.GetEvents(GetEventsRequest{
			StartLedger: 1,
			EndLedger:   2,
			Filters: []EventFilter{
				{
					ContractIDs: []string{contractId.HexString()},
					Topics: []TopicFilter{
						[]SegmentFilter{
							{scval: &xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter}},
							{scval: &xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number}},
						},
					},
				},
			},
		})
		assert.NoError(t, err)

		client.AssertExpectations(t)
		tx := results[3]
		id := EventID{ID: toid.New(tx.Ledger, int32(3), 0), EventOrder: 0}.String()
		value, err := xdr.MarshalBase64(xdr.ScVal{
			Type: xdr.ScValTypeScvU63,
			U63:  &number,
		})
		assert.NoError(t, err)
		expected := []EventInfo{
			{
				EventType:      "contract",
				Ledger:         tx.Ledger,
				LedgerClosedAt: tx.LedgerCloseTime.Format(time.RFC3339),
				ContractID:     contractId.HexString(),
				ID:             id,
				PagingToken:    id,
				Topic:          []string{counterXdr, value},
				Value:          EventInfoValue{XDR: value},
			},
		}
		assert.Equal(t, expected, events)
	})

	t.Run("filtering by event type", func(t *testing.T) {
		client := &mockTransactionClient{}
		contractId := xdr.Hash([32]byte{})
		results := []horizon.Transaction{
			{
				ID:              fmt.Sprintf("%d", 0),
				PT:              toid.New(1, int32(0), 0).String(),
				Ledger:          1,
				LedgerCloseTime: now,
				ResultMetaXdr: transactionMetaWithEvents(t,
					[]xdr.ContractEvent{
						contractEvent(
							contractId,
							xdr.ScVec{
								xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter},
							},
							xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter},
						),
						systemEvent(
							contractId,
							xdr.ScVec{
								xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter},
							},
							xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &counter},
						),
					},
				),
			},
		}
		page := horizon.TransactionsPage{}
		page.Embedded.Records = results
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(1, 0, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(page, nil).Once()

		events, err := EventStore{Client: client}.GetEvents(GetEventsRequest{
			StartLedger: 1,
			EndLedger:   2,
			Filters: []EventFilter{
				{EventType: "system"},
			},
		})
		assert.NoError(t, err)

		client.AssertExpectations(t)
		tx := results[0]
		id := EventID{ID: toid.New(tx.Ledger, int32(0), 0), EventOrder: 1}.String()
		expected := []EventInfo{
			{
				EventType:      "system",
				Ledger:         tx.Ledger,
				LedgerClosedAt: tx.LedgerCloseTime.Format(time.RFC3339),
				ContractID:     contractId.HexString(),
				ID:             id,
				PagingToken:    id,
				Topic:          []string{counterXdr},
				Value:          EventInfoValue{XDR: counterXdr},
			},
		}
		assert.Equal(t, expected, events)
	})

	t.Run("pagination", func(t *testing.T) {
		client := &mockTransactionClient{}
		contractId := xdr.Hash([32]byte{})
		results := []horizon.Transaction{}
		for i := 0; i < 180; i++ {
			number := xdr.Int64(i)
			results = append(results, horizon.Transaction{
				ID:              fmt.Sprintf("%d", i),
				PT:              toid.New(1, int32(i), 0).String(),
				Ledger:          1,
				LedgerCloseTime: now,
				ResultMetaXdr: transactionMetaWithEvents(t,
					[]xdr.ContractEvent{
						contractEvent(
							contractId,
							xdr.ScVec{
								xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
							},
							xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
						),
					},
				),
			})
		}
		for i := 180; i < 210; i++ {
			number := xdr.Int64(i)
			results = append(results, horizon.Transaction{
				ID:              fmt.Sprintf("%d", i),
				PT:              toid.New(2, int32(i-180), 0).String(),
				Ledger:          2,
				LedgerCloseTime: now,
				ResultMetaXdr: transactionMetaWithEvents(t,
					[]xdr.ContractEvent{
						contractEvent(
							contractId,
							xdr.ScVec{
								xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
							},
							xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number},
						),
					},
				),
			})
		}
		pages := []horizon.TransactionsPage{{}, {}}
		pages[0].Embedded.Records = results[:200]
		pages[1].Embedded.Records = results[200:]
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(1, 0, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(pages[0], nil).Once()
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(2, 19, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(pages[1], nil).Once()

		// Find one on the second page
		number := xdr.Int64(205)
		numberScVal := xdr.ScVal{Type: xdr.ScValTypeScvU63, U63: &number}
		numberXdr, err := xdr.MarshalBase64(numberScVal)
		assert.NoError(t, err)

		events, err := EventStore{Client: client}.GetEvents(GetEventsRequest{
			StartLedger: 1,
			EndLedger:   2,
			Filters: []EventFilter{
				{Topics: []TopicFilter{
					[]SegmentFilter{
						{scval: &numberScVal},
					},
				}},
			},
		})
		assert.NoError(t, err)

		client.AssertExpectations(t)
		tx := results[205]
		id := EventID{ID: toid.New(tx.Ledger, int32(25), 0), EventOrder: 0}.String()
		expected := []EventInfo{
			{
				EventType:      "contract",
				Ledger:         tx.Ledger,
				LedgerClosedAt: tx.LedgerCloseTime.Format(time.RFC3339),
				ContractID:     contractId.HexString(),
				ID:             id,
				PagingToken:    id,
				Topic:          []string{numberXdr},
				Value:          EventInfoValue{XDR: numberXdr},
			},
		}
		assert.Equal(t, expected, events)
	})

	t.Run("starting cursor in the middle of operations and events", func(t *testing.T) {
		client := &mockTransactionClient{}
		contractId := xdr.Hash([32]byte{})
		results := []horizon.Transaction{}
		datas := []xdr.ScSymbol{
			// ledger/transaction/operation/event
			xdr.ScSymbol("5/2/0/0"),
			xdr.ScSymbol("5/2/0/1"),
			xdr.ScSymbol("5/2/1/0"),
			xdr.ScSymbol("5/2/1/1"),
		}
		results = append(results, horizon.Transaction{
			ID:              "l5/t2",
			PT:              toid.New(5, int32(2), 0).String(),
			Ledger:          5,
			LedgerCloseTime: now,
			ResultMetaXdr: transactionMetaWithEvents(t,
				[]xdr.ContractEvent{
					contractEvent(
						contractId,
						xdr.ScVec{
							counterScVal,
						},
						xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &datas[0]},
					),
					contractEvent(
						contractId,
						xdr.ScVec{
							counterScVal,
						},
						xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &datas[1]},
					),
				},
				[]xdr.ContractEvent{
					contractEvent(
						contractId,
						xdr.ScVec{
							counterScVal,
						},
						xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &datas[2]},
					),
					contractEvent(
						contractId,
						xdr.ScVec{
							counterScVal,
						},
						xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &datas[3]},
					),
				},
			),
		})
		page := horizon.TransactionsPage{}
		page.Embedded.Records = results
		client.On("Transactions", horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        toid.New(5, 2, 0).String(),
			Limit:         200,
			IncludeFailed: false,
		}).Return(page, nil).Once()

		id := EventID{ID: toid.New(5, 2, 1), EventOrder: 1}.String()
		events, err := EventStore{Client: client}.GetEvents(GetEventsRequest{
			StartLedger: 1,
			EndLedger:   6,
			Pagination: &PaginationOptions{
				Cursor: id,
			},
		})
		assert.NoError(t, err)

		expectedXdr, err := xdr.MarshalBase64(xdr.ScVal{Type: xdr.ScValTypeScvSymbol, Sym: &datas[len(datas)-1]})
		assert.NoError(t, err)
		client.AssertExpectations(t)
		tx := results[0]
		expected := []EventInfo{
			{
				EventType:      "contract",
				Ledger:         tx.Ledger,
				LedgerClosedAt: tx.LedgerCloseTime.Format(time.RFC3339),
				ContractID:     contractId.HexString(),
				ID:             id,
				PagingToken:    id,
				Topic:          []string{counterXdr},
				Value:          EventInfoValue{XDR: expectedXdr},
			},
		}
		assert.Equal(t, expected, events)
	})
}

func transactionMetaWithEvents(t *testing.T, events ...[]xdr.ContractEvent) string {
	operationEvents := []xdr.OperationEvents{}
	for _, e := range events {
		operationEvents = append(operationEvents, xdr.OperationEvents{
			Events: e,
		})
	}
	meta, err := xdr.MarshalBase64(xdr.TransactionMeta{
		V:          3,
		Operations: &[]xdr.OperationMeta{},
		V3: &xdr.TransactionMetaV3{
			TxResult: xdr.TransactionResult{
				Result: xdr.TransactionResultResult{
					InnerResultPair: &xdr.InnerTransactionResultPair{},
					Results:         &[]xdr.OperationResult{},
				},
			},
			Events: operationEvents,
		},
	})
	assert.NoError(t, err)
	return meta
}

func contractEvent(contractId xdr.Hash, topic []xdr.ScVal, body xdr.ScVal) xdr.ContractEvent {
	return xdr.ContractEvent{
		ContractId: &contractId,
		Type:       xdr.ContractEventTypeContract,
		Body: xdr.ContractEventBody{
			V: 0,
			V0: &xdr.ContractEventV0{
				Topics: xdr.ScVec(topic),
				Data:   body,
			},
		},
	}
}

func systemEvent(contractId xdr.Hash, topic []xdr.ScVal, body xdr.ScVal) xdr.ContractEvent {
	return xdr.ContractEvent{
		ContractId: &contractId,
		Type:       xdr.ContractEventTypeSystem,
		Body: xdr.ContractEventBody{
			V: 0,
			V0: &xdr.ContractEventV0{
				Topics: xdr.ScVec(topic),
				Data:   body,
			},
		},
	}
}

type mockTransactionClient struct {
	mock.Mock
}

func (m *mockTransactionClient) Transactions(request horizonclient.TransactionRequest) (horizon.TransactionsPage, error) {
	args := m.Called(request)
	return args.Get(0).(horizon.TransactionsPage), args.Error(1)
}
