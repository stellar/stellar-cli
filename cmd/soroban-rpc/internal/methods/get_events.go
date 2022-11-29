package methods

import (
	"context"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"
	"github.com/stellar/go/clients/horizonclient"
	"github.com/stellar/go/protocols/horizon"
	"github.com/stellar/go/support/errors"
	"github.com/stellar/go/toid"
	"github.com/stellar/go/xdr"
)

// MAX_LEDGER_RANGE is the maximum allowed value of endLedger-startLedger
// Just guessed 4320 as it is ~6hrs
const MAX_LEDGER_RANGE = 4320

type EventInfo struct {
	EventType      string         `json:"type"`
	Ledger         int32          `json:"ledger,string"`
	LedgerClosedAt string         `json:"ledgerClosedAt"`
	ContractID     string         `json:"contractId"`
	ID             string         `json:"id"`
	PagingToken    string         `json:"pagingToken"`
	Topic          []string       `json:"topic"`
	Value          EventInfoValue `json:"value"`
}

type EventInfoValue struct {
	XDR string `json:"xdr"`
}

type GetEventsRequest struct {
	StartLedger int32              `json:"startLedger,string"`
	EndLedger   int32              `json:"endLedger,string"`
	Filters     []EventFilter      `json:"filters"`
	Pagination  *PaginationOptions `json:"pagination,omitempty"`
}

func (g *GetEventsRequest) Valid() error {
	// Validate start & end ledger
	// Validate the ledger range min/max
	if g.EndLedger < g.StartLedger {
		return errors.New("endLedger must be after or the same as startLedger")
	}
	if g.EndLedger-g.StartLedger > MAX_LEDGER_RANGE {
		return fmt.Errorf("endLedger must be less than %d ledgers after startLedger", MAX_LEDGER_RANGE)
	}

	// Validate filters
	if len(g.Filters) > 5 {
		return errors.New("maximum 5 filters per request")
	}
	for i, filter := range g.Filters {
		if err := filter.Valid(); err != nil {
			return errors.Wrapf(err, "filter %d invalid", i+1)
		}
	}

	return nil
}

func (g *GetEventsRequest) Matches(event xdr.ContractEvent) bool {
	if len(g.Filters) == 0 {
		return true
	}
	for _, filter := range g.Filters {
		if filter.Matches(event) {
			return true
		}
	}
	return false
}

type EventFilter struct {
	EventType   string        `json:"type,omitempty"`
	ContractIDs []string      `json:"contractIds,omitempty"`
	Topics      []TopicFilter `json:"topics,omitempty"`
}

func (e *EventFilter) Valid() error {
	switch e.EventType {
	case "", "system", "contract":
		// ok
	default:
		return errors.New("if set, type must be either 'system' or 'contract'")
	}
	if len(e.ContractIDs) > 5 {
		return errors.New("maximum 5 contract IDs per filter")
	}
	if len(e.Topics) > 5 {
		return errors.New("maximum 5 topics per filter")
	}
	for i, id := range e.ContractIDs {
		out, err := hex.DecodeString(id)
		if err != nil || len(out) != 32 {
			return fmt.Errorf("contract ID %d invalid", i+1)
		}
	}
	for i, topic := range e.Topics {
		if err := topic.Valid(); err != nil {
			return errors.Wrapf(err, "topic %d invalid", i+1)
		}
	}
	return nil
}

// TODO: Implement this more efficiently (ideally do it in the real data backend)
func (e *EventFilter) Matches(event xdr.ContractEvent) bool {
	return e.matchesEventType(event) && e.matchesContractIDs(event) && e.matchesTopics(event)
}

func (e *EventFilter) matchesEventType(event xdr.ContractEvent) bool {
	if e.EventType == "contract" && event.Type != xdr.ContractEventTypeContract {
		return false
	}
	if e.EventType == "system" && event.Type != xdr.ContractEventTypeSystem {
		return false
	}
	return true
}

func (e *EventFilter) matchesContractIDs(event xdr.ContractEvent) bool {
	if len(e.ContractIDs) == 0 {
		return true
	}
	if event.ContractId == nil {
		return false
	}
	needle := hex.EncodeToString((*event.ContractId)[:])
	for _, id := range e.ContractIDs {
		if id == needle {
			return true
		}
	}
	return false
}

func (e *EventFilter) matchesTopics(event xdr.ContractEvent) bool {
	if len(e.Topics) == 0 {
		return true
	}
	v0 := event.Body.MustV0()
	for _, topicFilter := range e.Topics {
		if topicFilter.Matches(v0.Topics) {
			return true
		}
	}
	return false
}

