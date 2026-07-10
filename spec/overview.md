# Axiom IR — Specification Overview

Axiom IR is a universal, proof-carrying intermediate representation for machine reasoning.

Axiom IR lets AI systems, tools, humans, and independent runtimes exchange reasoning as
portable computational objects rather than disposable natural-language traces.

A model must not merely emit an answer. It must be capable of emitting an **Axiom Module**
containing claims, evidence, assumptions, contexts, operations, derivations, proof
obligations, uncertainty values, contradictions, provenance, and invalidation conditions.

An Axiom Module is:

* **executable** — its state transitions are real computations, not documentation;
* **verifiable** — verification is a transition reachable only through discharged first-class
  proof-obligation nodes;
* **serializable** — a canonical, content-addressed, portable encoding;
* **explainable** — every conclusion is traceable to its operation, evidence, and assumptions;
* **incrementally recomputable** — changing one premise invalidates exactly the represented
  dependent region;
* **portable across model vendors** — the encoding and semantics are independent of any
  producer.

Axiom IR occupies, for machine reasoning, the role an intermediate representation occupies
for compilers: precise semantics, stable interchange, transformation safety, independent
implementations, verification, optimization, and conformance.

## Central abstraction

```
reasoning = typed, inspectable, replayable state transitions over claims
```

Axiom is **not** a knowledge graph, a workflow engine, a theorem prover, a proof assistant,
a Datalog engine, a build system, a factor graph, a probabilistic programming language, an
argumentation framework, a notebook, a JSON schema, a chain-of-thought format, a structured
output protocol, an agent trace, or a generic DAG executor. See
[`docs/research/novelty-audit.md`](../docs/research/novelty-audit.md) for the formal
novelty argument.

## The one-sentence definition

> Axiom IR is a typed reasoning transition calculus in which a claim becomes *verified* only
> through derivation+receipt transitions in which every mandatory proof obligation is a
> first-class node in state `satisfied` (or `waived` under a spec-defined weaker class),
> where contradiction is a preserved first-class witness, where contexts are first-class
> persistent environments, and where the entire verified state is reconstructible by
> deterministic replay from content-addressed external receipts, yielding a
> dependency-correct incremental-invalidation engine.

## Document map

| Document | Scope |
|---|---|
| [`semantics.md`](semantics.md) | Normative transition calculus, judgments, invariants, proof sketches |
| [`type-system.md`](type-system.md) | Initial claim type system, value model, canonicalization |
| [`instructions.md`](instructions.md) | Instruction set: syntax, type rules, transitions, examples |
| [`language.md`](language.md) | The textual Axiom language: grammar, lexer, parser, formatter |
| [`encoding.md`](encoding.md) | Canonical encoding, content addressing, hashing, test vectors |
| [`contexts.md`](contexts.md) | Context semantics, branching, merge, isolation |
| [`uncertainty.md`](uncertainty.md) | Typed uncertainty algebra, conversion rules |
| [`contradictions.md`](contradictions.md) | Contradiction kinds, witnesses, preservation |
| [`receipts.md`](receipts.md) | External-call receipt model, integrity, replay |
| [`extensions.md`](extensions.md) | Open-standard extension model, negotiation, rejection |
| [`conformance.md`](conformance.md) | Conformance corpus structure and runner |

## Foundational invariant (verified execution mode)

No verified claim may exist without a complete, inspectable derivation record. Every
verified derived claim MUST identify:

1. its typed premises,
2. its operation,
3. its evidence dependencies,
4. its assumptions,
5. its active context,
6. its uncertainty semantics,
7. its discharged proof obligations,
8. its provenance,
9. its invalidation conditions,
10. the semantic version of every rule required to reproduce it.

Unsupported assertions may exist only in explicitly unverified states
(`asserted`, `assumed`, `observed`, `pending`, `disputed`, `challenged`,
`externally attested`). They MUST NOT silently become verified.

## Normative language

This specification uses **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**,
**SHOULD**, **SHOULD NOT**, **MAY** as defined by RFC 2119.

Explanatory notes are set off from normative text and are not independently normative.

## Conformance

The reference implementation in `crates/` is one conformant implementation. The
specification is authoritative: where the reference implementation disagrees with this
document, the document wins and the implementation is a bug. See
[`conformance.md`](conformance.md) for how an independent implementation reproduces the
expected corpus.
