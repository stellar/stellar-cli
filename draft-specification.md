## Preamble

```
SEP: 0000
Title: Soroban Contract Build Reproducibility and Verification
Author: Leigh McCulloch <@leighmcculloch>
Status: Draft
Created: 2026-05-01
Updated: 2026-05-01
Version: 0.1.0
Discussion: TBD
```

## Simple Summary

Standardize the metadata embedded in Soroban contract wasm artifacts so that any third party can reproducibly rebuild a contract from source and verify the on-chain artifact byte-for-byte against the claimed source.

This SEP describes a *rebuild-based* verification path. It is complementary to [SEP-55](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0055.md), which describes an *attestation-based* path (relying on signed evidence from a trusted CI environment instead of an independent rebuild). The two approaches address the same trust question with different operational and trust trade-offs; a single wasm can carry meta supporting either, both, or neither.

## Dependencies

- [SEP-46](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0046.md) — Contract Meta. Defines the `contractmetav0` Wasm custom section that this SEP populates with reproducibility entries.
- [SEP-55](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0055.md) — Soroban Smart Contracts Build Verification (informative). Defines an attestation-based verification mechanism. SEP-55 also defines a `source_repo` meta key.

## Motivation

Today, when a contract is deployed to mainnet, the on-chain artifact is opaque bytes. A user wishing to evaluate the contract has no programmatic way to confirm that those bytes were built from a particular source tree. Anyone *can* rebuild a contract and compare hashes, but the build environment — host OS, container image, rust toolchain version, cargo features, profile, manifest path — is not recorded anywhere on-chain or in the wasm itself. Without a standard set of inputs to the rebuild, two well-intentioned verifiers can produce different bytes from the same source and reach different conclusions.

This SEP closes that gap by:

1. Defining a stable set of meta entries that build tooling embeds in every reproducible build — enough information that a verifier can stand up a matching build environment.
2. Defining a deterministic verification algorithm — rebuild from the recorded environment, sha256, compare.

[SEP-55](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0055.md) addresses the same trust question via a different mechanism: the build CI signs an attestation, and verifiers check the attestation rather than rebuilding. Rebuild-based verification (this SEP) requires no trust in a particular CI but demands a deterministic build environment; attestation-based verification (SEP-55) accepts trust in the attesting CI but demands no rebuild. The two are not mutually exclusive — a builder can publish meta supporting both, and a verifier can pick whichever path best fits their threat model.

## Abstract

A reproducible Soroban contract build embeds a set of meta entries in the wasm's `contractmetav0` custom section — the toolchain version (`rsver`), the build cli's identity (`cliver`), the container image used (`bldimg`), the source repository and commit (`source_repo`, `source_rev`), and the per-package build options (`bldopt_*`). A verifier reads those entries, reconstructs the build environment, rebuilds, and compares the resulting wasm's sha256 to the original. This SEP defines the entries and the algorithm; it does not prescribe specific tooling.

## Specification

### 1. Build meta entries

A reproducible build embeds the following entries in the wasm's `contractmetav0` custom section. Each entry is a `ScMetaEntry::ScMetaV0` with a UTF-8 `key` and UTF-8 `val`. Values that fail their format regex below are not considered conformant.

