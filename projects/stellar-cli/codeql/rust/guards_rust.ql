/**
 * Export guard sites (canonicalization, URL parse, size caps, ...) and
 * which callable hosts each. CSV columns (per
 * docs/initializing-gesserit-v2.md §3.1):
 *   guard_kind, caller, guard_symbol, file, line
 *
 * The orchestrator joins these against routes by caller symbol; a guard in
 * the same callable as a sink call is taken as evidence of defense (used
 * for the on-route guard count in dispatch metadata).
 *
 * @kind table
 * @id stellar-cli/guards-rust
 * @name stellar-cli guard sites
 */

import rust
import StellarCliSinks

from
  StellarCliSinks::GuardSite guard,
  Function callerFn,
  string caller_symbol,
  string guard_symbol_str,
  string file_path,
  int line_val
where
  callerFn = guard.getEnclosingCallable() and
  StellarCliSinks::inProductionSource(callerFn.getFile()) and
  caller_symbol = callerFn.getCanonicalPath() and
  guard_symbol_str = guard.getStaticTarget().getCanonicalPath() and
  file_path = guard.getFile().getRelativePath() and
  line_val = guard.getLocation().getStartLine()
select guard,
  guard.getGuardKind() as guard_kind,
  caller_symbol as caller,
  guard_symbol_str as guard_symbol,
  file_path as file,
  line_val as line
