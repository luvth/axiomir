# Axiom IR — Limitations

This document states, honestly, what Axiom IR does **not** do. It is grounded in
`spec/semantics.md` §7 ("What is NOT claimed") and
`docs/research/novelty-audit.md` §7 ("Claims we must NOT make"), and is extended
with limitations observed directly in the implementation (`crates/`).

## 1. Axiom is not a sound theorem prover

"Verified" means *obligation-discharged under recorded receipts and the current
rule versions*. It is only as trustworthy as its evidence, its receipts, and its
operation implementations. The central invariant (`INV-VERIFY`, enforced in
`Module::verify` and re-checked by `Module::check_verification_invariant`) does
not express mathematical truth — it expresses that a claim has a complete
derivation, every mandatory obligation is `Satisfied`/`Waived`, and (for external
ops) the receipt integrity-checks. A claim built on false premises, a buggy
operation, or a forged receipt is "verified" in exactly this narrow sense.

## 2. Axiom does not resolve contradictions or do belief revision

A contradiction is a **first-class witness** (`Contradiction` node) that preserves
both branches, their derivations, evidence, and an inspectable `witness`. Axiom
*represents* conflict; it does not arbitrate, retract, or revise beliefs. Conflicting
claims can both remain present (in possibly different contexts), and no rule
silently unifies them — `Module::merge` reports conflicts and refuses to unify
distinct claims with the same `semantic_id`.

## 3. Axiom does not assess premise truth

The runtime checks *structural* and *typing* properties only. Whether a model's
asserted premises correspond to reality is outside scope. Evidence existence is
checked (`CoreError::UnknownEvidence`), but evidence *authenticity* is not
verified by the engine (see §6).

## 4. No cross-platform float determinism

The normative numeric path is **exact**: integer, reduced rational, and
fixed-scale decimal (`Num` in `axiom-types`). IEEE-754 floats are forbidden in
the normative path so that canonical encoding and deterministic replay never
depend on rounding. Values that are genuinely non-deterministic are typed
`NonDeterministic` or `Receipted` and can only be replayed via a receipt.

## 5. Semantic equivalence only where canonicalization is defined

Two claims share a `semantic_id` only when they have identical `(type, value)`.
Structural/alpha-equivalence is defined for values via canonical encoding; beyond
that, equivalence is node identity. Axiom does not attempt semantic normalization
of arbitrary expressions.

## 6. Implementation-level limitations (read from the code)

* **Contradiction classification is partial.** Automatic pairwise detection
  (`Runtime::detect_contradictions` → `classify_contradiction`) covers only three
  cases: boolean negation (`PropositionNegation`), disjoint numeric intervals
  (`DisjointInterval`), and same-magnitude/incompatible-unit quantities
  (`IncompatibleUnit`). The remaining `ContradictionKind` variants
  (`IncompatibleEquality`, `MutuallyExclusiveMembership`, `ViolatedPostcondition`,
  `EvidenceConflict`, `AssumptionConflict`) are **not** produced by automatic
  detection; they can only be recorded via an explicit
  `contradict a b as <kind>` statement. Most conflict kinds are therefore invisible
  unless the module (or a calling tool) asserts them.

* **Automatic contradiction detection is opt-in.** `Runtime::detect_contradictions`
  is a public method invoked by demos (e.g. `demos.rs` demo 2 "auto") but is **not**
  called during `Runtime::execute()` and is **not** run by the `axiom contradictions`
  CLI command (which only lists already-recorded contradiction nodes). A module
  that never emits `contradict` statements yields zero contradictions by default.

* **Cyclic modules fail closed, not with fixed-point semantics.** If dependency
  resolution reaches a state where no remaining statement can make progress,
  `execute` returns an error ("cycle or genuinely missing label"). There is no
  supported cyclic reasoning yet (see `docs/research/roadmap.md`).

* **No default hard node/claim-count cap.** The lexer bounds total tokens
  (`MAX_TOKENS = 1_000_000`), but the runtime does not cap the number of claims,
  obligations, or contradictions created. A maximal module is fully parsed and
  executed.

* **Evidence and receipt signatures are not validated.** `Evidence::signature`
  and `Receipt::signature` are carried but no code path checks them. Receipt
  *integrity* is enforced (hash over recorded fields), but a correct hash with an
  absent/invalid signature still passes. Trust in evidence is the host's
  responsibility.

* **Obligation discharge is coarse.** `discharge` records `discharged_by` as an
  evidence or receipt id and sets the state; there is no proof-term, no SMT
  discharge, and no semantic check that the discharging evidence actually
  justifies the obligation beyond its existence. Mandatory obligations block
  `verify`, but "satisfied" is a state, not a verified entailment.

* **Replay requires the originating receipt set.** Offline replay
  (`Runtime::replay_mode`) only reconstructs external outputs for operations whose
  receipts are supplied; a live-only external call with no recorded receipt cannot
  be replayed and will fail integrity within the verified path.

* **Only two obligation kinds are auto-satisfied at derive time.** In
  `Module::derive`, `TypeCompat` and `DimensionalConsistency` are marked
  `Satisfied` immediately because they are already enforced by the input type
  check / executor. Every other generated obligation (e.g. `NumericBounds`,
  `SourceSupport`, `OpPostcondition`, `ToolReceiptValidation` when no receipt is
  present) starts `Pending` and must be discharged explicitly by the module. A
  derived claim therefore cannot reach `Verified` until those pending mandatory
  obligations are discharged — but the engine provides no automatic way to satisfy
  them, so the module author must supply the discharging evidence/receipt.

* **`verify` is non-fatal and quarantine-based.** A `verify` statement that cannot
  be satisfied does not abort execution; the claim simply remains unverified and
  the failure is recorded in `Runtime::verify_failures`. This preserves valid
  reasoning but means "the module executed" is not the same as "every claim
  verifies" — tooling must inspect `verify_failures` and `verified_claims()`,
  not merely the absence of an error.

## 7. What this means in practice

Axiom gives you a tamper-evident, deterministic, obligation-gated record of
*how* a conclusion was reached and *whether* its mechanical preconditions were
met. It does not tell you the premises were true, the operations were correct, or
that two conflicting conclusions have been reconciled. Use it as an audit and
replay substrate for machine reasoning, not as a proof of correctness.
