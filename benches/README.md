# Axiom IR Benchmarks (`axiom-benches`)

This crate is a `criterion` benchmark suite for the Axiom IR workspace. It
measures the performance characteristics the Axiom IR spec requires, honestly
and reproducibly. **No numbers are fabricated**: `criterion` prints the real
measurements at runtime. Absolute wall-clock figures depend entirely on the
machine, toolchain, and load, so this document states methodology, not results.

## How to run

```sh
# Build and run the whole suite (slow — criterion default measurement is long):
cargo bench -p axiom-benches --benches

# Run a single benchmark target quickly (smaller sample size / measurement time):
cargo bench -p axiom-benches --bench parse -- --quick
cargo bench -p axiom-benches --bench incremental -- --quick

# List available bench targets:
#   parse format execute transactional encode graph
#   obligations contradiction incremental explain replay conformance
```

`--benches` restricts the run to benchmark targets (the crate also has a lib
target, `axiom_benches`, whose unit tests do not accept criterion flags).

## Hardware / toolchain assumptions

* **Compiler:** the workspace default toolchain (rustc ≥ 1.85 as pinned by
  `rust-version` in the root `Cargo.toml`). Benchmarks are compiled in the
  `bench` profile (`opt-level = 3`, `lto = "thin"`).
* **Hardware:** any machine capable of running the workspace. The numbers you
  get are specific to *your* CPU, cache, memory, and OS scheduler. **Do not
  compare absolute numbers across machines** — only relative trends on the same
  machine under the same conditions are meaningful.
* **Load:** close other CPU-heavy work before running; criterion is sensitive to
  scheduling jitter. Pinning the process / disabling turbo can tighten CIs but is
  not required.

## Statistical methodology

`criterion` 0.5 with its **default** configuration:

* **Sample size:** 100 measurements per benchmark (the quick preset lowers this).
* **Measurement time:** ≥ 5 s of measurement per benchmark (the quick preset
  lowers this).
* **Warm-up:** 3 s before measurement begins.
* **Confidence interval:** 95% bootstrap CI over the sample.
* **Outlier handling:** criterion detects and reports outliers; it does not
  silently drop them.
* Each run prints the estimated mean time and the 95% CI. Use those; do not read
  a single iteration's time as the result.

The suite never asserts a performance *target*. It only asserts that the code
**runs** (idempotence, verified-claim invariants, non-panicking execution).

## Scenarios and dataset sizes

| Bench target   | Scenario                                  | Dataset sizes (per input)                                   |
|----------------|-------------------------------------------|-------------------------------------------------------------|
| `parse`        | Parsing (no semantics)                    | chain depth 10/100/1000; wide fan-out width 10/100/1000; small module |
| `format`       | Idempotent `format_source` round-trip     | same families as `parse` (idempotence asserted in setup)    |
| `execute`      | Type-check + execution (`run_source`)     | chain 10/100/1000; wide 10/100/1000; small                  |
| `transactional`| Full execute vs re-execution of AST       | chain 10/100/1000; wide 10/100/1000; small (both paths)     |
| `encode`       | `canonical_bytes` / `content_id` / `module.digest()` | chain 10/100/1000; wide 10/100/1000            |
| `graph`        | `DependencyGraph::build` / `reverse_dependents` | chain 100/1000; wide 100/1000; contexts 100/500/1000; parallel 64×32 |
| `obligations`  | Obligation discharge + verify             | star of `div` derivations k = 10/50/200 (one mandatory `numeric-bounds` obligation each) |
| `contradiction`| `detect_contradictions` + `contradictions()` | n = 10/30/100 disjoint intervals (C(n,2) contradiction records) |
| `incremental`  | Invalidation + incremental vs full recompute | chain 100/1000; wide 100/1000; parallel 64×32 (private & shared premise) |
| `explain`      | `explain` on a deep-chain leaf            | chain 10/100/1000                                           |
| `replay`       | Cold replay vs warm replay vs live run    | single `call tool.calculator` fixture                       |
| `conformance`  | Execute every `conformance/valid/*.axiom` | all 18 valid fixtures (per-fixture + execute-all)           |

### Honest framing of the incremental comparison

`incremental` (scenario 10) is the only benchmark with a built-in *comparison*.
It reports both `invalidate_and_recompute` (incremental) and `full_recompute`
(full) on a module of many parallel chains sharing a common root:

* **Changing a *private* premise of one chain** — incremental recomputes only
  that chain; full recomputes everything. Incremental wins.
* **Changing the *shared root*** — incremental must recompute everything too.
  Incremental ≈ full; there is no free lunch.

The affected-set sizes (invalidated + recomputed claims) are printed to stderr
during the run, and a `std::time` one-shot comparison prints both wall-clock
costs. Read those together with the criterion timings; they are *measurements*,
not claims.

## API notes / deviations discovered while writing the suite

The public API matches the spec with these concrete details (useful if you
reuse these benches):

* `axiom_runtime::Runtime::contradictions()` returns **`Vec<Contradiction>`**
  (owned clones), not `Vec<&Contradiction>` as the brief suggested.
* `axiom_parser::format_source(&str) -> Result<String, Vec<Diagnostic>>` —
  matches. `parse_module` returns `ParseResult<ModuleAst>` (`Result<…,
  Vec<Diagnostic>>`); `run_source` parses internally and returns `Result<Runtime, RuntimeError>`.
* `axiom_runtime::run_source` uses a *replay-only* external executor with **no
  capabilities**, so it **cannot execute external `call` statements**. Benchmarks
  that need an external tool (`replay`, and any conformance fixture using `call`)
  therefore build a `Runtime` manually with `.with_builtin_tool().grant("tool:calculator")`
  instead of `run_source`.
* `axiom_incremental::{invalidate_and_recompute, full_recompute}` take
  `&dyn OpExecutor`; the benchmarks pass `&axiom_core::registry::BuiltinExecutor`.
* `axiom_core::Module` is `Clone`, but `Runtime` is **not** (it owns a
  `Box<dyn ExternalExecutor>`). Benchmarks that need a fresh copy per iteration
  (obligations, incremental) clone the `Module` and re-run the mutation/query on
  the clone; the clone cost is included in the measured block and is small
  relative to the work for the larger sizes.
* **Exact-arithmetic caveat:** the `Num` exact rational uses `i128`
  denominators. A long chain of `div` derivations that keeps halving (or, due to
  the crate's `checked_div` implementation, doubling) overflows `i128` capacity
  around ~120+ steps. The `obligations` generator therefore uses a *star* of
  independent `div(a, c_i)` derivations so its size is bounded by the number of
  obligations, not by denominator capacity. This is a property of the core's
  exact `Num`, not of the benchmark logic.

## Reproducibility

* The dataset generators in `src/generators.rs` are pure string builders, so the
  exact same source text is produced on every machine.
* Results are deterministic given the same binary and machine; they are **not**
  cross-machine comparable, and this document makes no performance claims.

Run `cargo bench -p axiom-benches --benches` and read the numbers criterion
prints for your environment.
