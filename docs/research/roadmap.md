# Axiom IR — Research Roadmap

This document lists the strongest near-term research milestones for Axiom IR.
Each is grounded in the current design (as implemented in `crates/`) and is
scoped to be concrete, credible, and feasible without overpromising.

## 1. Richer obligation engines with SMT-backed discharge

**Why it is valuable.** Today `Module::discharge` records a state
(`Satisfied`/`Waived`/...) tied to an evidence or receipt id, but performs no
semantic check that the discharger actually entails the obligation. Mandatory
obligations (`TypeCompat`, `DimensionalConsistency`, `NumericBounds`,
`ToolReceiptValidation`, `SourceSupport`, `OpPostcondition`) are gated, but their
"satisfied" status is asserted, not derived. An SMT-backed discharge would let
`require numeric-bounds on out` be *proven* from the actual computed bounds rather
than taken on trust, turning Axiom from "obligations are marked done" into
"obligations are discharged by proof."

**Why it is feasible.** The `Obligation` node is already first-class with a
`kind`, `target`, `inputs`, `severity`, and `state`, and the calculus exposes
explicit discharge transitions. An external obligation engine can be plugged in
alongside the existing `OpExecutor` injection pattern (the core already isolates
I/O behind injected callbacks). The exact-arithmetic `Num` type maps cleanly to
SMT bitvector/integer theories.

**What it would need.** A trait `ObligationEngine` mirroring `OpExecutor`;
translation of `ObligationKind` + involved `Value`s into SMT constraints; a
discharge path that emits a proof witness (not just a state) and is checked by
`check_verification_invariant`; regression fixtures in `axiom-conformance`.

## 2. Fixed-point semantics for supported cyclic reasoning

**Why it is valuable.** The current `Runtime::execute` rejects any module whose
dependency graph contains a cycle (it returns an error when the worklist can make
no further progress). Many legitimate reasoning patterns — mutual definitions,
iterative refinement, fixpoint constraints — are cyclic. Supporting a bounded
fixed-point evaluation would let Axiom express and verify a much larger class of
models instead of failing closed.

**Why it is feasible.** Dependencies are already explicit (`Claim::depends_on`,
forward/reverse maps in `axiom-incremental`), and derivation is a pure function of
inputs plus a registered operation. A stratified/coinductive evaluation order over
the existing worklist is a local extension, and `Num`'s exactness keeps
fixed-point iteration terminating in the common monotone cases.

**What it would need.** A cycle classifier in `execute` that distinguishes
*forbidden* cycles (genuine missing labels) from *supported* fixpoint cycles; a
bounded iteration count with a `Divergent` status rather than `Verified`;
invariant updates so `INV-VERIFY` still holds for convergent cycles; conformance
tests for both convergent and divergent cases.

## 3. Formal mechanization of the transition calculus (Coq/Lean)

**Why it is valuable.** `spec/semantics.md` gives an informal central invariant
(`INV-VERIFY`) and proof sketches. A machine-checked proof that "every `Verified`
claim has a complete derivation and discharged mandatory obligations, and that the
digest is deterministic" would convert Axiom's strongest security claim from
"documented and tested" to "proven." This is the cornerstone for trusting Axiom
as audit infrastructure.

**Why it is feasible.** The calculus is small and explicitly transition-based
(`M ⊢ instr → M', Δ` judgments). The core (`axiom-core`) is pure and I/O-free, so
the mechanization maps directly onto `Module` and its mutation functions. The
existing Rust tests (e.g. `verify_requires_derivation_and_obligations`,
`cannot_verify_asserted_claim`, `receipt_tampering_detected`) are precise
specifications to port.

**What it would need.** A Coq/Lean model of `Module` state and each instruction as
a transactional transition; proof of `INV-VERIFY` preservation and
determinism; an extraction/equivalence argument linking the model to the Rust
implementation (or a verified subset).

## 4. Standardized extension registry and governance

**Why it is valuable.** Extensions (`Type::Extension`, `Value::Extension`,
`ContradictionKind::Extension`, `ns:name@version` operations) are namespaced to
prevent collisions, and unrecognized extensions must be rejected explicitly
(`spec/extensions.md`). But there is no registry, version-pinning policy, or
governance for *which* extensions exist and how they are reviewed. A standardized
registry makes modules portable and lets a runtime answer "is this extension
trustworthy?" instead of merely "is it present?"

**Why it is feasible.** The addressing scheme (`ns:name@version`, content-addressed
operation identity via `OperationDef::identity`) already gives stable, collision-
free identifiers. A registry is a metadata layer over the existing identity
scheme; feature negotiation ("syntactically valid but requires unsupported
extension X@Y") is already specified.

**What it would need.** A canonical registry format and a resolver the runtime
queries before execution; a signature/attestation requirement for registered
extensions (closing the "invalid signatures" gap from the threat model); a
governance process for namespace allocation.

## 5. Distributed and attested execution receipts

**Why it is valuable.** Receipts are today local, host-produced records. For
multi-party or adversarial settings, a receipt should be *attested* by its
provider (the `Receipt::signature` field exists but is unenforced) and ideally
verifiable across organizations. This closes the forged-evidence / invalid-
signature gaps and enables untrusted producers to contribute verifiable external
results into a shared module.

**Why it is feasible.** The `Receipt` structure already carries `provider`,
`logical_time`, `inputs_hash`, `output_hash`, `integrity`, and an optional
`signature`. Replay already reconstructs outputs from the receipt alone, so adding
verifier-side signature checks is localized to `Receipt::verify_integrity` and the
replay path in `Runtime::exec_call`.

**What it would need.** A signature scheme and key-trust model (e.g. provider
keys anchored in a trusted set); enforcement of `signature` in
`verify_integrity`; a distributed receipt log format (the CLI already emits a
`ReceiptLog` JSON) so receipts can be shared and replayed across hosts.

---

These five milestones are ordered by leverage: (1) and (3) directly strengthen the
core trust guarantee; (2) expands expressiveness; (4) and (5) make extensions and
external evidence trustworthy and interoperable. None require rearchitecting the
existing pure core — each extends it through the seams (injected executors,
content-addressed identities, first-class obligation/discharge nodes) that are
already present.
