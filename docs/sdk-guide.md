# Axiom SDK & CLI Guide

This guide shows how to build, run, inspect, and verify Axiom IR modules using
the Rust `axiom-sdk` facade and the `axiom` command-line interface. It assumes
you have the workspace checked out and the `axiom` binary built (`cargo build
--bin axiom`).

Axiom IR is a proof-carrying intermediate representation: every derived claim
carries a complete derivation record, first-class proof obligations, and — for
external operations — an immutable receipt. "Verified" means *obligation-discharged
under recorded receipts and rule versions*, not mathematical proof. See
`docs/research/limitations.md` for the precise scope of that guarantee.

## 1. Building modules with `axiom-sdk` (Rust)

`axiom-sdk` re-exports the core vocabulary (`Module`, `ClaimStatus`, `Uncertainty`,
`Value`, `Type`, `Id`, `ContradictionKind`, `Quantity`, `Unit`, `Num`,
`content_id`, `Domain`, `parse_module`, `format_source`, `run_source`, `Runtime`)
and adds a fluent `Builder`.

### 1.1 A complete, runnable example

```rust
use axiom_sdk::{Builder, Uncertainty};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a builder (this constructs a Runtime internally).
    let mut b = Builder::new("demo");

    // 2. Assert two integer claims. Type::Rational is implied by assert_int.
    let a = b.assert_int("a", 2, Uncertainty::Exact);
    let c = b.assert_int("c", 3, Uncertainty::Exact);

    // 3. Derive `out = a + c` via the builtin pure operation `core.add`.
    //    derive_rational/derive_quantity/derive_bool all use the BuiltinExecutor.
    let out = b.derive_rational("out", "core.add", &[a, c]);

    // 4. Verify the derived claim. Verification failure is NOT fatal: an
    //    unverifiable claim simply stays unverified (quarantined).
    b.verify("out").map_err(|e| format!("verify failed: {e}"))?;

    // 5. Explain the claim structurally (operation, inputs, obligations).
    let explanation = b.explain("out")?;
    if let Some(d) = &explanation.derivation {
        println!("out derived by {}@{}", d.op, d.op_version);
    }

    // 6. Content-addressed module digest (deterministic, version-stable).
    let digest = b.digest();
    println!("module digest: {}", digest);

    // 7. Materialize the runtime for further inspection.
    let rt = b.build();
    println!("verified claims: {}", rt.verified_claims().len());
    Ok(())
}
```

`Builder` methods of note: `assert_int`, `assert_quantity`, `assert_bool`,
`assume_bool`, `derive_rational`, `derive_quantity`, `derive_bool`,
`verify(label)`, `explain(label)`, `digest()`, `verified_claims()`, and
`build() -> Runtime`. All `assert_*`/`derive_*` calls return a content-addressed
`Id` that can be passed as a derivation input.

The runtime also exposes `run_source(src) -> Result<Runtime, RuntimeError>` for
executing a textual module directly, and `with_builtin_tool()` /
`replay_mode(receipts)` for controlling external execution (see the threat model
in `docs/security/threat-model.md`).

### 1.2 The equivalent textual module

The Builder above is exactly equivalent to the following module, which you can
run with the CLI:

```text
module demo "1"
assert a = 2 : rational
assert c = 3 : rational
derive out = core.add(a, c) : rational
verify out
```

Other useful statement forms (verified against `crates/axiom-parser/src/parser.rs`):

```text
assert mass = q(5 "kg") : quantity
derive total = core.qadd(mass, mass) : quantity
observe temp = q(21 "C") : quantity
assume coeff = q(0.5 "1") : quantity scope "adj_factor"

evidence src "text/plain" "raw text" trust unverified
assert claim = true : bool evidence [src]

require numeric-bounds on out
discharge out by src as satisfied

contradict a c as proposition-negation

call sum = tool.calculator(a, c) : rational cap "tool:calculator"
verify sum

invalidate out because "premise retracted"
```

