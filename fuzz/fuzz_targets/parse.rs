#![no_main]
//! Nightly cargo-fuzz target: adversarial parser fuzzing.
//!
//! Run with:
//! ```text
//! rustup run nightly cargo +nightly fuzz run parse -- -runs=100000
//! ```
//!
//! Under cargo-fuzz, libFuzzer owns the panic hook, so a panic inside
//! `axiom_parser::parse_module` is reported as a crash — exactly what we want.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    axiom_fuzz::fuzz_parse(data);
});
