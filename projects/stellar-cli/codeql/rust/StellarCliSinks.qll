/**
 * Security model for stellar-cli. One predicate per concern; one MaterialSink
 * subclass per operation class.
 *
 * Design rule (docs/codeql-query-pack-improvement.md §1): classify by
 * *operation class*, never by the functions named in a known bug. Every class
 * here reads as a general rule that would hold if no specific stellar-cli bug
 * had ever been written. Functions are *examples* of the class, not the
 * definition of it.
 */

import rust
import codeql.rust.Concepts
import codeql.rust.elements.Call
import codeql.rust.elements.Callable

module StellarCliSinks {
  // ---------------------------------------------------------------------
  // Production source filter — exclude tests, vendored, generated.
  //
  // A floor: a real production file may still be skipped here, but a
  // non-production file must never slip through. Pack metrics gate on this.
  // ---------------------------------------------------------------------
  predicate inProductionSource(File f) {
    not f.getRelativePath().matches("%/tests/%") and
    not f.getRelativePath().matches("tests/%") and
    not f.getRelativePath().matches("%/target/%") and
    not f.getRelativePath().matches("target/%") and
    not f.getRelativePath().matches("%/vendor/%") and
    not f.getRelativePath().matches("%/.cargo/%") and
    // Generated bindings live next to their generators and are not auditable.
    not f.getRelativePath().matches("%/generated/%") and
    // Fuzz harnesses are not production attack surface.
    not f.getRelativePath().matches("%/fuzz/%") and
    not f.getRelativePath().matches("fuzz/%")
  }

