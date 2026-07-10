# Axiom IR — Normative Semantics

This document is the normative definition of the Axiom transition calculus. It specifies
module state, valid state transitions, the central invariant, and proof sketches for the
properties that the conformance suite and tests assert.

Where this document conflicts with the reference implementation (`crates/`), this document
is authoritative.

## 1. Module state

A module is a tuple of labelled, addressable collections:

```
M = (C, E, A, X, D, O, P, R)
```

| Component | Meaning | Implementation |
|---|---|---|
| `C` | claims | `Module.claims` |
| `E` | evidence | `Module.evidence` |
| `A` | assumptions | `Module.assumptions` |
| `X` | contexts | `Module.contexts` |
| `D` | derivations | `Module.derivations` |
| `O` | proof obligations | `Module.obligations` |
| `P` | contradiction witnesses | `Module.contradictions` |
| `R` | external receipts | `Module.receipts` |

Side structures: `operations` (registered operation definitions), `events` (the
deterministic event log), and a distinguished `root` context `X_root`.

All identifiers are content-addressed (see [`encoding.md`](encoding.md)). Identifiers are
**domain-separated**: a claim id, evidence id, assumption id, context id, derivation id,
obligation id, contradiction id, receipt id, and module id can never collide through
identical raw serialization because each is hashed under its own domain tag.

### Claim

A claim `c ∈ C` is a record:

```
c = ( id, semantic_id, label, span, ty, value, status, uncertainty,
      context_id, assumptions, evidence, derivation, obligations,
      invalidation_conditions, provenance )
```

* `id` — **node identity**: stable across recomputation; depends on `(label, context_id)`,
  NOT on `value`.
* `semantic_id` — **semantic identity**: a content hash over `(ty, value)` only. Two claims
  with the same proposition share a semantic id regardless of context, status, or label.
* `status` — one of `asserted | assumed | observed | pending | disputed | challenged |
  verified | invalidated | externally-attested`.

### Evidence

An evidence node `e ∈ E` carries `content_hash`, `media_type`, `provenance`, acquisition
metadata, `trust` classification, and optional `signature`. Axiom distinguishes **evidence
existence** from **evidence relevance**: attaching evidence to a claim records a dependency;
it does not discharge any obligation and does not make the claim verified.

### Assumption

An assumption `a ∈ A` is an addressable, scoped, challengeable, removable semantic input
that introduces exactly one claim. Removing or contradicting an assumption MUST invalidate
every verified claim whose dependency chain reaches it, and only those claims.

### Context

A context `x ∈ X` is a persistent reasoning environment:

```
x = ( id, parent, label, assumptions, inherited_claims, local_claims,
      contradictions, merge_of )
```

Contexts form a tree via `parent`. A claim is *local* to the context in which it was
created and *inherited* into child contexts. A claim MAY be verified in one context and
contradicted in another; contexts do not corrupt one another.

### Derivation

A derivation `d ∈ D` records the application of an operation to specific inputs in a
specific context:

```
d = ( id, op, op_version, inputs, output, output_label,
      generated_obligations, receipt, provenance, runtime_version )
```

### Proof obligation

A proof obligation `o ∈ O` is a first-class node:

```
o = ( id, kind, target, inputs, severity, state, discharged_by, message )
```

`state ∈ { pending, satisfied, failed, waived, unsupported }`. `severity ∈ { mandatory,
advisory }`. A mandatory obligation that is not `satisfied` or `waived` blocks `verify`.

### Contradiction

A contradiction `p ∈ P` is a first-class witness:

```
p = ( id, kind, claims, context, witness, evidence )
```

It preserves the conflicting claims, their derivations, their contexts, their evidence, and
an inspectable `witness`. It MUST NOT erase either branch.

### Receipt

A receipt `r ∈ R` is an immutable record of an external call outcome, carrying the
requested operation, normalized inputs hash, returned output, schema, provider, operation
version, logical time, and an integrity digest. Replay reconstructs the external output
from the receipt without re-invoking the external operation.

## 2. The central invariant

> **INV-VERIFY.** In verified execution mode, for every claim `c` with
> `c.status = verified`:
>
> 1. `c.derivation` is `Some(d)` with `d ∈ D` complete (all fields populated).
> 2. For every `o` in `c.obligations`: if `o.severity = mandatory` then
>    `o.state ∈ { satisfied, waived }`.
> 3. If the derivation's operation is external, `d.receipt` is `Some(r)` with
>    `r.verify_integrity() = true`.
> 4. The semantic version of every operation used to reproduce `c` is recorded.

No path MAY set `c.status = verified` for a claim that lacks a derivation, has an
undischarged mandatory obligation, or (if external) lacks an integrity-checked receipt. The
implementation enforces this in `Module::verify` and re-checks it after execution via
`Module::check_verification_invariant`.

## 3. Transition judgments

