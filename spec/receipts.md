# Axiom IR — External Receipts and Deterministic Replay

External operations (tool calls, model endpoints, database reads, signed attestations) are a
necessary part of real reasoning. Axiom makes them **deterministic and replayable** by
capturing their outcomes as immutable **receipts**.

## 1. The receipt model

A receipt `r ∈ R`:

```
r = ( id, operation, op_version, provider, logical_time,
      inputs_hash, output, output_hash, schema, integrity, signature )
```

* `operation`, `op_version` — the requested operation identity.
* `provider` — the external identity (e.g. `fixture:calculator`, `model:gpt-...`).
* `logical_time` — a logical clock where relevant (not wall-clock; replay is logical-time
  stable).
* `inputs_hash` — content hash of the normalized inputs.
* `output` — the returned value, carried inline so replay reconstructs it without re-calling.
* `output_hash` — content hash of `output`.
* `schema` — the declared output type.
* `integrity` — `SHA-256(receipt || 0x00 || canonical_bytes(integrity_fields))`.
* `signature` — optional cryptographic signature over `integrity` by `provider`.

## 2. Integrity verification

`r.verify_integrity()` recomputes `integrity` from the recorded fields and compares. If it
differs, the receipt was tampered with and MUST be rejected: any derivation that references
it MUST NOT verify, and replay MUST fail.

## 3. Capability gating

External operations are capability-controlled. The default runtime grants **no**
capabilities. A `call` with an ungranted capability fails with `ERR-CAPABILITY-DENIED`. The
operation's `capabilities` field declares what it requires; the runtime's granted set is
explicit. A hostile module cannot invoke undeclared capabilities.

## 4. Replay

Replay reconstructs a module's external outputs from a set of receipts **without** invoking
any live executor:

* In replay mode the runtime resolves each external derivation's output from its receipt
  (`OpExecutor::resolve_receipt`).
* Replay MUST NOT require the live capability; receipt presence + integrity are sufficient.
* Replay produces identical claim identities, derivation identities, obligation states,
  contradiction witnesses, event log, exported results, and final module digest as the
  original live execution — this is the determinism guarantee.

## 5. Receipt capture (live)

When a `call` executes live (capability granted, executor available), the runtime:

1. hashes the normalized inputs,
2. invokes the executor,
3. records the output as a receipt,
4. derives the output claim referencing that receipt,
5. satisfies the `tool-receipt-validation` obligation (the receipt is present and integral).

## 6. Forged-receipt defense

* An external derivation without a receipt cannot be verified (INV-VERIFY.3).
* A receipt with a mismatched `integrity` cannot be verified or replayed.
* A receipt whose `output` was edited after capture fails `verify_integrity` (asserted by the
  receipt-forgery tests).
* A receipt whose `inputs_hash` does not match the derivation's actual inputs is rejected by
  the executor at replay resolution.

## 7. Example

```
call sum = tool.calculator(a, b) : rational cap "tool:calculator"
```

Live: requires capability `tool:calculator`; emits a receipt `r`; `sum` is derived from `r`.
Replay (capability revoked): `sum` is reconstructed from `r` with identical value and digest.
If `r.output` is edited, replay fails integrity.
