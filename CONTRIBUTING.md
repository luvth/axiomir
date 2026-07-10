# Contributing to Axiom IR

Thank you for contributing. Axiom IR is a standardization-minded project: the `spec/`
directory is the **source of truth**, and the implementation in `crates/` must not diverge
from it. When a spec and the code disagree, the spec wins and the code must be corrected.

## Building and testing

A stable Rust toolchain is required:

```sh
rustup toolchain install stable
rustup default stable

cargo test --workspace
cargo run --release -p axiom-cli -- demo all
cargo run --release -p axiom-cli -- conform
cargo run --release -p axiom-cli -- doctor
```

Before opening a pull request, ensure:

```sh
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

## Commit style

This repository uses **meaningful, staged commits** — one logical change per commit, with a
subject line that states what the change does and why. Avoid "wip", "fix", or "updates" as
standalone subjects. Split large changes (e.g., a new operation plus its tests plus its docs)
into reviewable commits.

## Adding a builtin operation

Builtin operations are registered in `crates/axiom-core/src/registry.rs`. To add one:

1. Define the operation in `registry.rs`: its name, input and output types, required capabilities,
   determinism class (`Pure`, `NonDeterministic`, or `Receipted`), and its `exec` implementation
   in the `BuiltinExecutor`.
2. Add a unit test that exercises the operation through `Runtime` and asserts the derived value and
   `verified` status (where applicable).
3. If the operation is external, ensure it captures a receipt and that replay reconstructs the
   output without the live capability.
4. Update `docs/` only if the operation changes observable behavior; do not modify `spec/` unless
   the change is also reflected there first.

## Adding a conformance fixture

Conformance fixtures live under `conformance/`, split into `valid/` and `invalid/`. Each fixture
is a `.axiom` module plus a `.expect.json` file describing the expected outcome.

- `valid/<name>.axiom` + `valid/<name>.expect.json` — the module is expected to execute and
  satisfy the assertions in the expectation file (e.g., specific claims reach `verified`).
- `invalid/<name>.axiom` + `invalid/<name>.expect.json` — the module is expected to be rejected
  (parse error, type error, unit mismatch, undeclared premise, unknown operation, fabricated
  evidence, or an undischarged mandatory obligation).

Both `axiom conform` and the conformance tests assert the fixtures. A new capability is not
considered done until it has at least one `valid` and one `invalid` fixture.

## Coding standards

- **No TODOs, stubs, or placeholders.** A merged change is complete. If a feature is intentionally
  partial, document the limitation in `spec/` and `CHANGELOG.md`, not in code comments.
- **Tests are required.** Every operation, transition, and public API change ships with tests.
  Prefer property-based tests (`proptest`) for encoding and arithmetic.
- **Formatting and linting must pass.** `cargo fmt --all -- --check` and
  `cargo clippy --workspace -- -D warnings` are enforced in CI.
- **Determinism is non-negotiable.** Do not introduce floating-point arithmetic into the normative
  numeric path. Use exact arithmetic (integer, rational, decimal).
- **Standardization mindset.** Treat `spec/semantics.md` (INV-VERIFY, INV-MINIMALITY) and
  `spec/encoding.md` as authoritative. New node kinds, transitions, or obligations must be
  described there before they are implemented.

## Reporting issues

Open a public issue for bugs and feature requests. For vulnerabilities, follow `SECURITY.md`
instead — do **not** open public issues for security reports.