type TopicFilter []SegmentFilter

func (t *TopicFilter) Valid() error {
	if len(*t) < 1 {
		return errors.New("topic must have at least one segment")
	}
	if len(*t) > 4 {
		return errors.New("topic cannot have more than 4 segments")
	}
	return nil
}

func (t TopicFilter) Matches(event []xdr.ScVal) bool {
	for _, segmentFilter := range t {
		if segmentFilter.wildcard != nil {
			switch *segmentFilter.wildcard {
			case "*":
				// one-segment wildcard
				if len(event) == 0 {
					// Nothing to match, need one segment.
					return false
				}
				// Ignore this token
				event = event[1:]
			default:
				panic("invalid segmentFilter")
			}
		} else if segmentFilter.scval != nil {
			// Exact match the scval
			if len(event) == 0 || !segmentFilter.scval.Equals(event[0]) {
				return false
			}
			event = event[1:]
		} else {
			panic("invalid segmentFilter")
		}
	}
	// Check we had no leftovers
	return len(event) == 0
}

type SegmentFilter struct {
	wildcard *string
	scval    *xdr.ScVal
}

func (s *SegmentFilter) UnmarshalJSON(p []byte) error {
	s.wildcard = nil
	s.scval = nil

	var tmp string
	if err := json.Unmarshal(p, &tmp); err != nil {
		return err
	}
	if tmp == "*" {
		s.wildcard = &tmp
	} else {
		var out xdr.ScVal
		if err := xdr.SafeUnmarshalBase64(tmp, &out); err != nil {
			return err
		}
		s.scval = &out
	}
	return nil
}

type PaginationOptions struct {
	Cursor string `json:"cursor,omitempty"`
	Limit  uint   `json:"limit,omitempty"`
}

type EventStore struct {
	Client TransactionClient
}

type TransactionClient interface {
	Transactions(request horizonclient.TransactionRequest) (horizon.TransactionsPage, error)
}

// TODO: Extract this to a new package 'eventid'
// Build a lexically order-able id for this event record. This is
// based on Horizon's db2/history.Effect.ID method.
type EventID struct {
	*toid.ID
	EventOrder int32
}

// String returns a string representation of this id
func (id EventID) String() string {
	return fmt.Sprintf(
		"%019d-%010d",
		id.ToInt64(),
		id.EventOrder+1,
	)
}

func (id *EventID) Parse(input string) error {
	parts := strings.SplitN(input, "-", 2)
	if len(parts) != 2 {
		return fmt.Errorf("invalid event id %s", input)
	}

	// Parse the first part (toid)
	idInt, err := strconv.ParseInt(parts[0], 10, 64)
	if err != nil {
		return errors.Wrapf(err, "invalid event id %s", input)
	}
	parsed := toid.Parse(idInt)
	id.ID = &parsed

	// Parse the second part (event order)
	eventOrder, err := strconv.ParseInt(parts[1], 10, 64)
	if err != nil {
		return errors.Wrapf(err, "invalid event id %s", input)
	}
	// Subtract one to go from the id to the
	id.EventOrder = int32(eventOrder) - 1

	return nil
}