| key | description | format regex |
|---|---|---|
| `cliver` | Build cli version + git rev. | `^\d+\.\d+\.\d+(-[A-Za-z0-9.+-]+)?#([0-9a-f]{40}(-dirty)?)?$` |
| `rsver` | Resolved rustc version used for the build. | `^\d+\.\d+\.\d+(-[A-Za-z0-9.+-]+)?$` |
| `bldimg` | Fully-qualified container image used for the build, pinned by digest. Required for builds claiming `docker`-class reproducibility; absent for host-local builds. | `^[^@\s]+@sha256:[0-9a-f]{64}$` |
| `source_repo` | HTTPS URL of the source repository's origin. Recorded only when the working tree was clean at build time. | `^https?://\S+$` |
| `source_rev` | Full 40-char SHA-1 of the source commit (`HEAD`). Recorded only when the working tree was clean at build time. | `^[0-9a-f]{40}$` |
| `bldopt_manifest_path` | Path to the package's `Cargo.toml` relative to the repository root. | `^([^/\s]+/)*Cargo\.toml$` |
| `bldopt_package` | Cargo package name being built. | `^[A-Za-z][A-Za-z0-9_-]*$` |
| `bldopt_profile` | Cargo profile (e.g. `release`). | `^[A-Za-z][A-Za-z0-9_-]*$` |
| `bldopt_optimize` | Present and equal to `true` iff post-build wasm optimization (`wasm-opt`) was applied. | `^true$` |

Tooling may inject additional, application-specific entries; verifiers ignore unrecognized keys.

### 2. Build classes

A wasm's reproducibility class is determined by the meta entries present:

- **Class A — container-pinned**: all of `cliver`, `rsver`, `bldimg`, `source_repo`, `source_rev`, `bldopt_manifest_path`, `bldopt_package`, `bldopt_profile` are present and conformant. The build is reproducible to the bytes of `bldimg`'s pulled image content; verification produces identical bytes on any host with a working docker daemon and access to the registry.
- **Class B — host best-effort**: all of `cliver`, `rsver`, `source_repo`, `source_rev`, `bldopt_manifest_path`, `bldopt_package`, `bldopt_profile` are present and conformant; `bldimg` is absent. The build was performed on the host with a non-containerized toolchain. Verification is best-effort and may produce different bytes on different hosts due to environment differences not captured in meta.
- **Class C — non-reproducible**: any required entry above is absent. Verification cannot be claimed.

Tooling deploying to mainnet should warn when a wasm is Class B or C.

### 3. Verification algorithm

Given an on-chain or offline wasm `W`, a verifier produces a verification result as follows:

1. Compute `sha256(W)` → `original_hash`.
2. Parse `W`'s `contractmetav0` section. Extract the entries listed in §1.
3. Determine the build class per §2. If Class C, verification fails: the wasm carries insufficient meta to be reproduced.
4. Source acquisition is the verifier's responsibility. The verifier ensures a checkout of `source_repo` at commit `source_rev` is available before invoking the rebuild; this SEP does not mandate a clone strategy.
5. Reconstruct the build environment:
   - For Class A: pull `bldimg` (digest-pinned, so deterministic) and run the rebuild inside that container with the source checkout bind-mounted in. The rust toolchain inside the container MUST be the version recorded in `rsver` (e.g. via the `RUSTUP_TOOLCHAIN` environment variable when rustup is the in-image toolchain manager).
   - For Class B: use the host rust toolchain pinned to `rsver` (e.g. via `cargo +<rsver>` when rustup is the host toolchain manager).
6. In the reconstructed environment, perform a cargo build of the recorded package. The build MUST:
   - target `wasm32v1-none`,
   - use `--locked` (so `Cargo.lock` is honored verbatim),
   - use the package's `Cargo.toml` at `bldopt_manifest_path`,
   - target package `bldopt_package`,
   - use cargo profile `bldopt_profile`,
   - apply `wasm-opt` post-build optimization iff `bldopt_optimize` is present and equal to `true`,
   - and otherwise use cargo defaults (no extra features, default dependency resolution, etc.).
7. Locate the rebuilt wasm artifact for `bldopt_package` and compute its sha256 → `rebuilt_hash`.
8. Verification succeeds iff `rebuilt_hash == original_hash`.

In a workspace with multiple cdylib packages, a verifier MAY rebuild all packages and search for any rebuilt artifact whose hash matches `original_hash`; this accommodates cases where the recorded `bldopt_package` cannot be honored verbatim.

## Limitations

