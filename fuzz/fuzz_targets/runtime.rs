#![no_main]
//! Nightly cargo-fuzz target: adversarial runtime fuzzing.
//!
//! Run with:
//! ```text
//! rustup run nightly cargo +nightly fuzz run runtime -- -runs=100000
//! ```
//!
//! Execution of arbitrary parsed modules must never panic/abort; it may return
//! `Err`, which is fine. Under cargo-fuzz a panic is reported as a crash.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    axiom_fuzz::fuzz_runtime(data);
});
