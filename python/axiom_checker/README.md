# Independent Python conformance checker

A from-scratch Python re-implementation of a **subset** of the Axiom IR
semantics, used to validate part of the conformance corpus *without* depending
on the Rust crates. It reads only the plain-text fixtures and their
`.expect.json` contracts, so it is a genuine independent oracle: a bug in the
Rust runtime cannot mask a bug here, and vice versa.

```sh
python python/axiom_checker/run_conformance.py
# -> [PASS] arithmetic (valid) ok
#    [PASS] evidence_trusted_unsigned (invalid) rejected: ...
#    [SKIP] context (valid) unsupported: ...
#    independent Python checker: 14 passed, 0 failed, 3 skipped (of 17 fixtures)
```

## What it covers

- Values: exact integers/rationals, quantities with units, booleans.
- Operations: `core.add/sub/mul/div`, `core.qadd/qsub/qmul/qdiv`,
  `core.eq/neq/lt/le/gt/ge`, `core.interval`, `core.not`.
- **Obligation-gated verification**: `core.div` emits a *mandatory*
  `numeric-bounds` obligation that blocks `verified` until explicitly
  `discharge`d — the central novelty, checked independently.
- **Evidence authenticity**: a `trust=trusted` evidence node with no
  `provider`/`signature` is rejected.
- Failure modes: fabricated evidence, type mismatch, unit mismatch,
  undeclared premise, unknown operation, syntax error.

## What it does NOT cover (reported as SKIP, not failure)

Contexts (`branch`/`merge`), contradictions (`contradict`), incremental
invalidation (`invalidate`), and external calls + receipt replay (`call`). These
need the full Rust engine; the checker reports them as skipped so the supported
subset (13/17 fixtures) can be asserted cleanly.

This checker is intentionally minimal and exists only to provide an
independent cross-check of the corpus — it is not a substitute for the
reference runtime. It covers 14 of the 17 fixtures; the remaining 3
(contexts, contradictions, and external-call replay) need the full Rust engine
and are reported as skipped.
