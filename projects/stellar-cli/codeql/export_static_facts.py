#!/usr/bin/env python3
"""Fold decoded CodeQL CSV outputs into the read-only static-fact SQLite cache.

Schema and column shapes are dictated by gesserit's runtime consumer:
``src/gesserit/static_analysis.py`` (``CodeQLStore._route_from_row``). This
exporter is the only place the cache is written; the orchestrator opens it
``mode=ro``.

route_id stability contract (anchors only, per
``docs/initializing-gesserit-v2.md`` §3.1):
  route_kind, subsystem, root_symbol, sink_symbol, sink_role, typed-scope.
NOT included: source_commit, intermediate path symbols. A commit bump alone
must never churn route_ids, otherwise negative-memory investigation locks
would lose their anchors.
"""

from __future__ import annotations

import argparse
import csv
import fnmatch
import hashlib
import json
import sqlite3
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

# --- subsystem mapping -------------------------------------------------------
#
# Source of truth is ``projects/stellar-cli/project.toml`` (in gesserit). The
# exporter loads ``[subsystem_paths]`` and matches each route's caller file
# against the fnmatch globs. The same vocabulary is used in the .qll's
# ``subsystemFor`` predicate; keep the two in lockstep.
#
# When a file matches no glob the route gets ``subsystem = "_unmapped"`` so it
# is visible in metrics but never dispatched (the runtime only queries
# subsystems explicitly listed in objective.toml ``[targets]``).

_UNMAPPED = "_unmapped"


def _load_subsystem_paths(project_toml: Path) -> dict[str, list[str]]:
    with project_toml.open("rb") as f:
        data = tomllib.load(f)
    return data.get("subsystem_paths", {})


def _glob_match(path: str, pattern: str) -> bool:
    """Gitignore-style glob with `/`-aware segments.

    - `**` matches zero or more path segments.
    - `*` matches a single segment (no `/`).
    - Other fnmatch wildcards (`?`, `[abc]`) apply within a segment.

    Python's stdlib has no built-in for this until 3.13's `full_match`;
    we walk segments by hand to avoid a pin on a recent interpreter.
    """
    path_parts = path.split("/")
    pat_parts = pattern.split("/")

    def walk(pi: int, ti: int) -> bool:
        if pi == len(pat_parts):
            return ti == len(path_parts)
        seg = pat_parts[pi]
        if seg == "**":
            # `**` consumes zero or more path segments.
            if pi == len(pat_parts) - 1:
                return True  # trailing `**` matches everything left.
            for k in range(ti, len(path_parts) + 1):
                if walk(pi + 1, k):
                    return True
            return False
        if ti == len(path_parts):
            return False
        if fnmatch.fnmatchcase(path_parts[ti], seg):
            return walk(pi + 1, ti + 1)
        return False

    return walk(0, 0)


def _subsystem_for(file_path: str, paths: dict[str, list[str]]) -> str:
    for name, globs in paths.items():
        for pattern in globs:
            if _glob_match(file_path, pattern):
                return name
    return _UNMAPPED


# --- impact class catalog ----------------------------------------------------
#
# Maps sink_role (operation class) -> impact_class (consequence family). The
# .qll picks the sink_role; this map decides what dispatch-side framing the
# agent gets. Roles missing from this map land in "unspecified" — caught by
# metrics.py (a sign the .qll and exporter have drifted).

_IMPACT_CLASS: dict[str, str] = {
    # Process / code execution surfaces
    "process_spawn": "command_execution",
    "shell_exec": "command_execution",
    "plugin_dispatch": "command_execution",
    "container_exec": "command_execution",
    "wasm_compile_untrusted": "code_execution",
    # Deserialization / parsing of attacker bytes
    "deserialize_untrusted": "memory_safety",
    "untrusted_xdr_decode": "memory_safety",
    # Filesystem
    "filesystem_write_attacker": "path_integrity",
    "filesystem_read_attacker": "path_integrity",
    "path_traversal_risk": "path_integrity",
    # Secrets / keys
    "secret_read": "secret_disclosure",
    "key_material_export": "secret_disclosure",
    "log_secret_risk": "secret_disclosure",
    # Network
    "network_egress_user_url": "ssrf",
    "tls_validation_bypass": "transport_integrity",
    # Cryptography
    "crypto_weak_primitive": "crypto_integrity",
    "signature_verification": "crypto_integrity",
}


