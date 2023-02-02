package methods

import (
	"context"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"

	"github.com/stellar/go/support/errors"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/events"
)

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
	Filters     []EventFilter      `json:"filters"`
	Pagination  *PaginationOptions `json:"pagination,omitempty"`
}

func (g *GetEventsRequest) Valid(maxLimit uint) error {
	// Validate start
	// Validate the paging limit (if it exists)
	if g.StartLedger <= 0 {
		return errors.New("startLedger must be positive")
	}
	if g.Pagination != nil && g.Pagination.Limit > maxLimit {
		return fmt.Errorf("limit must not exceed %d", maxLimit)
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

const EventTypeSystem = "system"
const EventTypeContract = "contract"

type EventFilter struct {
	EventType   string        `json:"type,omitempty"`
	ContractIDs []string      `json:"contractIds,omitempty"`
	Topics      []TopicFilter `json:"topics,omitempty"`
}

func (e *EventFilter) Valid() error {
	switch e.EventType {
	case "", EventTypeSystem, EventTypeContract:
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

func (e *EventFilter) Matches(event xdr.ContractEvent) bool {
	return e.matchesEventType(event) && e.matchesContractIDs(event) && e.matchesTopics(event)
}

func (e *EventFilter) matchesEventType(event xdr.ContractEvent) bool {
	if e.EventType == EventTypeContract && event.Type != xdr.ContractEventTypeContract {
		return false
	}
	if e.EventType == EventTypeSystem && event.Type != xdr.ContractEventTypeSystem {
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
	v0, ok := event.Body.GetV0()
	if !ok {
		return false
	}
	for _, topicFilter := range e.Topics {
		if topicFilter.Matches(v0.Topics) {
			return true
		}
	}
	return false
}

type TopicFilter []SegmentFilter

const minTopicCount = 1
const maxTopicCount = 4

func (t *TopicFilter) Valid() error {
	if len(*t) < minTopicCount {
		return errors.New("topic must have at least one segment")
	}
	if len(*t) > maxTopicCount {
		return errors.New("topic cannot have more than 4 segments")
	}
	for i, segment := range *t {
		if err := segment.Valid(); err != nil {
			return errors.Wrapf(err, "segment %d invalid", i+1)
		}
	}
	return nil
}

// An event matches a topic filter iff:
//   - the event has EXACTLY as many topic segments as the filter AND
//   - each segment either: matches exactly OR is a wildcard.
func (t TopicFilter) Matches(event []xdr.ScVal) bool {
	if len(event) != len(t) {
		return false
	}

	for i, segmentFilter := range t {
		if !segmentFilter.Matches(event[i]) {
			return false
		}
	}

	return true
}

type SegmentFilter struct {
	wildcard *string
	scval    *xdr.ScVal
}

func (s *SegmentFilter) Matches(segment xdr.ScVal) bool {
	if s.wildcard != nil && *s.wildcard == "*" {
		return true
	} else if s.scval != nil {
		if !s.scval.Equals(segment) {
			return false
		}
	} else {
		panic("invalid segmentFilter")
	}

	return true
}

func (s *SegmentFilter) Valid() error {
	if s.wildcard != nil && s.scval != nil {
		return errors.New("cannot set both wildcard and scval")
	}
	if s.wildcard == nil && s.scval == nil {
		return errors.New("must set either wildcard or scval")
	}
	if s.wildcard != nil && *s.wildcard != "*" {
		return errors.New("wildcard must be '*'")
	}
	return nil
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

type eventScanner interface {
	Scan(eventRange events.Range, f func(xdr.ContractEvent, events.Cursor, int64) bool) error
}

type eventsRPCHandler struct {
	scanner      eventScanner
	maxLimit     uint
	defaultLimit uint
}

func (h eventsRPCHandler) getEvents(request GetEventsRequest) ([]EventInfo, error) {
	if err := request.Valid(h.maxLimit); err != nil {
		return nil, &jrpc2.Error{
			Code:    code.InvalidParams,
			Message: err.Error(),
		}
	}

	start := events.Cursor{Ledger: uint32(request.StartLedger)}
	limit := h.defaultLimit
	if request.Pagination != nil {
		if request.Pagination.Cursor != "" {
			var err error
			start, err = events.ParseCursor(request.Pagination.Cursor)
			if err != nil {
				return nil, errors.Wrap(err, "invalid pagination cursor")
			}
			// increment event index because, when paginating,
			// we start with the item right after the cursor
			start.Event++
		}
		if request.Pagination.Limit > 0 {
			limit = request.Pagination.Limit
		}
	}

	type entry struct {
		cursor               events.Cursor
		ledgerCloseTimestamp int64
		event                xdr.ContractEvent
	}
	var found []entry
	err := h.scanner.Scan(
		events.Range{
			Start:      start,
			ClampStart: false,
			End:        events.MaxCursor,
			ClampEnd:   true,
		},
		func(event xdr.ContractEvent, cursor events.Cursor, ledgerCloseTimestamp int64) bool {
			if request.Matches(event) {
				found = append(found, entry{cursor, ledgerCloseTimestamp, event})
			}
			return uint(len(found)) < limit
		},
	)
	if err != nil {
		return nil, &jrpc2.Error{
			Code:    code.InvalidRequest,
			Message: err.Error(),
		}
	}

	var results []EventInfo
	for _, entry := range found {
		info, err := eventInfoForEvent(
			entry.event,
			entry.cursor,
			time.Unix(entry.ledgerCloseTimestamp, 0).UTC().Format(time.RFC3339),
		)
		if err != nil {
			return nil, errors.Wrap(err, "could not parse event")
		}
		results = append(results, info)
	}
	return results, nil
}

func eventInfoForEvent(event xdr.ContractEvent, cursor events.Cursor, ledgerClosedAt string) (EventInfo, error) {
	v0, ok := event.Body.GetV0()
	if !ok {
		return EventInfo{}, errors.New("unknown event version")
	}

	var eventType string
	switch event.Type {
	case xdr.ContractEventTypeSystem:
		eventType = EventTypeSystem
	case xdr.ContractEventTypeContract:
		eventType = EventTypeContract
	default:
		return EventInfo{}, errors.New("unknown event type")
	}

	// base64-xdr encode the topic
	topic := make([]string, 0, 4)
	for _, segment := range v0.Topics {
		seg, err := xdr.MarshalBase64(segment)
		if err != nil {
			return EventInfo{}, err
		}
		topic = append(topic, seg)
	}

	// base64-xdr encode the data
	data, err := xdr.MarshalBase64(v0.Data)
	if err != nil {
		return EventInfo{}, err
	}

	return EventInfo{
		EventType:      eventType,
		Ledger:         int32(cursor.Ledger),
		LedgerClosedAt: ledgerClosedAt,
		ContractID:     hex.EncodeToString((*event.ContractId)[:]),
		ID:             cursor.String(),
		PagingToken:    cursor.String(),
		Topic:          topic,
		Value:          EventInfoValue{XDR: data},
	}, nil
}

// NewGetEventsHandler returns a json rpc handler to fetch and filter events
func NewGetEventsHandler(eventsStore *events.MemoryStore, maxLimit, defaultLimit uint) jrpc2.Handler {
	eventsHandler := eventsRPCHandler{
		scanner:      eventsStore,
		maxLimit:     maxLimit,
		defaultLimit: defaultLimit,
	}
	return handler.New(func(ctx context.Context, request GetEventsRequest) ([]EventInfo, error) {
		response, err := eventsHandler.getEvents(request)
		if err != nil {
			return nil, err
		}
		return response, nil
	})
}
