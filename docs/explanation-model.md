# The Explanation Model

In Axiom IR, an explanation is **derived from graph semantics, never generated prose**. The
runtime answers explanation queries by reading the first-class nodes of the module (`Claim`,
`Derivation`, `Obligation`, `Assumption`, `Evidence`, `Contradiction`, `Receipt`) and the
dependency edges produced by `Claim::depends_on`. There is no natural-language model
involved; the output is structured data that a human reader or a downstream tool consumes
verbatim.

The engine is `axiom_runtime::Runtime::explain(label)`, which returns an `Explanation`
struct (`crates/axiom-runtime/src/explain.rs`). The CLI exposes it as `axiom explain
<claim>` and the related provenance walk as `axiom trace <claim>`.

## The queries the runtime answers

Each query maps to a field of `Explanation` or to a companion method. `explain` populates:

| Explanation query | Source in the model |
|---|---|
| Why does this claim exist? | `status` + presence of `derivation`. No derivation ⇒ asserted/observed/assumed (a premise); a derivation ⇒ produced by an operation. |
| Which evidence supports it? | `evidence: Vec<Id>` (the claim's `Claim::evidence`). |
| Which assumptions it requires? | `assumptions: Vec<Id>` (the claim's `Claim::assumptions`). |
| Which operation produced it? | `derivation.op` + `derivation.op_version` (the `OperationDef` name/version). |
| Which obligations were discharged? | `obligations: Vec<ObligationInfo>` — each with `kind`, `severity` (`mandatory`/`advisory`), and `state` (`Pending`/`Satisfied`/`Failed`/`Waived`/`Unsupported`). |
| In which context is it valid? | `context: Id` (the `Claim::context_id`); contexts form a tree so validity is per-claim, not global. |
| What contradicts it? | `contradicts: Vec<Id>` — every `Contradiction` whose `claims` contains this id. |
| What would invalidate it? | `invalidation_conditions: Vec<String>` on the `Claim` (carried by `Explanation`); the incremental engine appends `"dependency <id> changed"` entries here when a dependency changes. |
| Which external calls contributed? | `derivation.receipt` — when non-`None`, the claim's value was reconstructed from a `Receipt` (whose `operation`/`provider` identify the external call). |

Two further queries are answered by companion machinery rather than a single `explain`
call:

- **Why did its status change?** `Runtime::provenance_diff(a, b)` returns the differing
  fields (`value`, `type`, `assumptions`, `evidence`) between two claims, isolating what
  moved a claim from one state to another. The CLI surfaces the broader view via
  `axiom diff <a> <b>`.
- **What changed between two executions?** `axiom diff <module-a> <module-b>` executes both
  modules and reports claims `only in A`, `only in B`, and `changed` (label + status/value
  transitions), plus both digests. This is the structural, graph-derived answer to "what
  changed", again with no prose generation.

## `axiom explain <claim>`

Human output lists each field; the `--json` output is the structured `Explanation` (minus
`invalidation_conditions`, which the CLI prints only in human form):

```json
{
  "label": "sum",
  "status": "verified",
  "type": "rational",
  "value": "5",
  "context": "context.1.<hex64>",
  "derivation": {
    "op": "core.add",
    "version": "1",
    "inputs": ["claim.1.<hex64>", "claim.1.<hex64>"]
  },
  "obligations": [
    { "kind": "type-compat", "severity": "mandatory", "state": "Satisfied" }
  ],
  "assumptions": [],
  "evidence": ["evidence.1.<hex64>"],
  "contradicts": []
}
```

Notes on the shape (verified against `crates/axiom-cli/src/commands.rs`):

- `derivation` is `null` for asserted/observed/assumed claims (no `Derivation`); for derived
  claims it carries `op`, `version`, and `inputs` (each an `Id` string).
- `obligations[].state` is the Rust `Debug` form of `ObligationState` (`Satisfied`,
  `Pending`, …); `severity` is the lower-cased `mandatory`/`advisory`.
- `evidence` and `assumptions` are `Id` string arrays; `contradicts` lists the ids of
  contradiction witnesses that reference this claim.

## `axiom trace <claim>`

`trace` walks the provenance chain recursively (`crates/axiom-runtime/src/commands.rs`:
`trace_node`): for the target claim it prints `<label> = <value> : <type> [<status>]`, then
`<- <op>@<version>` for its derivation, then recurses into each input claim indented two
spaces deeper. Unlike `explain`, the JSON is deliberately textual — a single `trace`
string built from those indented lines:

```json
{
  "root": "sum",
  "trace": "sum = 5 : rational [verified]\n  <- core.add@1\n    a = 2 : rational [asserted]\n    b = 3 : rational [asserted]"
}
```

The `trace` string is the same tree rendered for the human reader, so tooling and humans
see identical structure. Use `explain` when you want the flat, field-addressable record;
use `trace` when you want the full ancestor chain in one block.

## Contradiction witnesses vs. active detection

Two related methods answer "what contradicts this claim?":

- `Runtime::explain(label).contradicts` (and the CLI `axiom explain`) returns the
  contradiction witnesses **already recorded** in `Module::contradictions` that reference
  the claim. This is a pure read.
- `Runtime::detect_contradictions(ctx_label)` actively scans a context's `local_claims`
  pairwise and records new `Contradiction` nodes (e.g. `PropositionNegation` for
  unequal booleans, `DisjointInterval` for non-overlapping numeric intervals,
  `IncompatibleUnit` for equal magnitude with incompatible units). `axiom contradictions`
  lists the recorded witnesses; to populate them you run a module whose source emits
  `contradict` instructions, or call `detect_contradictions` programmatically.

## Using explanations from Rust

The SDK re-exports `explain` on the builder: `Builder::explain(label)` returns the same
`axiom_runtime::Explanation` struct used by the CLI, so Rust programs get the full
field-addressable record (including `invalidation_conditions`, which the CLI JSON omits):

```rust
use axiom_sdk::{Builder, Uncertainty};
let mut b = Builder::new("demo");
let a = b.assert_int("a", 2, Uncertainty::Exact);
let c = b.assert_int("c", 3, Uncertainty::Exact);
let out = b.derive_rational("out", "core.add", &[a, c]);
b.verify("out").unwrap();
let e = b.explain("out").unwrap();
assert_eq!(e.status, axiom_core::ClaimStatus::Verified);
assert!(e.derivation.is_some());
```

## Design intent

Because explanations are pure reads over the first-class graph, they are as trustworthy as
the module itself: an explanation cannot assert something the nodes do not support, and it
cannot be "wrong" in the way a generated summary can. This is consistent with the limits in
`spec/semantics.md` §7 — Axiom *represents* support, contradiction, and obligation state;
it does not synthesize justification. The `Explanation` struct and `trace` output are the
auditable window into that representation.

## Relationship to other docs

- The module state model and node types: `docs/architecture.md`
- How obligations gate `Verified` (the `obligations` field's meaning):
  `docs/architecture.md` and `spec/semantics.md` §2 (INV-VERIFY)
- External-call provenance (`derivation.receipt`): `docs/deterministic-replay.md`
