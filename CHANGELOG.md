# Changelog

All notable changes to Axiom IR are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to semantic versioning once a stable API is declared.

## [0.1.0] - 2026-07-10

Initial release. Axiom IR is a universal, proof-carrying intermediate representation and
deterministic runtime for machine reasoning.

The non-negotiable novelty is the **obligation-gated, receipt-replayable,
contradiction-preserving, context-aware verification transition over a typed claim graph with
dependency-correct incremental invalidation** (see `docs/research/novelty-audit.md`). The
central invariant is **INV-VERIFY**: in verified execution mode, every `verified` claim has a
complete derivation, every mandatory proof obligation is a first-class node in state
`satisfied` (or `waived`), and any external derivation carries an integrity-checked receipt.

### Added

- **Core IR + calculus.** The typed reasoning transition calculus: claim/evidence/assumption/
  context/derivation/obligation/contradiction/receipt node model, with transactional (all-or-
  nothing) state transitions (`spec/semantics.md`).
- **Canonical encoding + domain-separated hashing.** Stable content-addressed identities; claim,
  evidence, context, derivation, obligation, contradiction, receipt, and module ids are hashed
  under per-domain tags so they can never collide through identical raw serialization.
- **Exact-arithmetic type system + typed uncertainty algebra.** Integer, reduced rational, and
  fixed-scale decimal values (floating point is forbidden in the normative path); quantities with
  dimensional units; uncertainty kinds `exact`, `unknown`, `conflicting`, `probability`, `numeric`,
  `weight`, and externally-asserted — with no implicit conversion.
- **Textual language + parser + formatter.** A line-oriented grammar (`assert`, `observe`,
  `assume`, `derive`, `require`, `discharge`, `verify`, `contradict`, `branch`, `merge`,
  `invalidate`, `attest`, `call`, `evidence`) with a canonical, idempotent printer.
- **Deterministic runtime.** Obligation engine, contradiction detection, receipts, and replay;
  identical inputs/operations/capabilities/evidence/receipts always yield an identical module
  digest and event log.
- **Dependency-aware incremental invalidation.** Removing or changing a node invalidates exactly
  its represented transitive dependents (INV-MINIMALITY) and recomputes only the affected
  frontier, emitting a machine-readable `InvalidationReport`.
- **Explanation engine.** `axiom explain` and `axiom trace` surface the operation, inputs,
  evidence, assumptions, and obligations behind any claim.
- **Conformance runner + fixtures.** `axiom conform` runs the `valid/` and `invalid/` fixtures
  under `conformance/`.
- **CLI.** The `axiom` binary: `check`, `fmt`, `run`, `verify`, `explain`, `trace`,
  `contradictions`, `invalidate`, `replay`, `diff`, `graph`, `inspect`, `conform`, `doctor`,
  `demo`.
- **Rust SDK.** `axiom-sdk` for embedding Axiom IR in Rust programs.
- **Python bridge.** A Python binding surface for loading, executing, and inspecting modules.
- **Benchmarks.** A Criterion harness (`axiom-benches`) for execution and encoding throughput.
- **Fuzz harness.** `cargo-fuzz` targets for the parser and runtime (excluded from the workspace;
  requires nightly).

### Notes

- "Verified" means *obligation-discharged under recorded receipts and the current rule versions*;
  Axiom is **not** a sound theorem prover and does not resolve contradictions or perform belief
  revision (see `spec/semantics.md` §7 and `docs/research/novelty-audit.md` §7).
- The `spec/` directory is the source of truth; the implementation must not diverge from it.

[0.1.0]: https://example.org/axiom-ir/releases/tag/0.1.0
