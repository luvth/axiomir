# Axiom IR — Contradictions

Contradiction is a first-class semantic event. Axiom supports explicit, preserved
contradictions and MUST NOT erase either branch.

## 1. Contradiction kinds

| Kind | Trigger | Witness content |
|---|---|---|
| `PropositionNegation` | `Bool(x)` vs `Bool(y)`, `x ≠ y` | the two booleans |
| `IncompatibleEquality` | two equalities that cannot both hold | the equalities |
| `DisjointInterval` | `Interval[a,b]` vs `Interval[c,d]` with `b < c` or `d < a` | the intervals |
| `IncompatibleUnit` | equal magnitude, incompatible units | `q1`, `q2` |
| `MutuallyExclusiveMembership` | `InSet(x, S)` vs `NotInSet(x, S)` (or disjoint sets) | the membership relations |
| `ViolatedPostcondition` | an operation postcondition fails | the failed check |
| `EvidenceConflict` | two evidence nodes oppose each other | the evidence ids |
| `AssumptionConflict` | two assumptions are mutually exclusive | the assumption ids |
| `Extension(k)` | an extension-defined contradiction | extension data |

## 2. Contradiction as a witness

A contradiction node `p` records:

```
p = ( id, kind, claims, context, witness, evidence )
```

* `witness` is an inspectable, serializable record (summary + structured detail).
* The **conflicting claims are preserved**: their values, derivations, contexts, and
  evidence are untouched.
* The contradiction is recorded in the raising context's `contradictions` set.

## 3. Detection

Detection is two-level:

1. **Explicit** — the `contradict a b as kind` instruction records a witness the author
   asserts.
2. **Automatic** — `detect_contradictions(ctx)` scans the context's local claims pairwise and
   classifies conflicts using the table above (proposition negation, disjoint intervals,
   incompatible units). Each detected conflict becomes a first-class contradiction node.

## 4. Preservation semantics

* Neither branch is erased.
* A claim may be `verified` in one context and referenced by a contradiction in another.
* Contradictions do not change claim `status`; they are additional structure.
* Removing an assumption that a contradiction depends on changes the contradiction's context
  but does not delete the witness unless the involved claims are themselves invalidated.

## 5. Example

```
observe sensor_low  = q(15.0 "C") : quantity
observe sensor_high = q(28.0 "C") : quantity
contradict sensor_low sensor_high as disjoint-interval
```

The runtime records a `DisjointInterval` witness preserving both observations. The global
module remains in an unresolved conflict state; each branch can still be reasoned about
locally. `detect_contradictions(root)` would also classify this automatically.

## 6. What Axiom does NOT do

Axiom **represents** contradictions; it does **not** resolve them, choose a "winner", or
perform belief revision. Resolution is out of scope and MUST NOT be implied.
