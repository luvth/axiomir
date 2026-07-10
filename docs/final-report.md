# Axiom IR — First Credible Release: Final Report

*Date: 2026-07-10. Status: all acceptance gates satisfied; repository committed; git clean.*

---

## 1. One-sentence definition

Axiom IR is a universal, proof-carrying intermediate representation and deterministic runtime for machine reasoning, in which a model emits an *executable, verifiable, replayable, incrementally recomputable* module of claims rather than a disposable natural-language trace.

## 2. Exact novel technical contribution

A **typed reasoning transition calculus** in which *verified* is a status reachable only through the discharge of first-class, content-addressed **proof-obligation** nodes; where **contradiction is a preserved first-class witness** (never erasure); where **contexts are first-class persistent environments** with explicit, merge-conflict-aware semantics; and where the entire verified state is **reconstructible by deterministic replay from content-addressed external receipts** — yielding a **dependency-correct incremental-invalidation engine** that invalidates exactly the represented dependent region and nothing else.

The sharp primitive: *a verified claim is a node whose only valid producers are derivation+receipt transitions in which every mandatory proof obligation is a first-class node in state `satisfied` (or `waived` under a spec-defined weaker class), and whose complete derivation record is itself a content-addressed, replayable object.* This is distinguishable from a knowledge graph (no obligation-gated verification, no contradiction witnesses, no receipt replay), a proof assistant (no proof *term* is required; the proof is the executed, receipted derivation plus discharged obligations), Datalog (no verified/obligation gating, no uncertainty, no preserved contradictions, no replay), a workflow engine (derives *claims* and gates *verification*), and a provenance DAG (no verified state, no contradiction witnesses, no context-local validity). Full defense in `docs/research/novelty-audit.md`.

## 3. Semantic model

A module state is `M = (C, E, A, X, D, O, U, R)` — claims, evidence, assumptions, contexts, derivations, obligations, uncertainty states, receipts — plus operations, contradictions, and an append-only event log. Each first-class object has a clear execution role:

- **Claim**: typed proposition with stable semantic identity (`semantic_id` = hash of type+value, context/status-independent) and node identity (`claim_node_id` = hash of label+context). Distinguishes semantic identity, node identity, source span, and human label.
- **Evidence**: immutable, addressable support with content hash, media/schema type, provenance, acquisition metadata, trust class, optional signature. Existence ≠ relevance; attaching evidence never verifies.
- **Assumption**: independently addressable, scoped, challengeable, removable, traceable through all dependent claims.
- **Context**: persistent environment = assumption set + inherited + local claims + contradiction state + parent link. Supports branch, inheritance, comparison, controlled merge with conflict detection, branch-local invalidation.
- **Operation**: versioned, declares id/version/input types/output type/determinism class/uncertainty rule/obligation kinds/capabilities/side-effect class.
- **Derivation**: operation identity+version, input hashes, output claim, generated obligations, receipt, provenance, runtime version.
- **Obligation**: first-class node with kind, severity (mandatory/advisory), state (pending/satisfied/failed/waived/unsupported). Undischarged mandatory obligation blocks verification.
- **Contradiction**: first-class event with kind, participating claims, context, inspectable/serializable witness.
- **Uncertainty**: typed algebra value (exact/unknown/probability-interval/numeric-interval/evidence-weight/conflicting/externally-asserted/extension), with forbidden implicit conversion.

## 4. Formal invariants

Central invariant (verified execution): *No derived claim may enter `Verified` unless it has a complete derivation record and every mandatory proof obligation is `Satisfied` (or `Waived` under a spec-defined weaker class).* Unsupported assertions live only in explicitly unverified states (asserted/assumed/observed/pending/disputed/challenged/externally-attested) and never silently become `Verified`.

The reference runtime enforces, and `Module::check_verification_invariant` re-validates after every execution:

- **INV-DERIVATION**: every `Verified` claim has a complete `Derivation` and (for external ops) a valid receipt.
- **INV-OBLIGATION**: every `Verified` claim has all mandatory obligations discharged.
- **INV-RECEIPT**: external derivations require an integrity-checked receipt (`Receipt::verify_integrity`).
- **INV-MINIMALITY**: invalidation dirties exactly the transitive dependency frontier.
- **INV-DETERMINISM**: identical canonical input + identical op versions + identical receipts + identical capabilities + identical runtime version ⇒ identical claim/derivation/obligation/event/digest output.
- **INV-DOMAIN**: domain-separated hashing prevents cross-category identity collisions.

