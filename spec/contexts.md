# Axiom IR — Contexts

Axiom MUST support incompatible reasoning branches without corrupting the global module. A
**context** is a persistent reasoning environment, not a tag.

## 1. Context structure

```
x = ( id, parent, label, assumptions, inherited_claims, local_claims,
      contradictions, merge_of )
```

* `id` — content-addressed over `(parent, label, assumptions)`. The root context has a fixed
  marker id (`parent = none`).
* `parent` — forms a tree of contexts.
* `assumptions` — assumptions active in this context.
* `inherited_claims` — claims visible by inheritance from ancestors (copied as ids).
* `local_claims` — claims created in this context.
* `contradictions` — contradiction witnesses raised in this context.
* `merge_of` — `Some((a, b))` if this context was produced by a merge.

## 2. Context creation (branch)

`branch L from P with A1, A2` creates `x` with `parent = P`, `assumptions = [A1, A2]`,
`inherited_claims = X_P.local_claims`. The new context sees every claim of its parent.

* A claim created in a child context is local to that child; it does NOT change the parent's
  local set.
* A contradiction detected in a child is recorded in the child's `contradictions` and does
  NOT mutate the referenced claims.

## 3. Context comparison

Two contexts are comparable by their claim sets and statuses. The runtime can report, for a
given `semantic_id`, the set of contexts in which it is `verified`, `contradicted`,
`challenged`, or `invalidated`.

## 4. Controlled merge

`merge M = A + B` produces `x` with `merge_of = (A, B)`,
`assumptions = A.assumptions ∪ B.assumptions`,
`inherited = A.local_claims ∪ B.local_claims`.

### Merge conflicts

If a claim with the same `semantic_id` exists in both `A` and `B` but with a **different
value**, it is a **merge conflict** and is reported. Merge conflicts are:

* **NOT** silently unified.
* recorded in the `Merge` event's conflict count.
* left to the user to resolve explicitly (e.g., by `contradict`, `invalidate`, or a new
  derivation).

Claims present in only one branch are inherited cleanly.

## 5. Branch-local invalidation

Invalidating a claim in one context (or removing an assumption scoped to one context) affects
only claims whose dependency chain reaches it within that context's relevant graph. Because
claim `status`, `derivation`, and `obligations` are properties of the node and the node is
created within a specific context, a status change in one context cannot corrupt an
unrelated context. This is asserted by the context-isolation tests.

## 6. Example

```
assume frictionless = true : bool scope "ideal"
branch optimistic from root with frictionless
derive d = qmul(force, distance) : quantity ctx optimistic
verify d
```

Here `d` is `verified` in `optimistic` but has no node in `root`; removing the `frictionless`
assumption invalidates `d` and only `d`.