- This SEP makes the *build* reproducible. It does not make the *source* trustworthy. Verification proves the deployed bytes match a particular source tree; whether that source is correct, audited, or non-malicious is an orthogonal concern.
- Class B (host best-effort) verification is environment-dependent. Two verifiers may legitimately disagree.
- Verifiers depend on the integrity of the docker image registry hosting `bldimg` and on the integrity of the source host (`source_repo`). Compromise of either invalidates the verification.
- The current `cliver` regex permits three legacy install-path renderings (clean sha, dirty sha, empty rev). A future revision of this SEP will narrow that regex once cli build tooling normalizes the rendering.

## Design Rationale

**Why digest-pin `bldimg`?** A registry tag is mutable; a content digest is not. A verifier pulling a digest is guaranteed to receive the same bytes the original builder used. Recording a tag would punt the reproducibility question to the registry's mutable state.

**Why allow Class B at all?** Mandating containers would lock out builders without a working daemon (CI environments without privileged docker, restricted corporate networks, hobbyists on locked-down machines). A best-effort tier with explicit "may not match" semantics is more useful than no record at all, provided consumers understand the tier difference.

**Why record `rsver`?** Cargo's wasm output is sensitive to rustc version. Without recording the toolchain, two verifiers on different default toolchains would legitimately produce different bytes from the same source. Recording `rsver` lets each verifier pin to the exact version the original build used (typically via `cargo +<rsver>` on hosts, or `RUSTUP_TOOLCHAIN` inside containers).

**Why does this SEP not define a verification result format?** Verification is a continuous activity and verifiers vary in audience, storage, and tooling preferences. Mandating a schema would over-constrain implementations whose only obligation, from this SEP's point of view, is to follow §3 faithfully. Verifiers are free to publish however suits them; consumers that aggregate across verifiers can adapt to each verifier's format.

**Why have both this SEP and SEP-55?** They answer overlapping but distinct questions. SEP-55 (attestation-based) answers "did a particular trusted CI compile this wasm from this source?" — useful when the verifier is willing to trust the CI provider's signing infrastructure and wants to skip the cost of rebuilding. This SEP (rebuild-based) answers "does this source, compiled with the recorded environment, produce these exact bytes?" — useful when the verifier wants no third-party trust assumption beyond the source host and the container image registry. A wasm that carries meta supporting both gives consumers maximum flexibility; a verifier picks the path matching their threat model.

**Source-repo format alignment with SEP-55.** SEP-55 defines `source_repo` as `github:<user>/<repo>`. This SEP defines it as an HTTPS URL — the form already produced by existing build tooling. Both SEPs are draft and a future revision should converge on a single format (or define independent keys) to avoid ambiguity. Until then, tooling consuming `source_repo` should be tolerant of either form and able to derive a clone URL from each.

**Why `bldopt_*` per-field rather than a single struct?** Meta entries are flat key-value strings. Encoding a struct (e.g. JSON) in a single value is opaque to consumers that just want one field; flat keys are inspectable with grep.

## Security Concerns

- **Source-host trust.** `source_repo` is a URL the verifier fetches from; a compromised host (or an attacker between verifier and host) can serve a different commit at the recorded `source_rev`. SHA-1 collisions in git are not considered practical at the time of writing but verifiers SHOULD prefer hosts that publish signed tags or commit signatures where available.
- **Container-image trust.** A digest pin is integrity-protective only as long as the digest references a valid manifest at the registry. Registry compromise (or image deletion) breaks verification.
- **Verifier compromise.** A verifier can publish false-positive attestations. Consumers SHOULD weigh attestations by verifier reputation and aggregate from multiple independent verifiers.
- **Verifier non-determinism.** A verifier MUST itself be reproducible (pinned cli version, pinned base OS). A drifting verifier produces noise that consumers cannot distinguish from genuine build divergence.
- **Meta tampering.** A malicious builder could publish a wasm with deceptive `source_repo`/`source_rev` claims that don't reproduce. Verification's value is precisely catching this case — a non-matching rebuild is a positive signal that the meta is wrong.
- **What this does not protect against.** This SEP says nothing about the soundness of the source itself. Verified builds are necessary but not sufficient for trust.