Proof sketches for these are in `spec/semantics.md` (verification-completeness by construction of the `verify` transition; determinism by receipt-sealing + canonical encoding; invalidation minimality by graph reachability; context isolation by per-context claim/derivation sets).

## 5. Trusted computing base

Pure, I/O-free crates constitute the auditable TCB: `axiom-core` (transition calculus, `verify`, `check_verification_invariant`, `Receipt::verify_integrity`), `axiom-encoding` (canonical JSON, domain-separated `content_id`/`hash_domain`), `axiom-types` (value/number/unit/uncertainty). All side effects enter only as receipts or an injected `ExternalExecutor` + explicit capability set. Default runtime installs `ReplayOnlyExecutor` and grants **no** capabilities.

## 6. Architecture implemented

Rust workspace (preferred for precise data models, deterministic behavior, safe concurrency, robust parsers, content addressing, incremental computation, portable binaries, embeddable library, bindings). Crates:

- `axiom-types` — exact arithmetic (`Num`), units, typed value model, uncertainty algebra.
- `axiom-encoding` — canonical JSON, domain-separated content addressing.
- `axiom-core` — semantic objects, transition calculus, verification invariant, receipt integrity.
- `axiom-parser` — lexer, recursive-descent parser, typed AST, diagnostics, idempotent canonical formatter.
- `axiom-runtime` — transactional deterministic execution, operation registry, obligation engine, contradiction detection, context engine, external receipts, replay, explanation.
- `axiom-incremental` — forward/reverse dependency indexes, invalidation frontier, recomputation, status-change report.
- `axiom-conformance` — fixture corpus + runner.
- `axiom-cli` — developer CLI.
- `axiom-sdk` — fluent Rust builder facade.
- `bindings/python` — honest CLI bridge (no reimplemented semantics).
- `benches/`, `fuzz/`, `spec/`, `docs/`, `conformance/`, `examples/`.

## 7. Textual language

Line-oriented, readable, compact, deterministic, unambiguous, parseable without model heuristics, source-control friendly. One instruction per line: `evidence, assert, observe, assume, derive, require, discharge, verify, challenge, contradict, branch, merge, invalidate, attest, call`. Supports declarations, typed values, evidence references, assumptions, contexts, derivations, operations, obligations, external calls, queries. Full lexer/parser/typed-AST/semantic-analysis/diagnostics/formatter/canonical-printer with formatter idempotence and round-trip tests. Reference in `spec/language.md`.

Example:
```
module demo1 "1"
evidence e_force "text/plain" "force=10N"
observe force = q(10 "N") : quantity evidence [e_force]
assume coeff = q(0.5 "1") : quantity scope "adj_factor"
derive work = qmul(force, dist) : quantity
verify work
```

## 8. Canonical encoding

Provider-neutral canonical JSON: recursively sorted object keys, no insignificant whitespace, stable integer/string formatting. `Num` serialized as exact string form (no IEEE-754). Domain-separated content addressing: `Id = domain || 0x00 || SHA-256(canonical_bytes)`. Twelve domains (Claim, Evidence, Assumption, Context, Derivation, Obligation, Contradiction, Receipt, Module, Operation, Event, Extension) so a claim digest can never equal an evidence digest of identical content. Same semantic module ⇒ same canonical bytes ⇒ same digest across runs. Test vectors with source, canonical representation, digests, and expected execution results in `spec/encoding.md` and the conformance corpus.

## 9. Deterministic replay mechanism

External operations are capability-gated and never treated as pure. On a live call, the runtime invokes the executor, captures inputs/output/schema/provider/logical-time/op-version into a signed-by-hash `Receipt`, and stores it. In replay mode (no capabilities), the external `call` resolves the output purely from the matching, integrity-checked receipt; no live side effect occurs. Receipt integrity is recomputed from recorded fields and compared to the stored digest, so any tampering is detected. Replay reproduces identical claim identities, derivation identities, obligation states, contradiction witnesses, event log, exported results, and final module digest. Demo 3 and the `replay` conformance fixture assert live-digest == replay-digest.

## 10. Incremental invalidation algorithm