def _impact_class_for(sink_role: str) -> str:
    return _IMPACT_CLASS.get(sink_role, "unspecified")


# --- CSV ingestion -----------------------------------------------------------


@dataclass
class _Symbol:
    kind: str
    subsystem: str
    symbol: str
    file: str
    line: int
    trust_boundary: str = ""
    input_shape: str = ""


@dataclass
class _Sink:
    kind: str
    subsystem: str
    symbol: str
    file: str
    line: int
    sink_role: str
    impact_class: str


@dataclass
class _Edge:
    kind: str
    caller: str
    caller_file: str
    caller_line: int
    callee: str
    callee_file: str
    callee_line: int
    call_file: str
    call_line: int
    sink_role: str


@dataclass
class _Guard:
    guard_kind: str
    caller: str
    guard_symbol: str
    file: str
    line: int


@dataclass
class _Buf:
    symbols: list[_Symbol] = field(default_factory=list)
    sinks: list[_Sink] = field(default_factory=list)
    edges: list[_Edge] = field(default_factory=list)
    guards: list[_Guard] = field(default_factory=list)


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="") as f:
        return list(csv.DictReader(f))


def _ingest(args: argparse.Namespace, paths: dict[str, list[str]]) -> _Buf:
    buf = _Buf()

    for csv_path in args.entrypoints_csv:
        for row in _read_csv(Path(csv_path)):
            buf.symbols.append(
                _Symbol(
                    kind=row.get("kind", "entrypoint"),
                    subsystem=row.get("subsystem") or _subsystem_for(row["file"], paths),
                    symbol=row["symbol"],
                    file=row["file"],
                    line=int(row["line"]),
                    trust_boundary=row.get("trust_boundary", ""),
                    input_shape=row.get("input_shape", ""),
                )
            )

    for csv_path in args.sinks_csv:
        for row in _read_csv(Path(csv_path)):
            buf.sinks.append(
                _Sink(
                    kind=row.get("kind", "sink"),
                    subsystem=row.get("subsystem") or _subsystem_for(row["file"], paths),
                    symbol=row["symbol"],
                    file=row["file"],
                    line=int(row["line"]),
                    sink_role=row["sink_role"],
                    impact_class=row.get("impact_class") or _impact_class_for(row["sink_role"]),
                )
            )

    for csv_path in args.routes_csv:
        for row in _read_csv(Path(csv_path)):
            buf.edges.append(
                _Edge(
                    kind=row.get("kind", "call"),
                    caller=row["caller"],
                    caller_file=row["caller_file"],
                    caller_line=int(row["caller_line"]),
                    callee=row["callee"],
                    callee_file=row["callee_file"],
                    callee_line=int(row["callee_line"]),
                    call_file=row["call_file"],
                    call_line=int(row["call_line"]),
                    sink_role=row.get("sink_role", ""),
                )
            )

    for csv_path in args.guards_csv:
        for row in _read_csv(Path(csv_path)):
            buf.guards.append(
                _Guard(
                    guard_kind=row["guard_kind"],
                    caller=row["caller"],
                    guard_symbol=row["guard_symbol"],
                    file=row["file"],
                    line=int(row["line"]),
                )
            )

    return buf


# --- route_id hashing --------------------------------------------------------


def _route_id(
    route_kind: str,
    subsystem: str,
    root_symbol: str,
    sink_symbol: str,
    sink_role: str,
    scope_json: str,
) -> str:
    # Stable-anchor inputs only; intermediate path and commit are excluded so
    # a commit bump never churns identities.
    payload = "|".join([route_kind, subsystem, root_symbol, sink_symbol, sink_role, scope_json])
    digest = hashlib.sha256(payload.encode("utf-8")).hexdigest()[:16]
    return f"codeql:{digest}"


