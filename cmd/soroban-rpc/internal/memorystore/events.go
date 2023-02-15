package memorystore

import (
	"encoding/json"
	"fmt"
	"io"
	"math"
	"sort"
	"strconv"
	"strings"

	"github.com/stellar/go/ingest"
	"github.com/stellar/go/toid"
	"github.com/stellar/go/xdr"
)

var (
	// MinEventCursor is the smallest possible cursor
	MinEventCursor = EventCursor{}
	// MaxEventCursor is the largest possible cursor
	MaxEventCursor = EventCursor{
		Ledger: math.MaxUint32,
		Tx:     math.MaxUint32,
		Op:     math.MaxUint32,
		Event:  math.MaxUint32,
	}
)

// EventCursor represents the position of a Soroban event.
// Soroban events are sorted in ascending order by
// ledger sequence, transaction index, operation index,
// and event index.
type EventCursor struct {
	// Ledger is the sequence of the ledger which emitted the event.
	Ledger uint32
	// Tx is the index of the transaction within the ledger which emitted the event.
	Tx uint32
	// Op is the index of the operation within the transaction which emitted the event.
	Op uint32
	// Event is the index of the event within in the operation which emitted the event.
	Event uint32
}

// String returns a string representation of this cursor
func (c EventCursor) String() string {
	return fmt.Sprintf(
		"%019d-%010d",
		toid.New(int32(c.Ledger), int32(c.Tx), int32(c.Op)).ToInt64(),
		c.Event,
	)
}

// MarshalJSON marshals the cursor into JSON
func (c EventCursor) MarshalJSON() ([]byte, error) {
	return json.Marshal(c.String())
}

// UnmarshalJSON unmarshalls a cursor from the given JSON
func (c *EventCursor) UnmarshalJSON(b []byte) error {
	var s string
	if err := json.Unmarshal(b, &s); err != nil {
		return err
	}

	if parsed, err := ParseCursor(s); err != nil {
		return err
	} else {
		*c = parsed
	}
	return nil
}

// ParseCursor parses the given string and returns the corresponding cursor
func ParseCursor(input string) (EventCursor, error) {
	parts := strings.SplitN(input, "-", 2)
	if len(parts) != 2 {
		return EventCursor{}, fmt.Errorf("invalid event id %s", input)
	}

	// Parse the first part (toid)
	idInt, err := strconv.ParseInt(parts[0], 10, 64) //lint:ignore gomnd
	if err != nil {
		return EventCursor{}, fmt.Errorf("invalid event id %s: %w", input, err)
	}
	parsed := toid.Parse(idInt)

	// Parse the second part (event order)
	eventOrder, err := strconv.ParseInt(parts[1], 10, 64) //lint:ignore gomnd
	if err != nil {
		return EventCursor{}, fmt.Errorf("invalid event id %s: %w", input, err)
	}

	return EventCursor{
		Ledger: uint32(parsed.LedgerSequence),
		Tx:     uint32(parsed.TransactionOrder),
		Op:     uint32(parsed.OperationOrder),
		Event:  uint32(eventOrder),
	}, nil
}

func cmp(a, b uint32) int {
	if a < b {
		return -1
	}
	if a > b {
		return 1
	}
	return 0
}

// Cmp compares two cursors.
// 0 is returned if the c is equal to other.
// 1 is returned if c is greater than other.
// -1 is returned if c is less than other.
func (c EventCursor) Cmp(other EventCursor) int {
	if c.Ledger == other.Ledger {
		if c.Tx == other.Tx {
			if c.Op == other.Op {
				return cmp(c.Event, other.Event)
			}
			return cmp(c.Op, other.Op)
		}
		return cmp(c.Tx, other.Tx)
	}
	return cmp(c.Ledger, other.Ledger)
}

type event struct {
	contents   xdr.ContractEvent
	txIndex    uint32
	opIndex    uint32
	eventIndex uint32
}

func (e event) cursor(ledgerSeq uint32) EventCursor {
	return EventCursor{
		Ledger: ledgerSeq,
		Tx:     e.txIndex,
		Op:     e.opIndex,
		Event:  e.eventIndex,
	}
}

func readEvents(networkPassphrase string, ledgerCloseMeta xdr.LedgerCloseMeta) (events []event, err error) {
	var txReader *ingest.LedgerTransactionReader
	txReader, err = ingest.NewLedgerTransactionReaderFromLedgerCloseMeta(networkPassphrase, ledgerCloseMeta)
	if err != nil {
		return
	}
	defer func() {
		closeErr := txReader.Close()
		if err == nil {
			err = closeErr
		}
	}()

	for {
		var tx ingest.LedgerTransaction
		tx, err = txReader.Read()
		if err == io.EOF {
			err = nil
			break
		}
		if err != nil {
			return
		}

		if !tx.Result.Successful() {
			continue
		}
		for i := range tx.Envelope.Operations() {
			opIndex := uint32(i)
			var opEvents []xdr.ContractEvent
			opEvents, err = tx.GetOperationEvents(opIndex)
			if err != nil {
				return
			}
			for eventIndex, opEvent := range opEvents {
				events = append(events, event{
					contents:   opEvent,
					txIndex:    tx.Index,
					opIndex:    opIndex,
					eventIndex: uint32(eventIndex),
				})
			}
		}
	}
	return events, err
}

// seekEvents returns the subset of all events which occur
// at a point greater than or equal to the given cursor.
// events must be sorted in ascending order.
func seekEvents(events []event, cursor EventCursor) []event {
	j := sort.Search(len(events), func(i int) bool {
		return cursor.Cmp(events[i].cursor(cursor.Ledger)) <= 0
	})
	return events[j:]
}