1. Build forward+reverse dependency indexes from `Claim::depends_on`.
2. BFS the transitive dependents of the changed node (the invalidation frontier).
3. Remove the changed *premise* (asserted/observed/assumed, no derivation) from the affected set — it is the corrected input and stays valid.
4. Invalidate every affected claim (status → `Invalidated`, append `Invalidation` event + `invalidation_conditions`).
5. Recompute derived claims whose inputs are available, in topological worklist order to fixpoint; re-verify where obligations discharge.
6. Produce a `StatusChange` report (every transition) and the preserved set.

Proven by tests: premise change invalidates exactly the dependent chain; unrelated verified work is preserved; middle-node change recomputes only downstream; incremental result matches a fresh full execution logically. Benchmarks compare incremental vs full re-execution.

## 11. Contradiction and context semantics

Contradiction is a first-class event with kind (proposition-negation, incompatible-equality, disjoint-interval, incompatible-unit, mutually-exclusive-membership, violated-postcondition, evidence-conflict, assumption-conflict, extension) and an inspectable, serializable witness. It **never erases either branch** — both claims, derivations, contexts, evidence, and the exact witness are preserved. Contexts are first-class persistent environments with parent links and explicit merge semantics; a claim may be verified in one context and contradicted in another. Controlled merge reports merge conflicts (same `semantic_id`, different value) rather than silently unifying. Context isolation is tested (context-local verification while a global conflict remains).

## 12. Uncertainty model

Typed uncertainty algebra with explicit legal operations, conversion rules, composition rules, loss-of-information, and rejection conditions per type. Implicit conversion between incompatible models is **forbidden** (`Uncertainty::combine` returns `UncertaintyError::Incompatible`). Axiom fails explicitly rather than manufacturing fake precision (e.g., a probability-interval cannot silently become a numeric-interval). Initial types: exact, unknown, probability-interval, numeric-interval, evidence-weight, conflicting, externally-asserted, extension. Reference `spec/uncertainty.md`.

## 13. Security model

Assumes all model-generated modules are hostile. Layered defense: pure I/O-free TCB; syntactic parser boundary; capability-gated, default-deny external execution (default `ReplayOnlyExecutor`, no capabilities); domain-separated content addressing (hash confusion impossible); receipt integrity (forged/tampered receipts detected); capability escalation impossible (capabilities only via `Runtime::grant`); no FS/shell executor (path traversal / command injection unreachable). Hardening applied this release:
- **Parser recursion-depth guard** (`MAX_EXPR_DEPTH = 256`) — deeply nested values fail closed with a diagnostic instead of overflowing the stack.
- **Module statement cap** (`MAX_MODULE_STMTS = 100_000` in `Runtime::execute`) — oversized modules fail closed.

Threat catalog and honest residual risks in `docs/security/threat-model.md`.

## 14. Conformance strategy

`axiom conform` runs the reference implementation against a corpus of valid + invalid fixtures, each paired with an `.expect.json` describing expected verified set, obligation states, contradiction count, invalidation frontier, replay equality, error substrings, or pinned digest. Fixtures are plain text + JSON, so an independent implementation reproduces the same outcomes without reading Rust. Current corpus: 15 fixtures (10 valid, 5 invalid), all passing. The runner reports pass/fail per fixture with detail.

## 15. Demonstrations implemented

1. **Typed mathematical reasoning** — exact/decimal arithmetic, quantities with units, intervals, assumptions, postconditions, obligations; successful verification; unit-mismatch rejection; assumption dependency; incremental recomputation after a numeric premise change.
2. **Contradictory evidence** — two incompatible observations; contradiction detection; explicit witness; both branches preserved; context-local verification while global conflict remains.
3. **External receipt and offline replay** — capability declaration, execution, immutable receipt capture, verification, replay with external access disabled, identical final digest; tampered receipt rejected.
4. **Model-produced invalid reasoning** — deterministic mock/structured fixtures emitting a module with missing premise, invalid type, undischarged obligation, fabricated evidence reference; rejected or quarantined while valid portions preserved.
5. **Incremental repair** — verified module; one evidence/assumption change; exact dirty frontier; selective invalidation; preservation of unrelated verified claims; targeted recomputation; structured explanation of every status transition.
6. **Competing model outputs** — two independently produced modules reaching different conclusions; structural diff; shared premises; divergent assumptions/operations/evidence; earliest semantic divergence point.
7. **Independent-standard behavior** — formatting does not alter semantic identity; declaration order does not alter identity where order is semantically irrelevant; canonical encoding stable; replay stable; unsupported extensions fail explicitly.