## Changelog

* `v0.1.0` - Initial draft.

## Appendix A: Example Implementations

The following implementations demonstrate the spec in practice. They are illustrative, not normative — any tool that produces conformant meta and any verifier that follows §3 satisfies this SEP.

### A.1. `stellar` CLI (build and verify)

The Stellar Development Foundation's `stellar` command-line tool implements both meta-embedding (at build time) and the §3 verification algorithm (via a `verify` subcommand).

**Producing a Class A wasm:**

```
$ stellar contract build --backend docker
ℹ Pulling from stellar/stellar-cli
   Digest: sha256:cb2fc3...
ℹ contract build --manifest-path /source/contracts/foo/Cargo.toml --profile release --locked --meta bldimg=docker.io/stellar/stellar-cli@sha256:cb2fc3...
   Compiling foo v…
✅ Build Complete

$ stellar contract info meta --wasm target/wasm32v1-none/release/foo.wasm
cliver=26.0.0#abc1234567890abcdef1234567890abcdef12345
rsver=1.83.0
bldimg=docker.io/stellar/stellar-cli@sha256:cb2fc3...
source_repo=https://github.com/user/my-contract
source_rev=abc1234567890abcdef1234567890abcdef12345
bldopt_manifest_path=contracts/foo/Cargo.toml
bldopt_package=foo
bldopt_profile=release
```

**Verifying a deployed contract:**

```
$ stellar contract build verify --contract-id CXXX… --network mainnet
ℹ Loading contract from network...
ℹ Loading meta from contract...
   Original wasm hash: 9f86d081…
   stellar-cli version: 26.0.0#abc1234…
   rust version: 1.83.0
   Docker image: docker.io/stellar/stellar-cli@sha256:cb2fc3...
   Manifest path: contracts/foo/Cargo.toml
   Package: foo
   Profile: release
ℹ contract build --manifest-path /source/contracts/foo/Cargo.toml --profile release --locked --meta bldimg=...
   Compiling foo v…
✅ Build Complete
✅ Verified: rebuilt foo wasm matches 9f86d081…
```

In the docker case, the `verify` subcommand pulls `bldimg` and runs the build inside it, setting `RUSTUP_TOOLCHAIN=<rsver>` so the in-container rust matches the recorded version. In the local case, it invokes `cargo +<rsver>` against the host toolchain.

### A.2. CI-driven verification at scale (`contract-verifications`)

The `stellar-experimental/contract-verifications` repository ([link](https://github.com/stellar-experimental/contract-verifications)) is one example of running §3 in CI. It runs a daily GitHub Actions job that walks an upstream wasm corpus, performs §3 against each entry using `stellar contract build verify`, and publishes per-wasm JSON records under version control.

A minimal sketch of the same pattern:

```yaml
name: verify
on:
  schedule: [{ cron: '0 6 * * *' }]
jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: curl -sSf https://soroban.stellar.org/install | sh
      - run: |
          for wasm in wasms/*.wasm; do
            hash=$(sha256sum "$wasm" | cut -d' ' -f1)
            test -f "results/$hash.json" && continue   # idempotency
            stellar contract build verify --wasm "$wasm" \
              | tee "results/$hash.json"
          done
      - run: git add results/ && git commit -m "verify run" && git push
```

A verifier following this pattern in production should:

- pin their own verifier-tool version so their results are themselves reproducible, and disclose that version alongside each result;
- record the original wasm sha256, the rebuilt sha256, and the verification outcome at a minimum;
- avoid mutating published results — re-running a verification produces a new record rather than overwriting an existing one;
- skip wasms already verified by the same verifier at the same tool version (idempotency).

The choice of storage, scheduling, and record format is left to the verifier; conformance to §3 is the only thing that makes results comparable across verifiers.