# --- SQLite schema -----------------------------------------------------------
#
# Column names track ``CodeQLStore._route_from_row`` in
# ``src/gesserit/static_analysis.py``. Adding columns is safe; renaming or
# removing breaks the consumer. ``static_symbol`` / ``static_sink`` /
# ``static_call_edge`` / ``static_bridge_endpoint`` are intermediate tables
# used by metrics + future tooling — the runtime only reads ``static_route``
# and ``static_guard``.

_SCHEMA = [
    """
    CREATE TABLE static_symbol (
        symbol_role TEXT NOT NULL,
        subsystem TEXT NOT NULL,
        symbol TEXT NOT NULL,
        file TEXT NOT NULL,
        line INTEGER NOT NULL,
        tags_json TEXT NOT NULL DEFAULT '[]'
    )
    """,
    """
    CREATE TABLE static_sink (
        sink_symbol TEXT NOT NULL,
        sink_role TEXT NOT NULL,
        impact_class TEXT NOT NULL,
        subsystem TEXT NOT NULL,
        file TEXT NOT NULL,
        line INTEGER NOT NULL
    )
    """,
    """
    CREATE TABLE static_call_edge (
        caller_symbol TEXT NOT NULL,
        callee_symbol TEXT NOT NULL,
        sink_role TEXT NOT NULL DEFAULT '',
        caller_file TEXT NOT NULL,
        caller_line INTEGER NOT NULL,
        callee_file TEXT NOT NULL,
        callee_line INTEGER NOT NULL,
        call_file TEXT NOT NULL,
        call_line INTEGER NOT NULL
    )
    """,
    """
    CREATE TABLE static_guard (
        guard_kind TEXT NOT NULL,
        caller_symbol TEXT NOT NULL,
        guard_symbol TEXT NOT NULL,
        file TEXT NOT NULL,
        line INTEGER NOT NULL
    )
    """,
    """
    CREATE TABLE static_bridge_endpoint (
        bridge_name TEXT NOT NULL,
        source_symbol TEXT NOT NULL,
        target_symbol TEXT NOT NULL,
        language TEXT NOT NULL
    )
    """,
    """
    CREATE TABLE static_route (
        route_id TEXT PRIMARY KEY,
        language TEXT NOT NULL,
        route_kind TEXT NOT NULL,
        subsystem TEXT NOT NULL,
        root_symbol TEXT NOT NULL,
        root_file TEXT NOT NULL,
        path_symbols_json TEXT NOT NULL,
        path_locations_json TEXT NOT NULL,
        sink_symbol TEXT NOT NULL,
        sink_role TEXT NOT NULL,
        impact_class TEXT NOT NULL,
        route_family TEXT NOT NULL,
        source_fingerprint TEXT NOT NULL DEFAULT '',
        language_segments_json TEXT NOT NULL DEFAULT '[]',
        bridge_edges_json TEXT NOT NULL DEFAULT '[]'
    )
    """,
    "CREATE INDEX idx_route_subsystem ON static_route(subsystem)",
    "CREATE INDEX idx_route_sink_role ON static_route(sink_role)",
]


def _init_db(out: Path) -> sqlite3.Connection:
    if out.exists():
        out.unlink()
    out.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(out)
    for stmt in _SCHEMA:
        conn.execute(stmt)
    return conn


# --- Route synthesis ---------------------------------------------------------
#
# Each tagged edge (sink_role != "") becomes one 1-hop route. ``route_family``
# echoes ``sink_role`` (kept distinct so the schema can evolve; the consumer
# treats them as parallel). The route's ``path_symbols`` is the caller→callee
# 2-element sequence; the .qll could in principle emit longer paths but
# multi-hop discovery is the hypothesis agent's job (per
# docs/initializing-gesserit-v2.md §3.4).


