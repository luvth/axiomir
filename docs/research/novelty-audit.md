# Axiom IR — Novelty Audit

*Document status: normative for the research record. Written before broad implementation.*
*Author: founding researcher. Date: 2026-07-10.*

This audit states, without inflation, what Axiom IR inherits from prior fields, what
it changes, what is genuinely new, what is merely engineering synthesis, what claims
we must **not** make, and how the design could be falsified.

---

## 1. Position of this audit

The assignment requires that Axiom IR not reduce to a renamed version of any single
prior formalism. It also forbids the lazy claim "Axiom combines X and Y." The honest
verdict is: **every individual component of Axiom is inherited from an existing field;
the contribution is the precise integration of those components into a single,
portable, proof-carrying, deterministic IR for machine reasoning, governed by one
central invariant and one sharp primitive.**

We name the primitive explicitly in §4. We then defend it against each candidate
reduction in §5.

---

## 2. What is inherited (and from where)

| Mechanism | Prior field | What we adopt |
|---|---|---|
| Typed claims as propositions | Type theory, proof assistants | Claim carrier types (Bool, Int, Qty…) |
| Proof obligations gating proof | Proof assistants, ATP | Obligations as first-class nodes |
| Assumption-based contexts | ATMS (de Kleer) | Contexts = assumption sets |
| Justification-based retraction | JTMS (Doyle) | Dependency-tracked invalidation |
| Contradiction as object | Argumentation frameworks | Contradiction witness node |
| Deterministic re-execution | Build systems, self-adjusting computation | Receipt-based replay |
| Content-addressed identity | Git, IPFS, provenance systems | Domain-separated semantic hashes |
| Provenance graphs | Provenance (PROV) | Evidence + provenance nodes |
| Uncertainty algebras | Probabilistic programming, Dempster–Shafer | Typed uncertainty, no implicit conversion |
| Versioned transformations | Build systems, capability systems | Versioned operations + capabilities |

No component is claimed as invented. The audit below is about the combination.

---

## 3. What Axiom changes relative to each field

- **vs proof assistants (Coq, Lean, Isabelle):** those require a *human-constructed*
  formal proof term. Axiom does **not** require a proof term. Axiom requires a
  *complete, inspectable derivation record* plus *discharged first-class proof
  obligations*; the "proof" is the executed derivation plus its receipts and discharged
  obligations, not a hand-written tactic script. Axiom is an *execution record*, not a
  formal proof language.
- **vs theorem provers (Z3, Vampire):** those discharge obligations with search over a
  fixed logic. Axiom treats obligation *discharge* as an operation whose result is
  recorded as a node; the obligation node is addressable and participates in incremental
  invalidation. Axiom does not itself do search; it records that search happened and
  under what version.
- **vs Datalog / logic programming:** Datalog derives facts by rule application over a
  monotone least model; contradictions are undefined or reduce to negation-as-failure.
  Axiom has **no least-model semantics**, supports **explicit contradictory states that
  do not erase either branch**, carries **uncertainty and evidence**, and gates
  "verified" behind obligation discharge.
- **vs abstract interpretation:** that computes sound approximations of program runs.
  Axiom computes a *recorded* reasoning graph; it is not required to be a sound
  abstraction of anything.
- **vs knowledge graphs:** those are RDF-like triple stores with no obligation-gated
  verification, no contradiction-as-witness, no deterministic replay from receipts, no
  incremental-invalidation frontier.
- **vs workflow / DAG engines:** those schedule tasks for side effects. Axiom derives
  *claims* and gates *verification*; tasks (operations) are one node kind among many.
- **vs provenance systems (PROV, provenance DAGs):** those record *where data came
  from*; they do not define a *verified* state reachable only through discharged proof
  obligations, nor contradiction witnesses, nor context-local validity.
- **vs self-adjusting computation / build systems:** those recompute changed outputs.
  Axiom recomputes *verification state* (not just values) and preserves
  context-specific contradictions.
- **vs truth-maintenance systems (TMS/ATMS):** ATMS enumerates contexts (label sets)
  under which a node is in/out. Axiom keeps contexts as first-class persistent
  environments with parent links and explicit merge semantics, makes contradictions
  explicit witnesses, and adds obligation-gated verification on top.

---

## 4. The genuinely new primitive

> **Central contribution.** Axiom IR is a *typed reasoning transition calculus* in which
> **verification is a transition reachable only through the discharge of first-class,
> content-addressed proof-obligation nodes**, where **contradiction is a preserved
> first-class witness (not erasure)**, where **contexts are first-class persistent
> environments with explicit, merge-conflict-aware semantics**, and where **the entire
> verified state is reconstructible by deterministic replay from content-addressed
> external receipts** — yielding a **dependency-correct incremental-invalidation engine
> that invalidates exactly the represented dependent region and nothing else.**