  // ---------------------------------------------------------------------
  // Subsystem mapping — must agree with project.toml [subsystem_paths] and
  // the exporter's subsystem_for(). Prefix-based; the longest matching
  // prefix wins via the cascade.
  //
  // The QL side is informational only — the authoritative mapping happens
  // in the exporter (which loads project.toml at export time). This
  // predicate is here so .ql exporter queries can pre-tag rows.
  // ---------------------------------------------------------------------
  string subsystemFor(File f) {
    exists(string p | p = f.getRelativePath() |
      // Workspace library crates
      p.matches("cmd/stellar-cli/%") and result = "cli-binary"
      or
      p.matches("cmd/doc-gen/%") and result = "doc-gen"
      or
      p.matches("cmd/crates/stellar-ledger/%") and result = "ledger-hardware"
      or
      p.matches("cmd/crates/soroban-spec-tools/%") and result = "spec-tools"
      or
      p.matches("cmd/crates/soroban-spec-typescript/%") and result = "spec-typescript"
      or
      p.matches("cmd/crates/soroban-test/%") and result = "test-harness"
      or
      // CLI command areas — check before the cli-library catch-all.
      p.matches("cmd/soroban-cli/src/commands/cache/%") and result = "cache-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/cfg/%") and result = "cfg-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/container/%") and result = "container-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/contract/%") and result = "contract-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/env/%") and result = "env-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/fees/%") and result = "fees-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/keys/%") and result = "keys-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/ledger/%") and result = "ledger-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/message/%") and result = "message-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/network/%") and result = "network-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/plugin/%") and result = "plugin-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/snapshot/%") and result = "snapshot-commands"
      or
      p.matches("cmd/soroban-cli/src/commands/tx/%") and result = "tx-commands"
      or
      // Internal infrastructure modules.
      p.matches("cmd/soroban-cli/src/config/%") and result = "configuration"
      or
      (p.matches("cmd/soroban-cli/src/log/%") or p = "cmd/soroban-cli/src/log.rs")
        and result = "logging"
      or
      p.matches("cmd/soroban-cli/src/signer/%") and result = "signing"
      or
      // cli-library catch-all for the soroban-cli crate root + utils.
      (p.matches("cmd/soroban-cli/src/%") and
       not p.matches("cmd/soroban-cli/src/commands/%") and
       not p.matches("cmd/soroban-cli/src/config/%") and
       not p.matches("cmd/soroban-cli/src/log/%") and
       not p = "cmd/soroban-cli/src/log.rs" and
       not p.matches("cmd/soroban-cli/src/signer/%"))
        and result = "cli-library"
    )
  }

  // ---------------------------------------------------------------------
  // Attack surface (file-based floor). The init doc warns: "static
  // call-graph reachability is unreliable; a file-based floor never drops a
  // real caller." For a CLI tool, the surface is broader than for a daemon
  // — *every* command parses user-controlled argv, so every command crate
  // is in-surface.
  // ---------------------------------------------------------------------
  predicate inAttackSurface(File f) {
    inProductionSource(f) and
    (
      // All command subcommands take untrusted argv directly.
      f.getRelativePath().matches("cmd/soroban-cli/src/commands/%")
      or
      // Configuration parses user-supplied TOML / env vars.
      f.getRelativePath().matches("cmd/soroban-cli/src/config/%")
      or
      // Signing handles attacker-supplied envelopes (e.g. `tx sign`).
      f.getRelativePath().matches("cmd/soroban-cli/src/signer/%")
      or
      // Spec parsing operates on attacker WASM and JSON.
      f.getRelativePath().matches("cmd/crates/soroban-spec-tools/%")
      or
      f.getRelativePath().matches("cmd/crates/soroban-spec-typescript/%")
      or
      // Hardware-wallet I/O handles HID frames that could be attacker-shaped
      // if a malicious device or middleware is on the bus.
      f.getRelativePath().matches("cmd/crates/stellar-ledger/%")
      or
      // The stellar-cli binary entry point sits on argv.
      f.getRelativePath().matches("cmd/stellar-cli/%")
    )
  }

  // ---------------------------------------------------------------------
  // Sink classification. Each class is one *operation class*; membership is
  // the predicate, not a function list. requiresAttackSurface() defaults
  // to true (only material when reachable from in-surface code) — override
  // to false for intrinsically dangerous primitives.
  // ---------------------------------------------------------------------

  abstract class MaterialSink extends Call {
    abstract string getSinkRole();

    string getImpactClass() {
      // Default; the exporter has the authoritative map. Specific subclasses
      // may override.
      result = "unspecified"
    }

    /**
     * If true, this sink is only material when the caller sits in the attack
     * surface (filtered by `inAttackSurface`). If false, the operation is
     * intrinsically dangerous regardless of caller.
     */
    predicate requiresAttackSurface() { any() }
  }

  /**
   * Spawning a subprocess. Intrinsically dangerous — a CLI tool that spawns
   * a subprocess with any attacker-influenced argument crosses a trust
   * boundary. This includes the synchronous std::process::Command and the
   * async tokio::process::Command.
   */
  class ProcessSpawn extends MaterialSink {
    ProcessSpawn() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        path = "<std::process::Command>::new" or
        path = "<std::process::Command>::spawn" or
        path = "<std::process::Command>::output" or
        path = "<std::process::Command>::status" or
        path = "<tokio::process::Command>::new" or
        path = "<tokio::process::Command>::spawn" or
        path = "<tokio::process::Command>::output" or
        path = "<tokio::process::Command>::status"
      )
    }

    override string getSinkRole() { result = "process_spawn" }

    override string getImpactClass() { result = "command_execution" }

    override predicate requiresAttackSurface() { none() }
  }

  /**
   * Shell execution — Command invocations whose program is a known shell, or
   * which pass `-c` (a strong indicator the argv contains shell syntax).
   */
  class ShellExec extends MaterialSink {
    ShellExec() {
      // A Command::new with a shell program. The literal text includes the
      // surrounding quotes, so the comparison list uses pre-quoted strings.
      exists(string path, string prog |
        path = this.getStaticTarget().getCanonicalPath() and
        prog = this.getPositionalArgument(0).(StringLiteralExpr).getTextValue()
      |
        (path = "<std::process::Command>::new" or path = "<tokio::process::Command>::new") and
        prog in [
          "\"sh\"", "\"/bin/sh\"", "\"bash\"", "\"/bin/bash\"", "\"zsh\"",
          "\"powershell\"", "\"cmd\"", "\"cmd.exe\""
        ]
      )
    }

    override string getSinkRole() { result = "shell_exec" }

    override string getImpactClass() { result = "command_execution" }

    override predicate requiresAttackSurface() { none() }
  }

  /**
   * Filesystem writes that take an attacker-supplied path. The classification
   * here is "write of a path expression" — pairing with an upstream taint
   * source happens at dispatch (the route is reachable from the attack
   * surface) and in the hypothesis agent (the actual flow).
   */
  class FilesystemWrite extends MaterialSink {
    FilesystemWrite() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        path = "std::fs::write" or
        path = "std::fs::create_dir" or
        path = "std::fs::create_dir_all" or
        path = "std::fs::remove_file" or
        path = "std::fs::remove_dir" or
        path = "std::fs::remove_dir_all" or
        path = "std::fs::rename" or
        path = "std::fs::copy" or
        path = "std::fs::hard_link" or
        path = "<std::fs::File>::create" or
        path = "<std::fs::File>::create_new" or
        path = "<std::fs::OpenOptions>::open" or
        path = "tokio::fs::write" or
        path = "tokio::fs::create_dir" or
        path = "tokio::fs::create_dir_all" or
        path = "tokio::fs::remove_file" or
        path = "tokio::fs::rename" or
        path = "<tokio::fs::File>::create" or
        path = "<tokio::fs::OpenOptions>::open"
      )
    }

    override string getSinkRole() { result = "filesystem_write_attacker" }

    override string getImpactClass() { result = "path_integrity" }
  }

  /**
   * Filesystem reads that take an attacker-supplied path. Less severe than
   * writes but still material — an attacker-chosen read can leak secrets
   * (read of `~/.stellar/identity/`, `/etc/passwd`, etc.).
   */
  class FilesystemRead extends MaterialSink {
    FilesystemRead() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        path = "std::fs::read" or
        path = "std::fs::read_to_string" or
        path = "std::fs::read_dir" or
        path = "std::fs::metadata" or
        path = "std::fs::canonicalize" or
        path = "<std::fs::File>::open" or
        path = "tokio::fs::read" or
        path = "tokio::fs::read_to_string" or
        path = "tokio::fs::read_dir" or
        path = "<tokio::fs::File>::open"
      )
    }

    override string getSinkRole() { result = "filesystem_read_attacker" }

    override string getImpactClass() { result = "path_integrity" }
  }

  /**
   * Deserialization of attacker-controlled bytes through serde-family
   * parsers. The danger here is malformed inputs reaching code that expects
   * well-formed structures (panics, OOM via large allocations, logic bugs
   * on adversarial structures).
   */
  class DeserializeUntrusted extends MaterialSink {
    DeserializeUntrusted() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        path = "serde_json::de::from_str" or
        path = "serde_json::de::from_slice" or
        path = "serde_json::de::from_reader" or
        path = "serde_json::de::from_value" or
        path = "toml::de::from_str" or
        path = "toml::de::from_slice" or
        path = "toml_edit::de::from_str" or
        path = "serde_yaml::from_str" or
        path = "serde_yaml::from_slice" or
        path = "bincode::deserialize" or
        path = "bincode::deserialize_from" or
        path = "rmp_serde::from_slice" or
        path = "rmp_serde::from_read"
      )
    }

    override string getSinkRole() { result = "deserialize_untrusted" }

    override string getImpactClass() { result = "memory_safety" }
  }

  /**
   * XDR decoding from attacker bytes — the Stellar transaction envelope
   * format. Same risk profile as deserialize_untrusted but distinguished
   * because XDR is the dominant network-input shape across stellar-cli.
   */
  class UntrustedXdrDecode extends MaterialSink {
    UntrustedXdrDecode() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        // stellar_xdr_next is the current generation; stellar_xdr the legacy.
        path.matches("stellar_xdr::%::from_xdr") or
        path.matches("stellar_xdr::%::from_xdr_base64") or
        path.matches("stellar_xdr::%::read_xdr") or
        path.matches("stellar_xdr::%::read_xdr_base64") or
        path.matches("stellar_xdr::%::read_xdr_to_end") or
        path.matches("stellar_xdr::%::from_xdr_bytes") or
        path.matches("stellar_xdr_next::%::from_xdr") or
        path.matches("stellar_xdr_next::%::from_xdr_base64") or
        path.matches("stellar_xdr_next::%::read_xdr")
      )
    }

    override string getSinkRole() { result = "untrusted_xdr_decode" }

    override string getImpactClass() { result = "memory_safety" }
  }

  /**
   * Reading environment variables. Material because (a) some env vars carry
   * secrets stellar-cli reads (mnemonics, secret keys, API tokens) and (b)
   * any env-derived value crossing into a sensitive operation widens the
   * trust boundary. Not gated on attack surface — env access from any code
   * path is interesting.
   */
  class EnvironmentRead extends MaterialSink {
    EnvironmentRead() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        path = "std::env::var" or
        path = "std::env::var_os" or
        path = "std::env::vars" or
        path = "std::env::vars_os"
      )
    }

    override string getSinkRole() { result = "secret_read" }

    override string getImpactClass() { result = "secret_disclosure" }

    override predicate requiresAttackSurface() { none() }
  }

  /**
   * WASM compilation/instantiation of attacker bytes. A user-supplied .wasm
   * passed to `contract deploy` reaches a wasmparser/wasmtime/wasmer
   * compile step; the surface is large and historically high-impact.
   */
  class WasmCompileUntrusted extends MaterialSink {
    WasmCompileUntrusted() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        path = "<wasmparser::Parser>::new" or
        path.matches("wasmparser::%::parse") or
        path = "<wasmtime::Module>::new" or
        path = "<wasmtime::Module>::from_binary" or
        path = "<wasmtime::Module>::from_file" or
        path = "<wasmer::Module>::new" or
        path = "<wasmer::Module>::from_binary" or
        // soroban_env_host module path; broad match against the module facade.
        path.matches("<soroban_env_host::%::Module>::new") or
        path.matches("<soroban_env_host::%::Module>::from_binary")
      )
    }

    override string getSinkRole() { result = "wasm_compile_untrusted" }

    override string getImpactClass() { result = "code_execution" }

    override predicate requiresAttackSurface() { none() }
  }

  /**
   * Outbound HTTP — the URL is typically derived from network config that a
   * compromised config file could redirect (SSRF on the developer's host).
   */
  class NetworkEgressUserUrl extends MaterialSink {
    NetworkEgressUserUrl() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        path = "<reqwest::Client>::get" or
        path = "<reqwest::Client>::post" or
        path = "<reqwest::Client>::put" or
        path = "<reqwest::Client>::patch" or
        path = "<reqwest::Client>::delete" or
        path = "<reqwest::Client>::head" or
        path = "<reqwest::Client>::request" or
        path = "reqwest::get" or
        path = "<reqwest::blocking::Client>::get" or
        path = "<reqwest::blocking::Client>::post" or
        path = "reqwest::blocking::get" or
        path = "<hyper::Client>::request" or
        path = "<hyper::Client>::get"
      )
    }

    override string getSinkRole() { result = "network_egress_user_url" }

    override string getImpactClass() { result = "ssrf" }
  }

  // ---------------------------------------------------------------------
  // Guards / sanitizers. Their presence on a route signals an established
  // defense (the hypothesis agent will weigh this when deciding viability).
  // ---------------------------------------------------------------------

  abstract class GuardSite extends Call {
    abstract string getGuardKind();
  }

  /**
   * Path canonicalization — resolves a path through symlinks and verifies
   * existence, the standard defense against `../` traversal.
   */
  class PathCanonicalize extends GuardSite {
    PathCanonicalize() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        path = "std::fs::canonicalize" or
        path = "<std::path::Path>::canonicalize" or
        path = "tokio::fs::canonicalize" or
        path = "dunce::canonicalize" or
        path = "dunce::simplified" or
        path = "path_clean::clean"
      )
    }

    override string getGuardKind() { result = "path_canonicalization" }
  }

  /**
   * URL parsing / validation through the `url` crate. Parsing is a guard in
   * the sense that an invalid URL causes early rejection.
   */
  class UrlParse extends GuardSite {
    UrlParse() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        path = "<url::Url>::parse" or
        path = "url::Url::parse"
      )
    }

    override string getGuardKind() { result = "url_validation" }
  }

  /**
   * Explicit size-limit checks on a length/byte buffer (a precondition of
   * decode paths). Detected structurally — any comparison or `.min()` that
   * caps a `len()` against a constant.
   */
  class SizeCap extends GuardSite {
    SizeCap() {
      exists(string path | path = this.getStaticTarget().getCanonicalPath() |
        // Common Rust ergonomics for capping a size before allocation/read.
        path = "<core::cmp::Ord>::min" or
        path = "<core::cmp::Ord>::max" or
        path = "std::cmp::min" or
        path = "std::cmp::max"
      )
    }

    override string getGuardKind() { result = "size_cap" }
  }
}
