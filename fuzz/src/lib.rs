//! Shared adversarial fuzz logic for Axiom IR.
//!
//! This library is used by two front-ends:
//!
//! 1. The nightly `cargo-fuzz` targets (`fuzz_targets/parse.rs`,
//!    `fuzz_targets/runtime.rs`), which call these functions directly. Under
//!    cargo-fuzz, libFuzzer installs its own panic hook, so a panic here is
//!    reported as a *crash* (the desired behaviour for finding bugs).
//!
//! 2. The stable smoke harness (`src/main.rs`), which wraps every call in
//!    [`std::panic::catch_unwind`] so a panic in the Axiom code is recorded as
//!    a *finding* rather than aborting the harness.
//!
//! # Security invariant under test
//!
//! Parsing arbitrary input and (non-privileged) execution of any parsed module
//! must **never** panic or abort. They may return `Err` — that is the expected,
//! graceful path for malformed or unsupported input. A panic is a real bug in
//! `axiom-parser` / `axiom-runtime` / `axiom-core`.

/// Fuzz the parser with arbitrary bytes.
///
/// The parser must never panic: it should only return a `Result`. The return
/// value of `parse_module` is intentionally ignored.
///
/// # Panics
///
/// This function does **not** catch panics on its own. Nightly `cargo-fuzz`
/// relies on that (a panic = a crash to report). The stable smoke harness wraps
/// the call in `catch_unwind`.
pub fn fuzz_parse(data: &[u8]) {
    let src = String::from_utf8_lossy(data);
    let _ = axiom_parser::parse_module(&src);
}

/// Fuzz the runtime with arbitrary bytes.
///
/// `run_source` parses internally and then executes, so this exercises both the
/// parser and the executor. Execution of arbitrary (parsed) modules must never
/// panic or abort; returning `Err` is fine.
///
/// # Panics
///
/// Like [`fuzz_parse`], this does not catch panics itself; the smoke harness is
/// responsible for that.
pub fn fuzz_runtime(data: &[u8]) {
    let src = String::from_utf8_lossy(data);
    if let Ok(_ast) = axiom_parser::parse_module(&src) {
        // Only execute modules that actually parsed. `run_source` re-parses,
        // but we guard with the explicit check so a parse that *should* fail
        // never reaches the executor.
        let _ = axiom_runtime::run_source(&src);
    }
}
