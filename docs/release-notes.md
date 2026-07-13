# Axiom IR v0.2.0 — stable release

A universal, proof-carrying intermediate representation for machine reasoning,
with cryptographic receipt/evidence authenticity and a deterministic producer.

## Highlights

- **Obligation-gated verification** — a `verified` claim is a node whose only
  valid producers are derivations in which every mandatory proof obligation is a
  first-class node in state `satisfied`. Unsupported assertions never silently
  become `verified`.
- **Cryptographic evidence & receipt authenticity** — HMAC-SHA256 over
  host-configured `TrustRoot`s. Forged, unsigned, or mismatched `trust = trusted`
  evidence is rejected (`EvidenceForgery`). Receipts are bound to the derivation's
  actual inputs and verified on replay.
- **Deterministic producer** (`axiom-producer`) — translates a structured
  `ReasoningPlan` (JSON) into valid Axiom IR source; types/arities validated
  against the shared operation registry. Drives CLI demo 8.
- **Exact, deterministic, replayable** — exact arithmetic (int/rational/decimal;
  floats forbidden), quantities with unit checks, dependency-correct incremental
  invalidation, offline receipt replay.

## Verification gates (all green)

| Gate | Result |
|---|---|
| Unit / integration / property tests | **93 passed, 0 failed** |
| Conformance corpus | **17 / 17 fixtures** |
| Demonstrations | **8 / 8** |
| `cargo clippy --workspace` | clean (`-D warnings`) |
| `cargo fmt --all --check` | clean |

## Install / build

```sh
cargo build --release -p axiom-cli      # produces ./target/release/axiom
# or download a prebuilt binary for your platform from the release assets
```

## Quick demo (under 1 minute)

The producer generates a module; a valid proof passes; a falsified proof fails;
a premise change invalidates only its dependents; replay recovers the same
digest.

```sh
./examples/quick-demo.sh
```

Or step by step:

```sh
# 1. producer generates the module
axiom produce --plan '{"module_name":"demo","premises":[{"label":"force","value":{"Quantity":[10,"N"]},"evidence":null},{"label":"dist","value":{"Quantity":[2,"m"]},"evidence":null}],"assumptions":[],"steps":[{"label":"work","op":"qmul","inputs":["force","dist"],"requires_obligation":false,"discharge_by":null,"verify":true}]}' --output demo.axiom

# 2. valid proof passes
axiom verify demo.axiom            # -> verified: work

# 3. falsified (incomplete) proof fails
#    same plan but requires_obligation:true with no discharge -> work NOT verified

# 4. a premise change invalidates ONLY its dependents
axiom invalidate demo.axiom force   # -> invalidated: work ; preserved: dist

# 5. replay recovers the same digest (offline, capability-free)
axiom run --emit-receipts r.json demo.axiom >/dev/null
axiom replay r.json                  # -> identical: true
```

## Independent cross-check

A from-scratch Python re-implementation validates part of the corpus without
using the Rust crates:

```sh
python python/axiom_checker/run_conformance.py
# independent Python checker: 14 passed, 0 failed, 3 skipped (of 17 fixtures)
```

## Checksums

Release assets are SHA-256 checksummed. Local macOS (Apple Silicon) binary:

```
SHA_MACOS_ARM64  axiom
```

CI (`.github/workflows/release.yml`) cross-builds Linux (x86_64), Windows
(x86_64), and macOS (x86_64 + Apple Silicon) on every tag and attaches
checksummed binaries to the release.

## License

Apache License 2.0. See `LICENSE`.