Each instruction is a judgment of the form `M, Γ ⊢ instr → M', Δ` where `Γ` is the
capability set and `Δ` is the emitted event(s). Preconditions are explicit; failure aborts
the transition without leaving a partially mutated semantic state (transactional
execution).

### assert

```
M ⊢ assert(label, ty, value, u, evs, ctx?) → M'
  pre:  value : ty
        ∀ e ∈ evs . e ∈ E
        ctx = ctx? ∪ X_root
  eff:  new claim c, status = asserted, c.evidence = evs, c.context_id = ctx
        X_ctx.local_claims += c.id
  event: Assert(c, asserted)
```

### observe

As `assert`, but `status = observed`. (`observe` is the provenance-channel form of
assertion: sensor reads, tool reads, attestations.)

### assume

```
M ⊢ assume(label, ty, value, scope, ctx?) → M', (a, c)
  pre:  value : ty
  eff:  c = assert(label, ty, value, unknown, [], ctx) with status = assumed
        a = (id, label, claim=c.id, scope, context=ctx, challengeable, removable)
        X_ctx.assumptions += a.id ; c.assumptions += a.id
  event: Assume(a, c)
```

### derive

```
M ⊢ derive(op, op_version, inputs, ctx?, out_label, receipt?, exec) → M'
  pre:  op identified by (op, op_version) is registered
        inputs ⊆ C ; |inputs| matches op.inputs arity
        ∀ i ∈ inputs, i.value : op.inputs[position]
        if op is external and requires a receipt: receipt ≠ none and r.verify_integrity()
        capability Γ permits op.capabilities
  eff:  v = exec(op, input.values)         // or resolved from receipt
        u = uncertainty_rule(op, inputs)
        c = claim(out_label, op.output, v, u, ctx) status = pending
        d = derivation(op, op_version, inputs, c, receipt?)
        c.derivation = d ; O += generated obligations (mandatory kinds auto-satisfied
            when enforced at derive time, e.g. type-compat, dimensional-consistency)
  event: Derive(d, c, op)
```

External operations MUST NOT be treated as pure unless a receipt makes replay possible. If
an external operation is requested without a receipt and no live executor with the required
capability is available, the transition fails.

### require

```
M ⊢ require(kind, target) → M'
  eff:  o = obligation(kind, target, severity=mandatory, state=pending)
        O += o ; C[target].obligations += o.id
```

### discharge

```
M ⊢ discharge(obligation_node, by, state?) → M'
  pre:  obligation_node ∈ O
        by ∈ E ∪ R
  eff:  O[obligation_node].state = state? ∪ satisfied
        O[obligation_node].discharged_by = by
  event: Discharge(obligation_node, by, state)
```

Discharging an obligation with `failed`, `waived`, or `unsupported` changes its state; only
`satisfied` (or `waived` under a spec-defined weaker class) permits `verify` to proceed.

### verify

```
M ⊢ verify(c) → M'
  pre:  c ∈ C ; c.derivation ≠ none
        ∀ o ∈ c.obligations . (o.severity = advisory) ∨ (o.state ∈ {satisfied, waived})
        if external op: c.derivation.receipt ≠ none and integrity-checked
  eff:  c.status = verified
  event: Verify(c)
  fail: otherwise ERR-SILENT-VERIFY (asserted/assumed/observed have no derivation;
        mandatory obligation undischarged; external without receipt)
```

### challenge

```
M ⊢ challenge(c) → M'
  eff:  c.status = challenged
  event: Challenge(c)
```

### contradict

```
M ⊢ contradict({c1, c2, ...}, kind, witness, ctx?) → M'
  pre:  |{c1,...}| ≥ 2 ; ∀ ci ∈ C
  eff:  p = contradiction(kind, claims, ctx, witness)
        P += p ; X_ctx.contradictions += p.id
  event: Contradict(p)
```

### branch

```
M ⊢ branch(label, parent, assumptions) → M'
  pre:  parent ∈ X ; assumptions ⊆ A
  eff:  x = context(parent, label, assumptions,
                     inherited = X_parent.local_claims)
  event: Branch(x, parent)
```

### merge

```
M ⊢ merge(a, b, label) → M'
  pre:  a ∈ X ; b ∈ X
  eff:  x = context(label, merge_of=(a,b), assumptions = A_a ∪ A_b,
                     inherited = local_claims(a) ∪ local_claims(b))
        conflicts = { semantic_id(s) : s ∈ local_claims(a) and a distinct claim with the
                     same semantic_id but different value exists in local_claims(b) }
        conflicts are reported and NOT silently unified
  event: Merge(x, a, b, |conflicts|)
```

### invalidate

```
M ⊢ invalidate(c, reason) → M'
  eff:  c.status = invalidated ; c.invalidation_conditions += reason
  event: Invalidate(c, reason)
```

### attest

```
M ⊢ attest(c) → M'
  eff:  c.status = externally-attested
  event: Attest(c)
```

### call (external, capability-gated)

