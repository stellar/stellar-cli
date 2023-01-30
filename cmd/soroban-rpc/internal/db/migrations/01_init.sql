-- +migrate Up
CREATE TABLE ledger_entries (
    key BLOB NOT NULL PRIMARY KEY,
    entry BLOB NOT NULL
);

-- metadata key-value store
CREATE TABLE metadata (
    key TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);



CREATE INDEX ledger_entries_key ON ledger_entries(key);


-- +migrate Down
drop table ledger_entries cascade;
drop table ledger_entries_meta cascade;
