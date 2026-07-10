# Axiom IR

Axiom IR is a universal, proof-carrying intermediate representation for machine reasoning.
It is a portable, executable, verifiable, serializable, and explainable format for reasoning
programs — an IR for machine reasoning in the same way that LLVM is an IR for compilers. A
module is a typed graph of claims, evidence, assumptions, contexts, derivations, proof
obligations, contradictions, and receipts, executed deterministically and incrementally
recomputable when premises change.

## A small example

A module begins with a header `module <name> "<version>"`, then a sequence of instructions.
Each instruction is one line: `evidence`, `assert`, `observe`, `assume`, `derive`, `require`,
`discharge`, `verify`, `contradict`, `branch`, `merge`, `invalidate`, `attest`, `call`.

```axiom
module demo "1"

evidence e_force "text/plain" "force = 10 N"
evidence e_dist  "text/plain" "dist = 2 m"

observe force = q(10 "N") : quantity evidence [e_force]
observe dist  = q(2 "m")  : quantity evidence [e_dist]
assume  coeff = q(0.5 "1") : quantity scope "adj_factor"

derive work = qmul(force, dist) : quantity
verify work

derive adj = qmul(work, coeff) : quantity
verify adj
```

Running it verifies the derived claims (each has a complete derivation and no undischarged
mandatory obligation):

```
$ axiom verify demo.axiom
module 'demo' verification
verified: work, adj
derived-but-not-verified:
```

### Rejected invalid reasoning

A dimensional mismatch is rejected at derive time — it is never silently "verified":

```axiom
module bad "1"
observe force = q(10 "N") : quantity
observe time  = q(3 "s")  : quantity
derive bad = qadd(force, time) : quantity   // N + s is not a quantity
```

```
$ axiom run bad.axiom
execution error in bad.axiom: core error: executor error: unit mismatch: quantity units incompatible: N vs s
```

### Changed premise + selective invalidation

`axiom invalidate <module> <node>` invalidates a node and runs the dependency-aware engine.
Only claims transitively depending on the changed node are invalidated; independent conclusions
are preserved (the `indep` claim below is untouched).

```
$ axiom invalidate examples/incremental.axiom base
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

### Offline replay from receipts

External operations are capability-gated. When granted at run time, the runtime captures an
immutable receipt. Replay reconstructs the external output from that receipt **without** the
live capability, and reproduces an identical module digest. The end-to-end guarantee is
demonstrated by `axiom demo 3`.

```
$ axiom run --cap tool:calculator --builtin-tool examples/external_tool.axiom \
    --emit-receipts receipts.json
$ axiom replay receipts.json
  live digest:    module.1.d3274a22...
  replay digest:  module.1.d3274a22...
  identical:     true
  receipts used: 1
```

> Note: `axiom replay` reconstructs a module from a receipt log whose `source` field carries the
> module source (so live execution is unnecessary). The `demo 3` subcommand exercises this
> identical-digest replay — including rejection of a tampered receipt — end to end.

### Structural explanation

`axiom explain` reports why a claim exists — its operation, inputs, evidence, assumptions, and
obligations:

```
$ axiom explain examples/typed_math.axiom work
claim 'work' (claim.1....)
  status:   verified
  type:     quantity
  value:    20 N*m
  derived by: core.qmul@1
    inputs: force, dist
  obligations: type-compat[mandatory]=Satisfied, dimensional-consistency[mandatory]=Satisfied
  evidence: e_force, e_dist
