/**
 * Export tagged call edges (caller -> material sink) for the route table.
 * CSV columns (per docs/initializing-gesserit-v2.md §3.1):
 *   kind, caller, caller_file, caller_line, callee, callee_file, callee_line,
 *   call_file, call_line, sink_role
 *
 * Routes are 1-hop edges; multi-hop discovery is the hypothesis agent's job
 * (per init doc §3.4). The exporter synthesizes a route_id per tagged row.
 *
 * Shape note: caller is bound as `Function` (not `Callable`) so that
 * `getFile()` / `getLocation()` / `getCanonicalPath()` resolve directly.
 * Sinks whose enclosing callable is a closure (no canonical path) are
 * dropped — those are rare and the hypothesis agent recovers them by
 * walking the body of the enclosing Function.
 *
 * @kind table
 * @id stellar-cli/routes-rust
 * @name stellar-cli routes (caller -> material sink)
 */

import rust
import StellarCliSinks

from
  StellarCliSinks::MaterialSink sink,
  Function callerFn,
  string caller_symbol,
  string caller_file_path,
  int caller_line_val,
  string callee_symbol,
  string call_file_path,
  int call_line_val
where
  callerFn = sink.getEnclosingCallable() and
  StellarCliSinks::inProductionSource(callerFn.getFile()) and
  (
    not sink.requiresAttackSurface()
    or
    StellarCliSinks::inAttackSurface(callerFn.getFile())
  ) and
  caller_symbol = callerFn.getCanonicalPath() and
  caller_file_path = callerFn.getFile().getRelativePath() and
  caller_line_val = callerFn.getLocation().getStartLine() and
  callee_symbol = sink.getStaticTarget().getCanonicalPath() and
  call_file_path = sink.getFile().getRelativePath() and
  call_line_val = sink.getLocation().getStartLine()
select sink,
  "call" as kind,
  caller_symbol as caller,
  caller_file_path as caller_file,
  caller_line_val as caller_line,
  callee_symbol as callee,
  // Callee file/line are not extracted (the static target is usually a
  // stdlib/crate symbol outside the source archive). The exporter treats
  // empty file / zero line as "external" and passes through.
  "" as callee_file,
  0 as callee_line,
  call_file_path as call_file,
  call_line_val as call_line,
  sink.getSinkRole() as sink_role
