# Axiom IR — Conformance

Axiom is an open standard. This document defines the conformance corpus and runner so that
an independent implementation can reproduce the expected outcomes without reading the Rust
source.

## 1. Corpus layout

The corpus lives in `conformance/`:

```
conformance/
  manifest.toml            ; lists every fixture and its expected outcome class
  valid/                   ; modules expected to execute successfully
    *.axiom                ; source text
    *.expect.json          ; expected: digest, verified claims, events, obligations,
                            ;   contradictions, invalidation sets, replay digest
  invalid/                 ; modules expected to be rejected
    *.axiom                ; source text (syntactically valid or not)
    *.expect.json          ; expected: error kind, span, rejected node
  receipts/                ; captured receipts for replay fixtures
  vectors/                 ; canonical-encoding test vectors
```

Each `*.expect.json` is self-describing: it pins the exact module digest, the set of
`verified` claim labels, the ordered event log, the obligation states, the contradiction
witness ids, the invalidation frontier for a named change, and the replay digest. The
reference runner (`axiom conform`) loads these and checks them.

## 2. Fixture classes

The suite covers:

* **valid modules** — type-check, execute, and reach expected verification states.
* **invalid modules** — rejected with a precise diagnostic (parse error, type error,
  silent-verify attempt, capability denial, receipt tampering, obligation-not-discharged).
* **expected canonical encodings** — pinned digests for deterministically encoded modules.
* **expected hashes** — semantic ids, node ids, module digests.
* **expected execution events** — the deterministic event log.
* **expected obligation states** — each obligation's final `state`.
* **expected contradiction witnesses** — kind, claims, witness summary.
* **expected invalidation sets** — the exact frontier for a named change.
* **expected replay outputs** — identical digest under receipt-only replay.

## 3. The runner

```
axiom conform [--fixtures <dir>] [--json]
```

The runner:

1. For each valid fixture: parse → execute → compare digest, verified set, events,
   obligations, contradictions against `*.expect.json`. For replay fixtures: execute live,
   capture receipts, execute again in replay mode, assert identical digest.
2. For each invalid fixture: assert the execution fails with the expected error class and,
   where applicable, the expected span.
3. For each encoding vector: assert `canonical_bytes`/`content_id` match the pinned value.
4. Emit a report: pass/fail per fixture, with the first mismatch detail. Exit code is
   non-zero on any failure.

## 4. Independent implementation contract

Another implementation reproduces the corpus by:

* reading `*.axiom` source (the grammar is in [`language.md`](language.md)),
* producing the same canonical bytes (the rules are in [`encoding.md`](encoding.md)),
* executing the transition calculus (the judgments are in [`semantics.md`](semantics.md)),
* and comparing its digest/events/obligations to the pinned `*.expect.json`.

No Rust-specific knowledge is required; the fixtures are plain text plus JSON. A mismatch
means either the implementation or the fixture is wrong, and the standard treats a fixture
that contradicts the spec as a spec bug to be fixed.

## 5. Properties asserted by the suite

The conformance suite pins the 15 required properties (see the final report), including:

* a verified derived claim always has a complete derivation,
* a claim with failed mandatory obligations cannot be verified,
* replay with identical receipts produces identical semantic output,
* removing a required premise invalidates all dependent conclusions,
* removing an unrelated premise does not invalidate independent conclusions,
* contradictions in one context do not corrupt unrelated contexts,
* canonical serialization is stable,
* formatting is idempotent,
* semantic hashes do not depend on irrelevant source formatting,
* unsupported uncertainty combinations are rejected,
* untrusted modules cannot invoke undeclared capabilities,
* receipt modification is detected,
* dependency propagation is deterministic,
* independent execution order does not alter pure deterministic results,
* no unsupported assertion can silently transition into verified state.