def _synthesize_routes(buf: _Buf, paths: dict[str, list[str]]) -> list[tuple]:
    rows: list[tuple] = []
    seen: set[str] = set()
    for edge in buf.edges:
        if not edge.sink_role:
            continue
        subsystem = (
            edge.caller_file
            and _subsystem_for(edge.caller_file, paths)
            or _UNMAPPED
        )
        scope_json = "{}"  # empty by design; scope is derived from roots at dispatch.
        rid = _route_id(
            route_kind=edge.kind,
            subsystem=subsystem,
            root_symbol=edge.caller,
            sink_symbol=edge.callee,
            sink_role=edge.sink_role,
            scope_json=scope_json,
        )
        if rid in seen:
            continue
        seen.add(rid)
        rows.append(
            (
                rid,
                "rust",
                edge.kind,
                subsystem,
                edge.caller,
                edge.caller_file,
                json.dumps([edge.caller, edge.callee]),
                json.dumps([
                    f"{edge.caller_file}:{edge.caller_line}",
                    f"{edge.callee_file}:{edge.callee_line}",
                ]),
                edge.callee,
                edge.sink_role,
                _impact_class_for(edge.sink_role),
                edge.sink_role,
                "",
                json.dumps(["rust"]),
                json.dumps([]),
            )
        )
    return rows


# --- write to DB -------------------------------------------------------------


def _write_db(conn: sqlite3.Connection, buf: _Buf, paths: dict[str, list[str]]) -> dict:
    conn.executemany(
        "INSERT INTO static_symbol VALUES (?,?,?,?,?,?)",
        [(s.kind, s.subsystem, s.symbol, s.file, s.line, "[]") for s in buf.symbols],
    )
    conn.executemany(
        "INSERT INTO static_sink VALUES (?,?,?,?,?,?)",
        [(s.symbol, s.sink_role, s.impact_class, s.subsystem, s.file, s.line) for s in buf.sinks],
    )
    conn.executemany(
        "INSERT INTO static_call_edge VALUES (?,?,?,?,?,?,?,?,?)",
        [
            (
                e.caller,
                e.callee,
                e.sink_role,
                e.caller_file,
                e.caller_line,
                e.callee_file,
                e.callee_line,
                e.call_file,
                e.call_line,
            )
            for e in buf.edges
        ],
    )
    conn.executemany(
        "INSERT INTO static_guard VALUES (?,?,?,?,?)",
        [(g.guard_kind, g.caller, g.guard_symbol, g.file, g.line) for g in buf.guards],
    )

    route_rows = _synthesize_routes(buf, paths)
    conn.executemany(
        """
        INSERT INTO static_route (
            route_id, language, route_kind, subsystem, root_symbol, root_file,
            path_symbols_json, path_locations_json, sink_symbol, sink_role,
            impact_class, route_family, source_fingerprint,
            language_segments_json, bridge_edges_json
        ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
        """,
        route_rows,
    )
    conn.commit()

    return {
        "static_symbol": len(buf.symbols),
        "static_sink": len(buf.sinks),
        "static_call_edge": len(buf.edges),
        "static_guard": len(buf.guards),
        "static_route": len(route_rows),
    }


# --- CLI ---------------------------------------------------------------------


def _main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", required=True, type=Path, help="output SQLite cache path")
    p.add_argument("--project-toml", required=True, type=Path, help="path to gesserit project.toml")
    p.add_argument("--source-commit", default="", help="git rev to stamp into source_fingerprint")
    p.add_argument("--entrypoints-csv", action="append", default=[])
    p.add_argument("--sinks-csv", action="append", default=[])
    p.add_argument("--routes-csv", action="append", default=[])
    p.add_argument("--guards-csv", action="append", default=[])
    args = p.parse_args()

    paths = _load_subsystem_paths(args.project_toml)
    buf = _ingest(args, paths)
    conn = _init_db(args.out)
    try:
        counts = _write_db(conn, buf, paths)
        if args.source_commit:
            conn.execute(
                "UPDATE static_route SET source_fingerprint = ?",
                (args.source_commit,),
            )
            conn.commit()
    finally:
        conn.close()

    for table, n in counts.items():
        print(f"{table}: {n}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
