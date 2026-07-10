# Axiom IR — Typed Uncertainty Algebra

Axiom does **not** use a universal `confidence: f64`. Uncertainty is one of a small,
explicitly scoped set of models. Each model defines its legal operations, conversion rules,
composition rules, loss of information, and rejection conditions. Implicit conversion
between incompatible models is **forbidden**: combining two incompatible models returns an
error rather than manufacturing fake precision.

## 1. Uncertainty models

| Model | Constructor (text) | Shape | Meaning |
|---|---|---|---|
| `Exact` | `exact` | — | Known with certainty under current receipts/obligations |
| `Unknown` | `unknown` | — | No uncertainty information available |
| `ProbabilityInterval` | `probability(lo, hi)` | `[0,1]²` | Subjective probability bounds |
| `NumericInterval` | `numeric(lo, hi)` | `Num²` | Numeric bounds on a quantity |
| `EvidenceWeight` | `weight(w)` | `[0,1]` | Normalized support weight from evidence |
| `Conflicting` | `conflicting` | — | Known to conflict with a preserved witness |
| `ExternallyAsserted` | `external("src")` | source string | Vouched by an external authority; no internal algebra |
| `Extension` | (opaque) | bytes | Extension-defined model |

## 2. Composition (conjunction)

`combine(u, v)` models "both must hold". Rules:

* `Exact` is the identity: `combine(Exact, u) = u`.
* `Unknown` dominates: `combine(Unknown, _) = Unknown` (nothing can be inferred).
* `Conflicting` dominates: `combine(Conflicting, _) = Conflicting`.
* `ExternallyAsserted` is passed through (we do not recompute it).
* `ProbabilityInterval` × `ProbabilityInterval`: bounds multiplied
  (`lo*lo, hi*hi`), rejecting bounds outside `[0,1]`.
* `EvidenceWeight` × `EvidenceWeight`: weights multiplied, rejecting a product > 1.
* `NumericInterval` × `NumericInterval`: intersection of bounds; if disjoint, the result is
  `Conflicting`.
* Any remaining cross-model pair is **rejected** with `Incompatible`.

## 3. Conversion rules

* There is **no** implicit conversion between models.
* `ProbabilityInterval` and `NumericInterval` are disjoint algebras; combining them is an
  error.
* An extension model combined with any other model is an error (the extension declares its
  own composition; the core will not guess).
* Axiom **fails explicitly** rather than approximate.

## 4. Loss of information

* `Unknown` absorbs information (dominates) — this is honest: once uncertainty is unknown,
  it stays unknown.
* `Exact` carries the least information loss; it is the default for pure internal
  operations whose inputs are themselves `Exact`.

## 5. Rejection conditions

* Probability or weight bounds outside `[0,1]`.
* `lo > hi` for any interval.
* Cross-model composition not enumerated above.
* Extension models (which the core cannot compose).

## 6. Determinism

Uncertainty composition is a pure function of its inputs and produces no side effects. It is
exact (no floating point): probabilities/weights are `Num` values. The reference `Uncertainty`
algebra is tested for the conjunction identities and rejection cases.

## 7. Example

```
assert sensor_low  = q(18.0 "C") : quantity uncertainty numeric(15.0, 20.0)
assert sensor_high = q(25.0 "C") : quantity uncertainty numeric(22.0, 28.0)
derive mid = qadd(sensor_low, sensor_high) : quantity
```

The derived midpoint carries the conjunction of the two numeric intervals; if the intervals
were disjoint, the uncertainty would be `Conflicting`.
