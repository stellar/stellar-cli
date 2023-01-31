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

-- table to store all ledgers
CREATE TABLE ledger_close_meta (
    sequence INTEGER NOT NULL PRIMARY KEY,
    meta BLOB NOT NULL
);

-- +migrate Down
drop table ledger_entries cascade;
drop table ledger_entries_meta cascade;
drop table ledger_close_meta cascade;
