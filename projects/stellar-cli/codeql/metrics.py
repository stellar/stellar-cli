#!/usr/bin/env python3
"""Static-fact-cache validation harness.

Implements the §6 gates from docs/initializing-gesserit-v2.md:

  1. Routes exist and are bounded (not zero; not 10k+ near-identical).
  2. Trivial-root fraction is low.
  3. On-attack-surface fraction is high.
  4. Determinism (route_id set identical across two same-commit exports).
  5. No test / vendor / generated noise in route source files.

Commands:
  metrics.py summary <cache.sqlite>           — print all gates + breakdown
  metrics.py determinism <a.sqlite> <b.sqlite> — compare route_id sets

Exit code 0 = all gates pass, 1 = at least one fails.

The harness reads the cache via `mode=ro`; it is safe to run against the
authoritative file (no mutation possible).
"""

from __future__ import annotations

import argparse
import sqlite3
import sys
from collections import Counter
from pathlib import Path

# Same trivial-leaf set as gesserit's runtime (`_TRIVIAL_TARGET_LEAVES` in
# src/gesserit/static_analysis.py). Kept here as a copy so the harness is
# self-contained; if the runtime list changes, mirror it.
_TRIVIAL_LEAVES = frozenset(
    {
        "getstate", "gettype", "type", "size", "empty", "begin", "end", "data",
        "threadismain", "shared_from_this", "make_shared", "now", "mark",
        "tostring", "getstring", "c_str", "lock", "unlock", "count", "v0", "v1",
    }
)


# Noise filters — any production file under one of these path patterns
# means the .qll's inProductionSource() predicate is letting noise through.
_NOISE_PATTERNS = (
    "/tests/", "tests/", "/target/", "target/",
    "/vendor/", "/.cargo/", "/generated/", "/fuzz/", "fuzz/",
)


# ---------------------------------------------------------------------------
# Gate thresholds. Calibrated for a CLI-tool-sized pack; if the catalog
# changes shape significantly, retune.
# ---------------------------------------------------------------------------

GATE_MIN_ROUTES = 10
GATE_MAX_ROUTES = 50_000
GATE_MAX_TRIVIAL_FRAC = 0.20  # at most 20% routes rooted in trivial leaves
GATE_MIN_ON_SURFACE_FRAC = 0.70  # at least 70% of routes on-surface


def _connect(path: Path) -> sqlite3.Connection:
    uri = f"file:{path}?mode=ro"
    return sqlite3.connect(uri, uri=True)


def _leaf(symbol: str) -> str:
    return symbol.rsplit("::", 1)[-1] if symbol else symbol


def _on_surface(file: str) -> bool:
    # Matches the .qll's inAttackSurface predicate (broad CLI floor).
    return (
        file.startswith("cmd/soroban-cli/src/commands/")
        or file.startswith("cmd/soroban-cli/src/config/")
        or file.startswith("cmd/soroban-cli/src/signer/")
        or file.startswith("cmd/crates/soroban-spec-tools/")
        or file.startswith("cmd/crates/soroban-spec-typescript/")
        or file.startswith("cmd/crates/stellar-ledger/")
        or file.startswith("cmd/stellar-cli/")
    )


def _is_noise(file: str) -> bool:
    return any(pat in file for pat in _NOISE_PATTERNS)


