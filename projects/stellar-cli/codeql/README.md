# stellar-cli CodeQL pack (gesserit v2 substrate)

This directory holds the offline CodeQL substrate the gesserit v2 scanner
dispatches against. The runtime never compiles QL or queries source — it only
reads the SQLite cache produced here.

See `docs/codeql.md` and `docs/initializing-gesserit-v2.md` in the gesserit
repo for the architecture and contracts.

## Layout

```
codeql/
├── README.md                  — this file
├── export_static_facts.py     — CSV → static-codeql.sqlite exporter
├── ffi-bridges.toml           — FFI bridge manifest (empty; pure-Rust target)
├── metrics.py                 — §6 gate harness (summary + determinism)
└── rust/
    ├── qlpack.yml             — pack manifest (declares codeql/rust-all dep)
    ├── StellarCliSinks.qll    — security model (operation classes + guards)
    ├── entrypoints.ql         — CSV: CLI subcommand entry callables
    ├── material_sinks.ql      — CSV: classified sink call-sites
    ├── routes_rust.ql         — CSV: caller → sink call edges
    └── guards_rust.ql         — CSV: guard call-sites
```

## Offline rebuild (one shot)

Pin the CodeQL CLI version in your runbook (this pack was authored against
CodeQL `2.25.6`). Run from this directory's parent (the stellar-cli source
root); `$REPO` is that directory below.

```sh
REPO=$PWD
PACK=$REPO/projects/stellar-cli/codeql/rust
DB=/tmp/stellar-cli-rust-db
AI=$REPO/ai-summary

# 1. Install pack dependencies (one-time / on dep bump).
codeql pack install "$PACK"

# 2. Compile the pack (catches QL errors before the DB build).
codeql query compile "$PACK"/*.ql

# 3. Build the Rust DB OUTSIDE the source root (the init doc §4 rule).
rm -rf "$DB"
codeql database create "$DB" \
  --language=rust \
  --source-root="$REPO" \
  --build-mode=none \
  --overwrite \
  -O rust.extract_dependencies_as_source=false

# 4. Run each query and decode to CSV.
mkdir -p "$AI/codeql-csv"
for q in entrypoints material_sinks routes_rust guards_rust; do
  codeql query run  "$PACK/$q.ql" --database="$DB" --output="$AI/codeql-csv/$q.bqrs"
  codeql bqrs decode --format=csv --output="$AI/codeql-csv/$q.csv" "$AI/codeql-csv/$q.bqrs"
done

# 5. Export the cache.
"$REPO/projects/stellar-cli/codeql/export_static_facts.py" \
  --out "$AI/static-codeql.sqlite" \
  --project-toml "<gesserit>/projects/stellar-cli/project.toml" \
  --source-commit "$(git rev-parse HEAD)" \
  --entrypoints-csv "$AI/codeql-csv/entrypoints.csv" \
  --sinks-csv      "$AI/codeql-csv/material_sinks.csv" \
  --routes-csv     "$AI/codeql-csv/routes_rust.csv" \
  --guards-csv     "$AI/codeql-csv/guards_rust.csv"

# 6. Validate via §6 gates.
"$REPO/projects/stellar-cli/codeql/metrics.py" summary "$AI/static-codeql.sqlite"
```

The exporter takes `--project-toml` so subsystem-glob matching stays in lockstep
with `projects/stellar-cli/project.toml` (in the gesserit repo) without needing
the gesserit checkout to be `$REPO`.

## Determinism check (gate 4)

```sh
"$REPO/projects/stellar-cli/codeql/metrics.py" determinism \
  /path/to/cache-a.sqlite /path/to/cache-b.sqlite
```

Two exports from the same commit must yield identical `route_id` sets.

## Catalog edits — the rules

- Classify by **operation class**, never by named functions from a known bug.
- Generalize, then verify each class member against the tree (start broad,
  enumerate, never copy a function list).
- The exporter owns three project-specific things only: `subsystem_for()`,
  `route_impact_class()` (via `_IMPACT_CLASS`), and the `route_id` hash —
  do not touch the route_id formula without ratcheting the cache version.
- Re-run gates after every `.qll` change; a rising trivial-root or noise
  fraction is a precision regression.
- See `docs/codeql-query-pack-improvement.md` for the full propose →
  measure → gate → human-merge loop.

## Activating in gesserit

Once the cache is built and gates pass, flip these in
`projects/stellar-cli/objectives/security-scan/objective.toml`:

```toml
[memory_dispatch]
enabled = true

[static_analysis]
enabled = true

[negative_memory]
enabled = true
```

Run `ProjectObjectiveConfig.load(..., validate=True)` to confirm the config
still loads with the v2 path resolved against `--repo-root` ($REPO).
