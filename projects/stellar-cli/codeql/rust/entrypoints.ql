/**
 * Export entrypoints (the attack-surface entry callables) for the static
 * fact cache. Output CSV columns (per docs/initializing-gesserit-v2.md §3.1):
 *   kind, subsystem, symbol, file, line, trust_boundary, input_shape
 *
 * @kind table
 * @id stellar-cli/entrypoints
 * @name stellar-cli entrypoints
 */

import rust
import StellarCliSinks

private predicate isCommandHandler(Function f) {
  // CLI subcommand handlers conventionally expose `run` or `cmd`. The
  // floor here is broad on purpose: clap-derived handlers may use either,
  // and any false positives surface as low-materiality entrypoints (the
  // route metric drops them; the exporter does not).
  (f.getName().getText() = "run" or f.getName().getText() = "cmd") and
  StellarCliSinks::inAttackSurface(f.getFile())
}

private predicate isBinaryMain(Function f) {
  f.getName().getText() = "main" and
  f.getFile().getRelativePath().matches("cmd/stellar-cli/%")
}

from Function f, string symbol, string file, int line, string subsystem
where
  (isCommandHandler(f) or isBinaryMain(f)) and
  symbol = f.getCanonicalPath() and
  file = f.getFile().getRelativePath() and
  line = f.getLocation().getStartLine() and
  subsystem = StellarCliSinks::subsystemFor(f.getFile())
select f,
  "entrypoint" as kind,
  subsystem,
  symbol,
  file,
  line,
  "user_argv" as trust_boundary,
  "cli_argv" as input_shape
