# Changelog

All notable changes to Axiom IR are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to semantic versioning once a stable API is declared.

## [0.2.0] - 2026-07-13

Security hardening and the deterministic producer pipeline that close all findings
(F1–F5) from `AUDIT-2026-07-11.md`.

### Added

- **Cryptographic receipt + evidence authenticity** (`crates/axiom-core/src/crypto.rs`):
  keyed HMAC-SHA256 over host-configured `TrustRoot`s. Receipts are signed under a
  trust root on `add_receipt`; `Receipt::verify_integrity` checks both tamper-evidence
  and HMAC authenticity. A `trust = trusted` evidence node is authenticated against
  configured trust roots when present.
- **Enforced evidence authenticity through the real language surface.** The textual
  `evidence` statement now accepts `trust = <tier>`, `provider = "<ident>"`, and
  `signature = "<hex>"` clauses. `exec_evidence` threads provider + signature into the
  evidence node and rejects forged, unsigned, or mismatched trusted evidence via
  `EvidenceForgery`. (Closes audit F1.)
- **`axiom-producer` crate:** deterministic translator from a structured `ReasoningPlan`
  (JSON) to valid Axiom IR source. Output types and arities are validated against the
  shared operation registry, never a `op.starts_with('q')` heuristic. Wired into CLI
  demo 8. (Closes audit F2, F3.)
- **Conformance fixtures for evidence authenticity:**
  `conformance/valid/evidence_unverified.axiom` and
  `conformance/invalid/evidence_trusted_unsigned.axiom`. Corpus grows to 17 fixtures.
- **Runtime tests** (`crates/axiom-runtime/tests/evidence_auth.rs`) exercising the
  production evidence-auth path end to end: a valid signature passes; missing
  provider/signature, wrong provider/key, and tampered content/locator/media/label all
  fail; formatter and canonical round-trips preserve authentication; replay rejects
  tampered evidence.

### Security

- Evidence authenticity is now reachable for parsed and produced modules, not only
  programmatically-constructed evidence. Forged high-trust evidence is rejected when
  trust roots are configured (closes audit F1).
- Receipt binding is recomputed from the derivation's actual input values
  (`ReceiptInputMismatch`); live tool calls require a matching configured `TrustRoot`.
- The replay-path authenticator is consolidated into the execution gate (closes audit
  F4); an unused import was removed (closes audit F5).

### Notes

- No breaking changes to the IR, encoding, or CLI surface. Bumped to 0.2.0 for the new
  capability and the security closure.

[0.2.0]: https://example.org/axiom-ir/releases/tag/0.2.0

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