def cmd_summary(cache: Path) -> int:
    conn = _connect(cache)
    routes = conn.execute(
        "SELECT route_id, subsystem, sink_role, impact_class, root_symbol, root_file FROM static_route"
    ).fetchall()
    sinks = conn.execute(
        "SELECT sink_role, impact_class, file FROM static_sink"
    ).fetchall()
    guards = conn.execute("SELECT guard_kind FROM static_guard").fetchall()
    symbols = conn.execute("SELECT symbol_role FROM static_symbol").fetchall()
    conn.close()

    total = len(routes)
    failures: list[str] = []

    print(f"=== static-codeql.sqlite gate summary ({cache}) ===")
    print(f"routes:    {total}")
    print(f"sinks:     {len(sinks)}")
    print(f"guards:    {len(guards)}")
    print(f"symbols:   {len(symbols)}")
    print()

    # Gate 1: bounded.
    if total < GATE_MIN_ROUTES:
        failures.append(f"GATE 1 FAIL: only {total} routes (need >= {GATE_MIN_ROUTES})")
    elif total > GATE_MAX_ROUTES:
        failures.append(f"GATE 1 FAIL: {total} routes (cap {GATE_MAX_ROUTES})")
    else:
        print(f"GATE 1 PASS: {total} routes within bounds")

    # Gate 2: trivial-root fraction.
    trivial = sum(1 for r in routes if _leaf(r[4]).lower() in _TRIVIAL_LEAVES)
    trivial_frac = trivial / total if total else 0.0
    if trivial_frac > GATE_MAX_TRIVIAL_FRAC:
        failures.append(
            f"GATE 2 FAIL: trivial-root fraction {trivial_frac:.2%} > {GATE_MAX_TRIVIAL_FRAC:.0%}"
        )
    else:
        print(f"GATE 2 PASS: trivial-root fraction {trivial_frac:.2%}")

    # Gate 3: on-surface fraction.
    on_surface = sum(1 for r in routes if _on_surface(r[5]))
    on_surface_frac = on_surface / total if total else 0.0
    if on_surface_frac < GATE_MIN_ON_SURFACE_FRAC:
        failures.append(
            f"GATE 3 FAIL: on-surface fraction {on_surface_frac:.2%} < {GATE_MIN_ON_SURFACE_FRAC:.0%}"
        )
    else:
        print(f"GATE 3 PASS: on-surface fraction {on_surface_frac:.2%}")

    # Gate 5: noise check (test/vendor/generated).
    noisy = [r[5] for r in routes if _is_noise(r[5])]
    if noisy:
        failures.append(
            f"GATE 5 FAIL: {len(noisy)} routes from non-production files; sample: {noisy[:3]}"
        )
    else:
        print("GATE 5 PASS: no non-production routes")

    # Breakdown — not gated, but printed.
    print()
    print("--- by subsystem ---")
    by_sub = Counter(r[1] for r in routes)
    for sub, n in sorted(by_sub.items(), key=lambda x: (-x[1], x[0])):
        print(f"  {sub:25s} {n}")

    print()
    print("--- by sink_role ---")
    by_role = Counter(r[2] for r in routes)
    for role, n in sorted(by_role.items(), key=lambda x: (-x[1], x[0])):
        print(f"  {role:30s} {n}")

    print()
    print("--- by impact_class ---")
    by_impact = Counter(r[3] for r in routes)
    for impact, n in sorted(by_impact.items(), key=lambda x: (-x[1], x[0])):
        print(f"  {impact:25s} {n}")

    print()
    if failures:
        print("FAILED GATES:")
        for f in failures:
            print(f"  {f}")
        return 1
    print("ALL GATES PASS")
    return 0


def cmd_determinism(a: Path, b: Path) -> int:
    conn_a = _connect(a)
    conn_b = _connect(b)
    ids_a = {r[0] for r in conn_a.execute("SELECT route_id FROM static_route")}
    ids_b = {r[0] for r in conn_b.execute("SELECT route_id FROM static_route")}
    conn_a.close()
    conn_b.close()

    only_a = ids_a - ids_b
    only_b = ids_b - ids_a
    common = ids_a & ids_b

    print(f"a routes: {len(ids_a)}")
    print(f"b routes: {len(ids_b)}")
    print(f"shared:   {len(common)}")
    print(f"only_a:   {len(only_a)}")
    print(f"only_b:   {len(only_b)}")

    if only_a or only_b:
        print()
        print("GATE 4 FAIL: route_id sets differ across exports")
        if only_a:
            print(f"  sample only_a: {sorted(only_a)[:3]}")
        if only_b:
            print(f"  sample only_b: {sorted(only_b)[:3]}")
        return 1
    print()
    print("GATE 4 PASS: deterministic across exports")
    return 0


def _main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("summary", help="gate summary + breakdown")
    s.add_argument("cache", type=Path)
    d = sub.add_parser("determinism", help="compare two exports' route_id sets")
    d.add_argument("a", type=Path)
    d.add_argument("b", type=Path)
    args = p.parse_args()

    if args.cmd == "summary":
        return cmd_summary(args.cache)
    if args.cmd == "determinism":
        return cmd_determinism(args.a, args.b)
    return 2


if __name__ == "__main__":
    raise SystemExit(_main())
