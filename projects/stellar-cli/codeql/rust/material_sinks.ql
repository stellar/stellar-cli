/**
 * Export every MaterialSink instance, classified by operation class. CSV
 * columns (per docs/initializing-gesserit-v2.md §3.1):
 *   kind, subsystem, symbol, file, line, sink_role, impact_class
 *
 * The .qll's `requiresAttackSurface()` predicate is honored here: surface-
 * gated sinks are only emitted when their *enclosing callable* sits in the
 * attack surface, so a ubiquitous primitive (e.g. `fs::write`) does not
 * flood the catalog from non-surface code.
 *
 * @kind table
 * @id stellar-cli/material-sinks
 * @name stellar-cli material sinks
 */

import rust
import StellarCliSinks

private predicate sinkApplicable(StellarCliSinks::MaterialSink s) {
  StellarCliSinks::inProductionSource(s.getFile()) and
  (
    not s.requiresAttackSurface()
    or
    exists(Callable c | c = s.getEnclosingCallable() and StellarCliSinks::inAttackSurface(c.getFile()))
  )
}

from StellarCliSinks::MaterialSink sink, string symbol, string file, int line, string subsystem
where
  sinkApplicable(sink) and
  symbol = sink.getStaticTarget().getCanonicalPath() and
  file = sink.getFile().getRelativePath() and
  line = sink.getLocation().getStartLine() and
  subsystem = StellarCliSinks::subsystemFor(sink.getFile())
select sink,
  "sink" as kind,
  subsystem,
  symbol,
  file,
  line,
  sink.getSinkRole() as sink_role,
  sink.getImpactClass() as impact_class
