# Deterministic Execution and Replay

Axiom IR is designed so that a given module, executed under the same inputs and operation
versions, always produces the same claims, derivations, obligations, contradictions, event
log, and final digest. This document explains the mechanisms: exact arithmetic,
content-addressed receipts, capability gating, and the determinism theorem. It walks
through offline replay and receipt integrity.

## Exact arithmetic (no floats in the normative path)

The normative numeric type is `axiom_types::Num`, which is one of:

- `Int(i128)`,
- `Rational { num: i128, den: i128 }` (always reduced, `den > 0`),
- `Decimal { mantissa: i128, scale: u8 }` (fixed scale, `scale <= 38`).

`Num` offers `checked_add`, `checked_sub`, `checked_mul`, `checked_div`, `checked_neg`,
and a canonical `cmp_num` ordering. There is no IEEE-754 `f64` anywhere in the normative
path. Quantities (`Quantity { value: Num, unit: Unit }`) carry unit algebra
(`Unit::multiply`, `Unit::divide`, `Unit::same_dimension`), so dimension errors are
caught at type-check time, not approximated at runtime.

Because arithmetic is exact, canonical encoding (`axiom_encoding::canonical_bytes`) and
replay never depend on floating-point rounding. This is the precondition for the
determinism theorem.

## Content-addressed receipts

External operations (`OpClass::VerifiedExternal` / `UnverifiedExternal`) do not execute
inside the trusted core. Instead, a call captures an immutable `Receipt`
(`axiom_core::Receipt`):

```
Receipt {
    id, operation, op_version, provider, logical_time,
    inputs_hash, output, output_hash, schema, integrity, signature
}
```

`inputs_hash` is a content id over the operation's normalized inputs (in the runtime it is
`content_id(Domain::Receipt, input_values)`); `output` is the returned value, carried so
replay can reconstruct the claim without re-invoking the external operation; `integrity`
is a digest over `operation | op_version | provider | logical_time | inputs_hash | output
| schema`. `Receipt::verify_integrity()` recomputes that digest and compares it to
`integrity` — if they differ, the receipt was tampered with (see below).

External operations are registered at `VerifiedExternal` with
`DeterminismClass::Receipted`: deterministic *given an identical receipt*. The runtime
builds an `OperationDef` for each `call` (name = the op string, class `VerifiedExternal`,
`generates = [ToolReceiptValidation]`). The `ToolReceiptValidation` obligation is
auto-satisfied when a receipt is attached.

## Capability gating

External calls are capability-gated. A `call` statement names a capability with
`cap "<cap>"`, where `cap` is rendered by `Capability::Display` as `tool:calculator`,
`net:http`, `fs-read:...`, or `priv:...`. The runtime maintains a `BTreeSet` of granted
capability strings; `Runtime::grant(cap)` adds one.

- In live mode, `exec_call` checks `self.caps.contains(capability)` and returns
  `RuntimeError::CapabilityDenied` if the capability is absent. Only then does it invoke
  the live `ExternalExecutor` (`ToolCalculator` for `tool.calculator`) and record a
  receipt via `Module::add_receipt`.
- The default executor is `ReplayOnlyExecutor`, which refuses all live external calls. So
  a freshly constructed `Runtime` performs no external side effect unless the operator
  explicitly grants a capability and installs an executor (`with_builtin_tool`).

External operations MUST NOT be treated as pure unless a receipt makes replay possible.
If an external operation is requested without a receipt and no live executor with the
required capability is available, the transition fails.

## The determinism theorem

> Given identical canonical module input, operation implementations and versions,
> capability configuration, evidence, external receipts, and runtime semantic version,
> execution MUST produce identical claim identities, derivation identities, obligation
> states, contradiction witnesses, event log, exported results, and final module digest.

Why it holds (sketch, matching `spec/semantics.md` §4 and §6.2):

- **Inputs are canonical.** `Id` hashes `domain_tag || 0x00 || canonical_bytes` (SHA-256),
  and `Module::digest` sorts node ids before canonicalizing. Identical semantic content →
  identical ids → identical digest.
- **Internal ops are pure.** `BuiltinExecutor::exec` is a pure function of `(OperationDef,
  inputs)`. Pure op names are namespaced (`core.add`, …); a bare short name like `add`
  resolves to `core.add` via `Runtime::resolve_op`.
