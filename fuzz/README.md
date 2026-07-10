# Axiom IR — Fuzz Harness

Adversarial fuzz-testing for the Axiom IR parser (`axiom-parser`) and runtime
(`axiom-runtime`). The harness tries to break the parser and runtime with
hostile input — malformed syntax, parser resource exhaustion, deeply nested
structures, cyclic graphs, hash confusion, forged receipts, ungranted
capabilities, and more.

This directory (`fuzz/`) is **excluded from the root workspace** (see the root
`Cargo.toml` `exclude = ["fuzz"]`) and is a standalone crate. It has its own
`target/` directory, so building it never disturbs the main workspace build,
and it must **not** modify anything under `crates/`.

## Security invariant under test

> Parsing arbitrary bytes and (non-privileged) execution of any parsed module
> must **never panic or abort**. They may only return `Err`. A panic is a real
> bug in `axiom-parser` / `axiom-runtime` / `axiom-core`.

Two layers are provided:

1. **Nightly `cargo-fuzz` targets** (`fuzz_targets/parse.rs`,
   `fuzz_targets/runtime.rs`) for serious coverage-guided fuzzing. Under
   `cargo-fuzz`, libFuzzer owns the panic hook, so a panic is reported as a
   *crash* (the desired signal).
2. **Stable smoke harness** (`src/main.rs`, binary `axiom-fuzz-smoke`) that
   needs only the stable toolchain and demonstrates the fuzz logic without
   nightly. It wraps every call in `std::panic::catch_unwind`, so even if the
   Axiom code panics, the harness records the offending input as a *finding*
   (hex dump) and keeps going — it is crash-proof by construction.

The shared logic lives in `src/lib.rs` (`fuzz_parse`, `fuzz_runtime`) and is
referenced by both front-ends.

## How to run

### Stable smoke (no nightly required)

```sh
cd fuzz
cargo run
```

Expected output ends with a summary line such as:

```
fuzz smoke: 200008 inputs, 0 panics
no panics observed — Axiom code withstood all adversarial input
```

If any input panics the Axiom code, the harness prints the input as hex under a
`--- findings ---` section instead of crashing, e.g.:

```
fuzz smoke: 200008 inputs, 2 panics
  parser panics:  1
  runtime panics: 1
--- findings (panicking inputs; up to 32 shown) ---
[parse/random] 37 bytes: 6d6f64756c6520...
```

### Nightly cargo-fuzz (coverage-guided)

Requires the Rust nightly toolchain and `cargo-fuzz`:

```sh
rustup toolchain install nightly
cargo +nightly install cargo-fuzz
rustup run nightly cargo +nightly fuzz run parse   -- -runs=100000
rustup run nightly cargo +nightly fuzz run runtime -- -runs=100000
```

(These targets were verified to *compile* on stable as part of `cargo build`;
actual libFuzzer execution needs nightly + the sanitizer runtime provided by
`cargo-fuzz`.)

## What the smoke harness does

- **Randomized inputs:** `RANDOM_ITERS` (200,000) adversarial byte sequences
  produced by a tiny deterministic xorshift64* PRNG seeded from a per-iteration
  counter. Each is fed to both `fuzz_parse` and `fuzz_runtime`.
- **Hand-crafted adversarial cases** (`adversarial_cases()` in `src/main.rs`):
  - `deeply_nested_record` — a 2000-deep `record { inner = ... }` literal
    (parser + value-check recursion / stack pressure).
  - `long_identifier` — a 200,000-character identifier/token (lexer/parser must
    not blow up on oversized tokens).
  - `many_derive_chains` — 5,000 repeated `derive` statements.
  - `contradict_storm` — 5,000 contradictory `contradict` statements.
  - `ungranted_capability` — a `call` requiring `cap "tool:calculator"` that is
    never granted (must be handled gracefully, not panic).
  - `forged_receipt` — a `derive` referencing an unknown/missing receipt.
  - `cyclic_derivation` — self-referential `derive` graph.
  - `malformed_syntax` — truncated / junk syntax barrage.

## Adversarial categories targeted

| Category                     | Where exercised                                  |
|------------------------------|--------------------------------------------------|
| Malformed syntax             | randomized bytes, `malformed_syntax`             |
| Parser resource exhaustion   | `long_identifier`, huge randomized inputs        |
| Deeply nested structures     | `deeply_nested_record`                           |
| Many repeated constructs     | `many_derive_chains`, `contradict_storm`         |
| Cyclic graphs                | `cyclic_derivation`                              |
| Hash confusion               | randomized inputs (content-addressed identity)   |
| Forged receipts              | `forged_receipt`                                 |
| Ungranted / privileged caps  | `ungranted_capability`                           |

## Known issues to fix in core

None observed. The smoke harness ran `200008` inputs (200,000 randomized + 8
hand-crafted) against the current `axiom-parser` / `axiom-runtime` /
`axiom-core` and recorded **0 panics**. Both the parser (`parse_module`) and
runtime (`run_source`) returned `Err` gracefully for hostile input rather than
panicking.

If a future run reports panics, add the exact input (hex, as printed under
`--- findings ---`) and the expected-vs-actual behaviour here, together with a
pointer to the responsible code in `crates/`. **Do not patch `crates/` from
this harness** — that must be fixed in the core crates directly.

## Layout

```
fuzz/
  Cargo.toml            # standalone crate: lib + 3 bins (smoke, parse, runtime)
  src/
    lib.rs              # shared fuzz logic: fuzz_parse / fuzz_runtime
    main.rs             # stable smoke harness (xorshift PRNG + catch_unwind)
  fuzz_targets/
    parse.rs            # nightly cargo-fuzz target
    runtime.rs          # nightly cargo-fuzz target
  README.md
```