All seven pass (`axiom demo all` → `ALL DEMOS: PASS`).

## 16. Build commands

```
cargo build --workspace                 # debug build
cargo build --release -p axiom-cli      # release CLI
```

## 17. Test commands

```
cargo test --workspace                   # full suite (64 tests, 0 failures)
cargo test -p axiom-parser              # parser/lexer/formatter/round-trip
cargo test -p axiom-core                # calculus + properties
cargo test -p axiom-runtime             # execution, replay, capability, contradiction
cargo test -p axiom-incremental         # invalidation minimality/exactness
cargo test -p axiom-conformance         # runner test
cargo clippy --workspace --all-targets  # 0 warnings
cargo fmt --all --check                 # clean
```

## 18. Demo commands

```
cargo run --release -p axiom-cli -- demo all     # all 7 demos
cargo run -p axiom-cli -- demo 1                 # single demo
```

## 19. Benchmark commands

```
cargo bench -p axiom-benches                     # full criterion suite
cargo bench -p axiom-benches --bench parse -- --test   # smoke (run once)
```

Honest representative numbers (macOS, Apple Silicon, Rust 1.96.1, thin LTO):
- `parse_small`: 787 ns
- `parse_chain/1000`: 312 µs
- `invalidate_chain/1000`: 50.6 ms
- `invalidate_wide/1000`: 13.5 ms
- full re-execution `shared_64x32`: 21.0 ms

## 20. Test and conformance results

- **64 unit/integration/property tests pass, 0 fail.**
- **Conformance: 15/15 fixtures passed.**
- **Clippy: 0 warnings/errors. rustfmt: clean.**
- **Python bridge: 5/5 tests pass** (drives the real CLI).
- All 15 required invariant properties (verified-derivation-complete, obligation-gates-verify, replay-determinism, premise-invalidation, unrelated-preservation, contradiction-isolation, canonical-stability, formatter-idempotence, hash-independence-from-formatting, unsupported-uncertainty-rejected, capability-denial, receipt-tamper-detection, dependency-determinism, order-independence, no-silent-verify) have explicit executable tests.

## 21. Representative replay proof output

From Demo 3 / the `replay` conformance fixture: a live run with capability + builtin tool produces receipt `r`, then an offline run with `replay_mode(receipts)` and **no capability** reconstructs the module. Asserted: `digest_live == digest_replay`. Tampering with `r.output` makes `Receipt::verify_integrity()` return false and replay fails (`ReplayMissingReceipt` / integrity error). README §"Offline replay from receipts" shows the concrete `axiom replay` output with equal live/replay digests.

## 22. Representative invalidation proof output

From README §"Changed premise + selective invalidation" (and Demo 5):
```
invalidation frontier rooted at 'base' (claim.1....)
  invalidated: a, b
  recomputed:  a, b
  preserved:   1 verified claims untouched
  status changes:
    b: verified -> invalidated (transitive dependency on base changed)
    a: verified -> invalidated (transitive dependency on base changed)
    a: invalidated -> verified (recomputed from available inputs)
    b: invalidated -> verified (recomputed from available inputs)
```
The corrected premise `base` itself stays valid; only its represented dependents change.

## 23. Examples of invalid reasoning rejected

- **Fabricated evidence reference** (`assert x = 1 : rational evidence [ghost]`) → `UnknownEvidence`, rejected.
- **Missing/undeclared premise** (`derive orphan = add(missing, other)`) → `UnknownLabel`, rejected.
- **Undischarged mandatory obligation** (`derive out = add(a,b) ; require numeric-bounds on out ; verify out`) → verification fails; claim quarantined, never `Verified`.
- **Unit mismatch** (`derive bad = qadd(force_N, time_s)`) → dimensional-consistency obligation fails, rejected/not verified.
- **Unregistered extension operation** (`derive out = vendor.widget(a)`) → `UnknownOperation`, explicitly rejected (no silent substitution).

## 24. Honest limitations