- **External ops are receipt-bounded.** When a receipt is present, `derive` reconstructs
  the output from `executor.resolve_receipt(r)`, which returns `r.output` — fixed by the
  receipt's integrity hash. The live value is never consulted.
- **The event log and digest are functions of committed state**, which is identical.

## Offline replay walkthrough

Step 1 — a small module whose external call uses the fixture calculator:

```text
module calc "1"

assert a = 2 : rational
assert b = 3 : rational

call total = tool.calculator(a, b) : rational cap "tool:calculator"

verify total
```

`tool.calculator` sums its numeric inputs and is provided by `ToolCalculator`. It is
"external" only in the sense of capability gating and receipt capture; its computation is
exact and reproducible, which is what makes replay produce an identical digest.

Step 2 — live run, capturing receipts. The live run grants the capability and installs the
builtin tool, then writes a receipt log:

```text
axiom run calc.axiom --cap tool:calculator --builtin-tool --emit-receipts calc.receipts.json
```

`--emit-receipts <path>` writes a `ReceiptLog`:

```json
{ "source": "calc", "receipts": [ /* Receipt values */ ] }
```

(The CLI `Replay` command and `axiom-conformance` both read this `{source, receipts}`
shape.)

Step 3 — replay, no capability. Replay mode loads the receipts, inserts them into the
module, and executes **without** invoking any live executor (the default executor refuses
live calls):

```text
axiom replay calc.receipts.json
```

`Runtime::replay_mode(receipts)` sets `replay = true`, preloads the `Receipt`s, and
`exec_call` then resolves the output from the matching receipt (`operation == op &&
inputs_hash == inputs_hash && verify_integrity()`), never calling the external executor.
This is why replay does **not** require the capability — the presence and integrity of the
receipt is the sufficient condition.

The `axiom replay` command re-executes the module twice — once live (with the capability +
builtin tool) to get the reference digest, once in replay mode — and compares digests. Its
JSON output:

```json
{
  "live_digest": "module.1. <hex64>",
  "replay_digest": "module.1. <hex64>",
  "identical": true,
  "receipts": 1
}
```

`identical: true` is the assertion of the determinism theorem for this module. Exit code
is non-zero if the digests differ.

## Receipt integrity (tamper detection)

`Receipt::verify_integrity()` re-derives the integrity digest from the recorded fields and
compares it to `self.integrity`. In `Module::derive`, if a supplied receipt fails this
check the core returns `CoreError::ReceiptTampered` (surfaced as
`RuntimeError::…`); the module never uses the suspect output.

The core test suite demonstrates the guarantee directly: a receipt whose `output` is
mutated after creation fails `verify_integrity()`, while an untouched receipt passes. This
is what lets a downstream consumer trust a receipt they did not witness being created —
any alteration to `operation`, `op_version`, `provider`, `logical_time`, `inputs_hash`,
`output`, or `schema` is detected.

## What is NOT deterministic

- **`NonDeterministic` and `Receipted` operations.** A `NonDeterministic` operation may
  differ run to run and requires a receipt to replay; a `Receipted` external operation is
  deterministic only *given an identical receipt*. Without a receipt, an external call
  cannot be replayed and will fail in replay mode (`RuntimeError::ReplayMissingReceipt`).
- **Live external outputs.** The actual bytes returned by a live `ExternalExecutor` are not
  part of the guarantee; only the recorded receipt (and its integrity hash) is. If you
  lose the receipt log, you cannot reproduce an external derivation offline.
- **Capability configuration and event order.** Determinism assumes the *same* capability
  set and the same canonical input. The runtime commits events in execution order; two
  runs that reach the same final state via different execution orderings could in principle
  differ in event-log ordering, which is why the determinism theorem fixes the inputs,
  operation versions, *and* capability configuration.
- **Floating point.** There is none in the normative path by design; any future extension
  that introduced IEEE-754 arithmetic would forfeit cross-platform determinism and must be
  marked `NonDeterministic`.

## Relationship to other docs

- The TCB and module state model: `docs/architecture.md`
- Incremental invalidation and how a replayed receipt interacts with recomputation:
  `docs/incremental-invalidation.md`
- Normative semantics: `spec/semantics.md` §4 (Determinism), §5 (Replay)
