# Axiom IR — Instruction Set

Each instruction is a transition on the module state `M` (see
[`semantics.md`](semantics.md)). The set is deliberately orthogonal: a small core of
primitive transitions composes into the full reasoning lifecycle. Every instruction has a
textual form (see [`language.md`](language.md)), type rules, a state transition, verification
behavior, context behavior, failure cases, and a canonical representation.

Conventions: `label` is a source symbol; `:=` denotes assignment to the addressed claim
node; `ctx` defaults to the root context.

---

## 1. `assert`

```
assert <label> = <expr> : <type> [evidence [<labels>]] [uncertainty <u>] [ctx <ctx>]
```

Introduces an unsupported claim in `asserted` state.

* **Type rule:** `expr : type`.
* **Transition:** new claim `c`, `c.status = asserted`, `c.evidence = evidence`,
  `c.context_id = ctx`.
* **Verification behavior:** `asserted` is **not** verifiable. `verify` on it fails with
  `ERR-SILENT-VERIFY`.
* **Context:** `X_ctx.local_claims += c`.
* **Failure:** type error; unknown evidence label; malformed expression.
* **Canonical:** claim node id over `(label, ctx)`; semantic id over `(type, value)`.

Example:

```
assert pi_approx = 3.14159 : decimal uncertainty numeric(3.14, 3.15)
```

---

## 2. `observe`

```
observe <label> = <expr> : <type> [evidence [<labels>]] [uncertainty <u>]
```

As `assert` but `c.status = observed`. The provenance channel for sensor reads, tool reads,
and attestations.

* **Verification behavior:** `observed` is not verifiable without a derivation.
* Example:
  ```
  observe ambient = q(21.5 "C") : quantity evidence [thermometer]
  ```

---

## 3. `assume`

```
assume <label> = <expr> : <type> scope "<scope>" [ctx <ctx>]
```

Introduces an assumption node and an `assumed` claim.

* **Type rule:** `expr : type`.
* **Transition:** `c = assume(...)` with `status = assumed`; new assumption `a`;
  `X_ctx.assumptions += a`; `c.assumptions += a`.
* **Verification behavior:** `assumed` is not verifiable; but a *derived* claim may depend on
  it.
* **Failure:** type error.
* **Invalidation:** removing/contradicting `a` invalidates every verified claim whose
  dependency chain reaches `a`.

Example:

```
assume frictionless = true : bool scope "physics.ideal"
```

---

## 4. `derive`

```
derive <label> = <op>(<in-labels>) : <type> [receipt <r>] [ctx <ctx>]
```

Applies a registered operation to input claims, producing a `pending` derived claim.

* **Type rule:** `|inputs| = |op.inputs|` and each input value matches the declared input
  type at its position.
* **Transition:** compute output via the executor or resolve from `receipt`; create
  derivation `d`; generate obligations declared by `op`. Mandatory obligations enforced at
  derive time (type-compat, dimensional-consistency) start `satisfied`.
* **Verification behavior:** output is `pending` until `verify`.
* **Context:** `X_ctx.local_claims += c`.
* **Failure:** unknown operation/version; arity mismatch; type error; external op without
  receipt or capability; receipt integrity failure.
* **Example:**
  ```
  derive total = add(a, b) : rational
  derive energy = qmul(force, distance) : quantity
  ```

---

## 5. `require`

```
require <kind> on <label>
```

Creates a first-class proof obligation node targeting a claim.

* **Transition:** `o = obligation(kind, target, severity=mandatory, state=pending)`;
  `C[target].obligations += o`.
* **Verification behavior:** until `o` is discharged, `verify(target)` fails with
  `ERR-OBLIGATION-NOT-DISCHARGED`.
* **Example:**
  ```
  require numeric-bounds on total
  ```

---

## 6. `discharge`

```
discharge <obligation-label> by <evidence-or-receipt> [as <state>]
```

Sets an obligation's state, recorded as traceable to evidence or a receipt.

* **Transition:** `O[o].state = state? ∪ satisfied`; `O[o].discharged_by = by`.
* **Failure:** unknown obligation/evidence/receipt label.
* **Example:**
  ```
  discharge numeric-bounds on total by bounds-check
  ```

---

## 7. `verify`

```
verify <label>
```

Promotes a derived claim to `verified` through the central gate.

* **Precondition:** `c.derivation ≠ none`; all mandatory obligations `satisfied`/`waived`;
  external derivations require an integrity-checked receipt.
* **Failure:** `ERR-SILENT-VERIFY` (no derivation), `ERR-OBLIGATION-NOT-DISCHARGED`,
  `ERR-RECEIPT-REQUIRED`.
* This is the only path to `verified` for derived claims.

---

## 8. `challenge`

```
challenge <label>
```

Sets `c.status = challenged`. The claim remains inspectable; challenges are first-class and
do not erase the derivation.

---

## 9. `contradict`

```
contradict <a> <b> as <kind>
```

Records a first-class contradiction witness between two claims.

* **Transition:** `p = contradiction(kind, {a, b}, ctx, witness)`; preserves both claims,
  their derivations, contexts, evidence.
* **Failure:** fewer than two claims; unknown claim.
* **Example:**
  ```
  contradict sensor_low sensor_high as disjoint-interval
  ```

---

## 10. `branch`

```
branch <label> from <parent> [with <assumptions>]
```

Creates a first-class context with parent link and inherited claims.

* **Transition:** `x = context(parent, label, assumptions, inherited = X_parent.local_claims)`.
* **Failure:** unknown parent/assumption.

---

## 11. `merge`

```
merge <label> = <a> + <b>
```

Controlled merge of two contexts.

* **Transition:** `x = context(label, merge_of=(a,b), inherited = local(a) ∪ local(b))`.
* **Conflict detection:** claims with the same `semantic_id` but different `value` across the
  two branches are reported as merge conflicts and are NOT silently unified.
* **Example:**
  ```
  merge reconciled = optimistic + pessimistic
  ```

---

## 12. `invalidate`

```
invalidate <label> because "<reason>"
```

Sets `c.status = invalidated`, records the reason in `invalidation_conditions`.

---

## 13. `attest`

```
attest <label>
```

Sets `c.status = externally-attested`. Used when an external authority vouches for a claim
without an internal derivation.

---

## 14. `call` (external, capability-gated)

```
call <label> = <op>(<in-labels>) : <type> cap "<capability>"
```

Invokes an external operation under a declared capability. Capture a receipt, then derive.

* **Precondition:** the runtime has been granted `capability`. In replay mode the receipt is
  resolved from preloaded receipts and no capability is required.
* **Failure:** `ERR-CAPABILITY-DENIED` if the capability is not granted and not in replay.

Example:

```
call sum = tool.calculator(a, b) : rational cap "tool:calculator"
```

---

## 15. `replay` (runtime transition)

Not a textual instruction. Given a module and a set of receipts, the runtime reconstructs
every external output from receipts without invoking any live executor. Replay MUST NOT
require the live capability; receipt presence and integrity are sufficient.

---

## Orthogonality note

`assert`/`observe`/`assume` are the three *source* transitions (no derivation). `derive`
and `call` are the two *derivation* transitions. `require`/`discharge`/`verify` are the
*obligation* transitions. `challenge`/`contradict`/`invalidate` are the *status* transitions
that never silently promote. `branch`/`merge` are the *context* transitions. `attest` is the
external-vouch transition. This partitions the lifecycle cleanly: no convenience command
duplicates another's semantic effect.