func (a EventStore) GetEvents(request GetEventsRequest) ([]EventInfo, error) {
	if err := request.Valid(); err != nil {
		return nil, err
	}

	finish := toid.AfterLedger(request.EndLedger)

	var results []EventInfo

	// TODO: Use a more efficient backend here. For now, we stream all ledgers in
	// the range from horizon, and filter them. This sucks.
	cursor := EventID{
		ID:         toid.New(request.StartLedger, int32(0), 0),
		EventOrder: 0,
	}
	if request.Pagination != nil && request.Pagination.Cursor != "" {
		if err := cursor.Parse(request.Pagination.Cursor); err != nil {
			return nil, errors.Wrap(err, "invalid pagination cursor")
		}
	}
	err := a.ForEachTransaction(cursor.ID, finish, func(transaction horizon.Transaction) error {
		// parse the txn paging-token, to get the transactionIndex
		pagingTokenInt, err := strconv.ParseInt(transaction.PagingToken(), 10, 64)
		if err != nil {
			return errors.Wrapf(err, "invalid paging token %s", transaction.PagingToken())
		}
		pagingToken := toid.Parse(pagingTokenInt)

		// For the first txn, we might have to skip some events to get the first
		// after the cursor.
		operationCursor := cursor.OperationOrder
		eventCursor := cursor.EventOrder
		cursor.OperationOrder = 0
		cursor.EventOrder = 0
		if pagingToken.ToInt64() > cursor.ToInt64() {
			// This transaction is after the cursor, so we need to reset the cursor
			operationCursor = 0
			eventCursor = 0
		}

		var meta xdr.TransactionMeta
		if err := xdr.SafeUnmarshalBase64(transaction.ResultMetaXdr, &meta); err != nil {
			// Invalid meta back. Eek!
			return err
		}

		v3, ok := meta.GetV3()
		if !ok {
			return nil
		}

		ledger := transaction.Ledger
		ledgerClosedAt := transaction.LedgerCloseTime.Format(time.RFC3339)

		for operationIndex, operationEvents := range v3.Events {
			if int32(operationIndex) < operationCursor {
				continue
			}
			for eventIndex, event := range operationEvents.Events {
				if int32(eventIndex) < eventCursor {
					continue
				}
				if request.Matches(event) {
					v0 := event.Body.MustV0()

					eventType := "contract"
					if event.Type == xdr.ContractEventTypeSystem {
						eventType = "system"
					}

					// Build a lexically order-able id for this event record. This is
					// based on Horizon's db2/history.Effect.ID method.
					id := EventID{
						ID:         toid.New(ledger, pagingToken.TransactionOrder, int32(operationIndex)),
						EventOrder: int32(eventIndex),
					}.String()

					// base64-xdr encode the topic
					topic := make([]string, 0, 4)
					for _, segment := range v0.Topics {
						seg, err := xdr.MarshalBase64(segment)
						if err != nil {
							return err
						}
						topic = append(topic, seg)
					}

					// base64-xdr encode the data
					data, err := xdr.MarshalBase64(v0.Data)
					if err != nil {
						return err
					}

					results = append(results, EventInfo{
						EventType:      eventType,
						Ledger:         ledger,
						LedgerClosedAt: ledgerClosedAt,
						ContractID:     hex.EncodeToString((*event.ContractId)[:]),
						ID:             id,
						PagingToken:    id,
						Topic:          topic,
						Value:          EventInfoValue{XDR: data},
					})

					// Check if we've gotten "limit" events
					if request.Pagination != nil && request.Pagination.Limit > 0 && uint(len(results)) >= request.Pagination.Limit {
						return io.EOF
					}
				}
			}
		}
		return nil
	})
	if err == io.EOF {
		err = nil
	}
	return results, err
}

// ForEachTransaction runs f for each transaction in a range from start
// (inclusive) to finish (exclusive). If f returns any error,
// ForEachTransaction stops immediately and returns that error.
func (a EventStore) ForEachTransaction(start, finish *toid.ID, f func(transaction horizon.Transaction) error) error {
	delay := 10 * time.Millisecond
	cursor := toid.New(start.LedgerSequence, start.TransactionOrder, 0)
	for {
		transactions, err := a.Client.Transactions(horizonclient.TransactionRequest{
			Order:         horizonclient.Order("asc"),
			Cursor:        cursor.String(),
			Limit:         200,
			IncludeFailed: false,
		})
		if err != nil {
			hErr := horizonclient.GetError(err)
			if hErr != nil && hErr.Response != nil && (hErr.Response.StatusCode == http.StatusTooManyRequests || hErr.Response.StatusCode >= 500) {
				// rate-limited, or horizon server-side error, we can retry.

				// exponential backoff, to not hammer Horizon
				delay *= 2

				if delay > time.Second {
					return err
				}

				// retry
				time.Sleep(delay)
				continue
			} else {
				// Unknown error, bail.
				return err
			}
		}

		for _, transaction := range transactions.Embedded.Records {
			pt, err := strconv.ParseInt(transaction.PagingToken(), 10, 64)
			if err != nil {
				return errors.Wrapf(err, "invalid paging token %s", transaction.PagingToken())
			}
			if pt >= finish.ToInt64() {
				// Done!
				return nil
			}
			id := toid.Parse(pt)
			cursor = &id

			if err := f(transaction); err != nil {
				return err
			}
		}

		if len(transactions.Embedded.Records) < 200 {
			// Did not return "limit" transactions, and the query is open-ended, so this must be the end.
			return nil
		}
	}
}

// NewGetEventsHandler returns a json rpc handler to fetch and filter events
func NewGetEventsHandler(store EventStore) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request GetEventsRequest) ([]EventInfo, error) {
		response, err := store.GetEvents(request)
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