```

## Features

- **Proof-carrying**: every `verified` claim has a complete, content-addressed derivation record
  and every mandatory proof obligation is a first-class node in state `satisfied` (or `waived`).
- **Obligation-gated verification**: verification is a transition reachable only through the
  discharge of addressable proof obligations (INV-VERIFY).
- **Typed exact arithmetic**: integer, reduced rational, and fixed-scale decimal; floating point
  is forbidden in the normative numeric path. Quantities carry units and reject dimensional
  mismatches.
- **Typed uncertainty algebra**: `exact`, `unknown`, `conflicting`, `probability`, `numeric`,
  `weight`, and externally-asserted uncertainty — with no implicit conversion.
- **First-class contexts**: persistent, tree-structured reasoning environments with explicit,
  merge-conflict-aware semantics; a claim may be verified in one context and contradicted in
  another.
- **Contradiction witnesses**: explicit, preserved witnesses (not erasure) for disjoint intervals,
  incompatible equality, incompatible units, and more.
- **Deterministic runtime**: identical inputs, operations, capabilities, evidence, and receipts
  always produce an identical module digest and event log.
- **Dependency-aware incremental invalidation**: removing or changing a node invalidates exactly
  its represented dependents (INV-MINIMALITY) and recomputes only what is affected.
- **Offline replay**: external outputs are reconstructed from integrity-checked receipts without
  re-invoking the external operation.
- **Canonical encoding + domain-separated hashing**: stable content-addressed identities;
  claim/evidence/context/derivation/obligation/receipt/module ids never collide.
- **Explainable**: `explain` and `trace` expose the full provenance of any claim.
- **Hostile-model posture**: a default-deny `ReplayOnlyExecutor`; external capabilities are
  explicitly granted, never assumed.

## Quick start

```sh
# Build and run the full test suite
cargo test --workspace

# Run all seven required demonstrations (typed math, contradictions, replay, …)
cargo run --release -p axiom-cli -- demo all

# Run the conformance suite against the fixtures in conformance/
cargo run --release -p axiom-cli -- conform

# Environment and self-test
cargo run --release -p axiom-cli -- doctor

# Optional: benchmarks
cargo bench -p axiom-benches
```

## CLI

The `axiom` binary provides the following subcommands:

| Command | Description |
|---|---|
| `check` | Parse and type-check a module, reporting diagnostics. |
| `fmt` | Format a module (canonical printer); `--check` verifies stability. |
| `run` | Execute a module and report verification + digest. |
| `verify` | Report which claims verify and which remain blocked. |
| `explain` | Explain why a claim exists (operation, evidence, assumptions, obligations). |
| `trace` | Print the provenance chain of a claim. |
| `contradictions` | List contradiction witnesses. |
| `invalidate` | Invalidate a node and run the incremental engine. |
| `replay` | Replay a module from a receipt log (no live capabilities required). |
| `diff` | Structural diff between two executed modules. |
| `graph` | Emit the dependency graph as Graphviz DOT. |
| `inspect` | Inspect a node by label. |
| `conform` | Run the conformance suite. |
| `doctor` | Environment and self-test. |
| `demo` | Run a demonstration (`1`..`7` or `all`). |

Global flag: `--json` emits structured JSON instead of human-readable text.

## Repository layout

| Crate | Purpose |
|---|---|
| `crates/axiom-types` | Value/type system, exact arithmetic, uncertainty algebra. |
| `crates/axiom-encoding` | Canonical encoding and domain-separated content addressing. |
| `crates/axiom-core` | Module model, transition calculus, operation registry, receipts. |
| `crates/axiom-parser` | Textual language parser and canonical formatter. |
| `crates/axiom-runtime` | Deterministic executor, obligation engine, contradiction detection, receipts. |
| `crates/axiom-incremental` | Dependency-aware incremental invalidation engine. |
| `crates/axiom-conformance` | Conformance runner and fixtures (`conformance/`). |
| `crates/axiom-cli` | Command-line interface (`axiom`). |
| `crates/axiom-sdk` | Rust SDK for embedding Axiom IR. |
| `benches` | Criterion benchmarks (workspace member `axiom-benches`). |
| `fuzz` | Cargo-fuzz harness (excluded from the workspace; needs nightly). |
| `examples/` | Small, self-contained example modules (see `examples/README.md`). |
| `conformance/` | Valid and invalid conformance fixtures. |
| `spec/` | Normative semantics, encoding, and threat model. |
| `docs/` | Architecture, research, and security notes. |

A Python bridge, the benchmarking harness, and the fuzz harness are additional surfaces; see
`docs/` for details.

## Further reading

- `spec/semantics.md` — normative transition calculus, the central invariant (INV-VERIFY), and
  the minimality invariant (INV-MINIMALITY).
- `spec/encoding.md` — canonical encoding and domain-separated hashing.
- `docs/research/novelty-audit.md` — the precise novelty contribution and what is *not* claimed.
- `docs/security/threat-model.md` — the hostile-model assumption and capability posture.
- `CONTRIBUTING.md` — how to build, test, and extend Axiom IR.
- `LICENSE` — Apache License 2.0.
