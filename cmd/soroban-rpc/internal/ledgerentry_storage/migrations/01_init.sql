-- +migrate Up
CREATE TABLE ledger_entries (
    key bigint NOT NULL PRIMARY KEY,
    entry TEXT NOT NULL
);

-- metadata about the content in the ledger_entries table
CREATE TABLE ledger_entries_meta (
    key TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);



CREATE INDEX ledger_entries_key ON ledger_entries(key);


-- +migrate Down
drop table ledger_entries cascade;
drop table ledger_entries_meta cascade;