1. **Evidence is self-attested** — runtime checks existence, not authenticity; trust must be supplied by the host.
2. **Signatures are not validated** — `Receipt::signature`/`Evidence::signature` are carried but unenforced; integrity rests on the receipt hash.
3. **Automatic contradiction detection is opt-in** — `Runtime::detect_contradictions(ctx)` is a public method, not part of default execution.
4. **No fixed-point semantics for semantic cycles** — cyclic modules are rejected (by design) rather than resolved; acyclic dependency graphs only.
5. **No cross-platform floating-point determinism claim** — the normative numeric path is exact (integer/rational/decimal); floats are forbidden there.
6. **Uncertainty is a bounded initial algebra** — richer composition (e.g., correlated probability intervals) is future work.
7. **Fuzz targets require the `cargo-fuzz` nightly component** (not installed in this environment); the targets are in place and the parser/runtime have resource caps, but continuous fuzzing was not executed here.

None of these allow a hostile module to *silently* verify invalid reasoning or perform an unauthorized side effect.

## 25. Three strongest next research milestones

1. **Semantic equivalence and canonicalization beyond structural hash** — define alpha-equivalence / contextual equivalence so two modules that "say the same thing" under renaming/context collapse to one semantic identity, enabling real module diffing and deduplication.
2. **Receipt authenticity and evidence trust** — a signature-verification path binding receipts/evidence to trusted issuers (closing limitations 1–2) without compromising replay determinism.
3. **Fixed-point semantics for cyclic/recursive reasoning** — define well-founded or stratified fixpoint evaluation so legitimate mutually-recursive derivations execute rather than being rejected, with soundness proofs.

## 26. Major design decisions and discarded alternatives

- **Exact arithmetic over IEEE-754** — chosen for deterministic replay; floats explicitly excluded from the normative path.
- **Obligation-gated verification over a boolean "trusted" flag** — verification is a computed transition, not an asserted property; this is the central novelty.
- **Content-addressed, domain-separated identities over sequential integer ids** — enables replay, deduplication, and collision resistance.
- **Contradiction-as-witness over erasure/negation-as-failure** — preserves both branches for audit and context-local resolution.
- **Receipt-sealed replay over re-invocation** — external results become immutable, replayable objects; no live side effect in replay.
- **Hybrid worklist execution over strict topological sort** — declaration order becomes semantically irrelevant and forward references legal.
- **Discarded: a generic DAG executor, a JSON-schema-with-extras, a notebook format, a workflow engine, a theorem-prover front-end** — each fails the novelty/portability bar (see novelty audit).

## 27. Files and crates created

Crates: `axiom-types, axiom-encoding, axiom-core, axiom-parser, axiom-runtime, axiom-incremental, axiom-conformance, axiom-cli, axiom-sdk`. Support: `bindings/python` (CLI bridge + tests), `benches/` (13 criterion benches), `fuzz/` (parse + runtime targets), `conformance/` (15 fixtures), `examples/` (5 modules), `spec/` (12 normative docs, 1612 lines), `docs/` (architecture, deterministic-replay, explanation-model, incremental-invalidation, sdk-guide, python-guide, security/threat-model, research/novelty-audit, research/limitations, research/roadmap — 3272 lines), plus README/LICENSE/CHANGELOG/CONTRIBUTING/SECURITY/CODE_OF_CONDUCT/.CI workflow at `.github/workflows/ci.yml`.

## 28. Commit history summary

Nine coherent, staged commits on `master` (no remote push; environment not authorized):
1. `chore: workspace scaffolding, tooling, and repository metadata`
2. `feat(core): semantic foundation — typed values, canonical encoding, transition calculus`
3. `feat(parser): textual Axiom language — lexer, recursive-descent parser, canonical formatter` *(+ recursion-depth security hardening)*
4. `feat(runtime): deterministic execution, obligations, contexts, contradictions, receipts, replay` *(+ module size-cap security hardening)*
5. `feat(incremental): dependency-correct invalidation and recomputation engine`
6. `feat(cli+conformance): developer CLI, conformance corpus and runner, examples`
7. `feat(sdk+bindings+benches+fuzz): Rust SDK, Python bridge, benchmarks, fuzz targets`
8. `docs: normative specification, architecture, and research record`
9. `chore: commit Cargo.lock for reproducible builds`

## 29. Final Git status

Clean. `git status --short` is empty (working tree committed; `.DS_Store` and `target/` excluded via `.gitignore`; `Cargo.lock` tracked for reproducibility). 64 tests pass, 15/15 conformance, clippy clean, rustfmt clean, 7/7 demos pass, Python bridge 5/5 pass.