## 2. Inspecting with the `axiom` CLI

Every command accepts a global `--json` flag to emit structured JSON instead of
human-readable text. The commands are: `check`, `fmt`, `run`, `verify`,
`explain`, `trace`, `contradictions`, `invalidate`, `replay`, `diff`, `graph`,
`inspect`, `conform`, `doctor`, `demo`.

### 2.1 `axiom run` — execute and summarize

```sh
axiom run module.axm --json
```

JSON shape (from `module_summary` in `crates/axiom-cli/src/commands.rs`):

```json
{
  "ok": true,
  "name": "demo",
  "digest": "module.1.<hex>",
  "event_log_digest": "event.1.<hex>",
  "claim_count": 3,
  "verified": ["out"],
  "contradiction_count": 0,
  "obligation_count": 1,
  "by_status": { "verified": ["out"], "asserted": ["a", "c"] },
  "claims": [
    { "label": "a", "status": "asserted", "type": "Rational",
      "value": "2", "uncertainty": "exact", "context": "context.1.<hex>" }
  ]
}
```

### 2.2 `axiom verify` — what verifies vs. what is blocked

```sh
axiom verify module.axm --json
```

```json
{
  "ok": true,
  "verified": ["out"],
  "not_verified": ["sum (pending)"],
  "digest": "module.1.<hex>"
}
```

Note that a `verify` statement that cannot be satisfied does **not** abort the
module. The claim stays in its pre-verify state and the failure is recorded in
`Runtime::verify_failures` (visible to tooling) — valid reasoning is preserved.

### 2.3 `axiom explain` — provenance of one claim

```sh
axiom explain module.axm out --json
```

```json
{
  "label": "out",
  "status": "verified",
  "type": "Rational",
  "value": "5",
  "context": "context.1.<hex>",
  "derivation": {
    "op": "core.add",
    "version": "1",
    "inputs": ["claim.1.<hex-a>", "claim.1.<hex-c>"]
  },
  "obligations": [
    { "kind": "type-compat", "severity": "mandatory", "state": "Satisfied" }
  ],
  "assumptions": [],
  "evidence": [],
  "contradicts": []
}
```

Related commands: `axiom trace` (provenance chain as text/JSON), `axiom inspect`
(full node detail), `axiom graph` (Graphviz DOT), `axiom contradictions`
(witness list), and `axiom diff` (structural diff of two executed modules).

## 3. The Python bridge

Non-Rust users can drive the same engine through the Python package under
`bindings/python/`. It is a thin, **honest** wrapper: it shells out to the
compiled `axiom` binary and parses its `--json` output. It performs no Axiom
semantics of its own (pure stdlib, no third-party dependencies).

```python
from axiom import Axiom

ax = Axiom()                      # resolves the `axiom` binary (AXIOM_BIN / PATH / target/*/axiom)
result = ax.run("module.axm")     # parses --json output into a dict
print(result["verified"])         # ["out"]
expl = ax.explain("module.axm", "out")
print(expl["derivation"]["op"])   # "core.add"
```

The client never trusts its own computation: every result is the structured
payload emitted by the CLI, and a non-zero exit code raises `axiom.errors.AxiomError`
carrying the decoded JSON and exit code. This keeps the Python surface aligned
with the audited Rust core.

## 4. What to remember

* A claim is only `Verified` when it has a complete derivation and every
  mandatory obligation is `Satisfied` (or `Waived`). Assertions, observations,
  and assumptions can never silently become verified (`CoreError::SilentVerify`).
* External operations require a capability (granted explicitly via
  `Runtime::grant` / `--cap`) and, for verification, an integrity-checked
  receipt. The default runtime refuses all live external calls.
* The module digest is content-addressed and deterministic: identical inputs,
  operation versions, capabilities, evidence, and receipts always yield the same
  digest, which is what makes offline replay a sound check.