The single sharp primitive is best stated as:

> **A verified claim is a node whose *only* valid producers are derivation+receipt
> transitions in which every mandatory proof obligation is a first-class node in state
> `satisfied` (or `waived` under a spec-defined weaker class), and whose complete
> derivation record is itself a content-addressed, replayable object.**

This is sharper than "a knowledge graph with provenance" because the *gate* is a set
of addressable, dependency-tracked obligation nodes, and the *guarantee* is
reconstructibility by deterministic replay. Neither property holds in any single
prior formalism as an interchange IR.

---

## 5. Defense against the required reductions

- **"Why is this not just Datalog?"** Datalog has no verified/obligation gating, no
  uncertainty, no evidence relevance distinction, no preserved contradictions, no
  replay from receipts, no incremental *verification-state* invalidation. Axiom modules
  may *contain* Datalog-like derivations but are not a logic program.
- **"Why is this not just a proof assistant?"** No proof term is required; the proof is
  the executed, receipted derivation plus discharged obligations. A hostile,
  unverified module is a first-class object; Coq/Lean have no "asserted-but-unverified
  claim" state machine.
- **"Why is this not just a knowledge graph?"** KG nodes have no obligation-gated
  verified transition, no contradiction witnesses, no context-local validity, no replay.
- **"Why is this not just a workflow engine?"** Workflows schedule effects; Axiom
  derives *claims* and gates *verification*.
- **"Why is this not just a provenance DAG?"** Provenance DAGs record ancestry; they
  do not define a verified state reachable only via discharged obligations, nor
  contradiction witnesses, nor context-local validity.
- **"Why is this not just an argumentation framework?"** Argumentation frameworks
  compute acceptability of arguments under attack relations; Axiom records *how a claim
  was produced* (operation + receipts + obligations) and reconstructs it by replay, not
  merely its defeat relation.

---

## 6. What is merely engineering synthesis

- Canonical JSON encoding, domain-separated hashing, the CLI surface, the conformance
  runner, the Python bridge, the benchmarking harness, and the module file format are
  engineering, not research. They are necessary for a credible *standard* but carry no
  novel semantics.

---

## 7. Claims we must NOT make

- We do **not** claim Axiom is a sound theorem prover or that verified claims are
  mathematically proven. "Verified" means *obligation-discharged under recorded
  receipts and the current rule versions* — it is as trustworthy as its receipts,
  evidence trust classification, and operation implementations.
- We do **not** claim semantic equivalence beyond the explicitly defined canonicalization
  (structural/alpha-equivalence where defined; otherwise node identity).
- We do **not** claim cross-platform floating-point determinism. Exact arithmetic
  (integer, rational, decimal) is used where determinism matters; floats are forbidden
  in the normative numeric path.
- We do **not** claim to solve general contradiction resolution or belief revision.
  Axiom *represents* contradictions; it does not *resolve* them.
- We do **not** claim to detect all unsound reasoning. Axiom detects structural,
  type, obligation, receipt, and contradiction violations; it cannot judge whether a
  model's premises are true.

---

## 8. How the design could be falsified

1. **Replay falsification.** If two runs with identical canonical module + identical
   operation versions + identical receipts + identical capabilities ever produce
   different claim/derivation/obligation/event/digest output, the central determinism
   claim is false. (Conformance + replay tests assert this.)
2. **Invalidation falsification.** If removing a required premise ever fails to
   invalidate a represented dependent, or invalidates a claim with no represented
   dependency path to the changed node, the incremental claim is false. (Incremental
   tests assert minimality.)
3. **Verification-gate falsification.** If a claim ever reaches `verified` with a
   mandatory obligation not in `satisfied`/`waived`, the invariant is false. (Obligation
   tests assert this; the runtime forbids the transition.)
4. **Novelty falsification.** If an independent implementer can reproduce Axiom's
   observable behavior using *only* an existing formalism (e.g., a pure Datalog engine
   or a bare KG) without the obligation-gate + replay + contradiction-witness machinery,
   then the novelty claim collapses to engineering synthesis and must be restated.

---

## 9. Conclusion

Axiom IR is defensible as a *new integration standard*, not as a new logic. Its
non-negotiable novelty is the **obligation-gated, receipt-replayable,
contradiction-preserving, context-aware verification transition over a typed claim
graph with dependency-correct incremental invalidation.** The implementation and
conformance suite are the evidence; §8 lists the falsifiers.