```
M ⊢ call(out_label, op, inputs, ty, cap) → M'
  pre:  cap ∈ Γ
  eff:  live executor runs op(inputs) → receipt r (if not replay)
        or replay resolves r from preloaded receipts
        then derive(out_label, op, inputs, ty, receipt=r)
  event: receipt captured ; Derive(...)
```

### replay

Given a module `M` and a set of receipts `R*`, replay reconstructs every external output
from `R*` without invoking any live executor. Replay MUST NOT require the capability `cap`;
the presence and integrity of the receipt is the sufficient condition.

## 4. Determinism

Given identical:

* canonical module input,
* operation implementations and versions,
* capability configuration,
* evidence,
* external receipts,
* runtime semantic version,

execution MUST produce identical:

* claim identities,
* derivation identities,
* obligation states,
* contradiction witnesses,
* event log,
* exported results,
* final module digest.

The normative numeric path is **exact** (integer, reduced rational, fixed-scale decimal) so
that canonical encoding and deterministic replay never depend on IEEE-754 rounding. Floating
point is forbidden in the normative path. Where a value is genuinely non-deterministic, the
operation's determinism class is `NonDeterministic` or `Receipted`, and replay is possible
only via a receipt.

## 5. Incremental invalidation

When a node `n` (claim, evidence, assumption, receipt, context ancestor, or operation
version) changes:

1. Compute `frontier(n) = transitive dependents of n` via the reverse dependency index.
2. Invalidate every claim in `frontier(n)` whose validity depends on `n` (status =
   `invalidated`, with the change recorded in `invalidation_conditions` and an `Invalidate`
   event).
3. Preserve every verified claim outside `frontier(n)`.
4. Recompute eligible derivations in topological order (a derived claim is recomputable
   only when all its inputs are available and not themselves invalidated).
5. Regenerate affected obligations.
6. Re-detect contradictions in affected regions.
7. Emit a machine-readable `InvalidationReport` explaining every status change.

The engine MUST invalidate exactly `frontier(n)` and no claim outside it: removing an
unrelated premise MUST NOT invalidate independent conclusions (INV-MINIMALITY).

## 6. Proof sketches

### 6.1 Verified-claim derivation completeness (INV-VERIFY.1)

`verify` checks `c.derivation ≠ none` before setting `verified`. Any code path reaching the
assignment passes that check (```Module::verify``` returns `ERR-SILENT-VERIFY` otherwise).
Therefore every `verified` claim has a derivation. ∎

### 6.2 Deterministic replay under fixed receipts

External outputs are reconstructed from `receipt.output`, which is fixed by the receipt's
integrity hash. The deterministic executor (`BuiltinExecutor`) is a pure function of
`(op, inputs)`. Pure derivations depend only on inputs and the registered operation, both
fixed. The event log and digest are functions of the committed state, which is identical.
∎

### 6.3 Invalidity soundness (INV-VERIFY after invalidation)

`invalidate` sets status to `invalidated`, which is not `verified`. The reverse dependency
index computes `frontier(n)` transitively, so every claim whose derivation transitively
depends on `n` is reached. A claim with no represented dependency path to `n` is not in
`frontier(n)` and is never touched. ∎

### 6.4 Invalidity minimality (INV-MINIMALITY)

`frontier(n)` is defined as the transitive closure of the reverse dependency relation rooted
at `n`. By construction it contains exactly those claims that transitively depend on `n`
and no others. The recompute step re-derives only claims in `frontier(n)` whose inputs are
available. Therefore invalidation reaches all and only the represented dependents. This is
asserted by the incremental tests, including the "unrelated premise untouched" test. ∎

### 6.5 Context isolation

Claims are created in a specific context; `status`, `derivation`, and `obligations` are
per-claim, not per-context. A contradiction is recorded in the context that raised it and
does not mutate the claims it references. Branch inheritance copies claim ids into
`inherited_claims` but does not alias node state. Therefore an operation on one context
cannot corrupt an unrelated context. ∎

### 6.6 Obligation safety

`verify` iterates `c.obligations` and fails if any mandatory obligation is not
`satisfied`/`waived`. Discharge records `discharged_by ∈ E ∪ R`, so a satisfied obligation
is traceable to evidence or a receipt. `check_verification_invariant` re-validates the whole
module after execution. ∎

### 6.7 Canonical encoding stability

`canonical_bytes` recursively sorts object keys and emits no insignificant whitespace.
`content_id` hashes `domain_tag || 0x00 || canonical_bytes`. Identical semantic content
produces identical canonical bytes and therefore identical ids and digest. This is asserted
by the canonicalization tests. ∎

## 7. What is NOT claimed

* Axiom is **not** a sound theorem prover. "Verified" means *obligation-discharged under
  recorded receipts and rule versions*; it is as trustworthy as its evidence, receipts, and
  operation implementations.
* Axiom does **not** resolve contradictions or perform belief revision. It *represents* them.
* Axiom does **not** detect whether a model's premises are true.
* Axiom does **not** claim cross-platform floating-point determinism; the normative path is
  exact arithmetic.
