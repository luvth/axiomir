//! Stable-runnable adversarial smoke harness for Axiom IR.
//!
//! This needs only the stable Rust toolchain (no nightly, no cargo-fuzz). It:
//!   1. Generates adversarial byte sequences with a tiny deterministic xorshift
//!      PRNG seeded from a counter.
//!   2. Feeds them to the shared fuzz logic in `axiom_fuzz`, wrapped in
//!      [`std::panic::catch_unwind`] so a panic in the Axiom code is recorded as
//!      a *finding* rather than aborting the whole harness.
//!   3. Runs a set of hand-crafted adversarial cases (deeply nested records,
//!      huge tokens, derive storms, contradict storms, ungranted capabilities,
//!      forged receipts, cyclic derivations, malformed syntax).
//!
//! The harness is crash-proof by construction: even if `axiom-*` panics, we
//! print the offending input as hex and keep going. Run it with:
//! ```text
//! cd fuzz && cargo run
//! ```

use std::panic::{self, AssertUnwindSafe};

use axiom_fuzz::{fuzz_parse, fuzz_runtime};

/// Number of randomized adversarial inputs to generate.
const RANDOM_ITERS: u64 = 200_000;
/// Maximum number of panicking inputs to record (to bound memory).
const MAX_FINDINGS: usize = 32;

/// Tiny deterministic PRNG: xorshift64*. Std-only, no extra dependencies.
struct XorShift {
    state: u64,
}

impl XorShift {
    fn new(seed: u64) -> Self {
        XorShift { state: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_bytes(&mut self, len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            out.extend_from_slice(&self.next_u64().to_le_bytes());
        }
        out.truncate(len);
        out
    }
}

/// Run `fuzz_parse` on `data`, returning `true` if it panicked.
fn check_parse(data: &[u8]) -> bool {
    panic::catch_unwind(AssertUnwindSafe(|| fuzz_parse(data))).is_err()
}

/// Run `fuzz_runtime` on `data`, returning `true` if it panicked.
fn check_runtime(data: &[u8]) -> bool {
    panic::catch_unwind(AssertUnwindSafe(|| fuzz_runtime(data))).is_err()
}

/// Record a panicking input (capped to `MAX_FINDINGS`).
fn record(findings: &mut Vec<(String, Vec<u8>)>, label: &str, data: &[u8]) {
    if findings.len() < MAX_FINDINGS {
        findings.push((label.to_string(), data.to_vec()));
    }
}

/// Build a deeply nested record literal of the given depth, e.g.
/// `{ inner = { inner = ... true ... } }`.
fn nested_record(depth: usize) -> String {
    let mut body = String::new();
    for _ in 0..depth {
        body.push_str("{ inner = ");
    }
    body.push_str("true");
    for _ in 0..depth {
        body.push_str(" }");
    }
    body
}

/// Hand-crafted adversarial cases targeting specific abuse categories.
fn adversarial_cases() -> Vec<(String, String)> {
    let mut cases = Vec::new();

    // 1. Deeply nested `record { ... }` literal — parser recursion / stack
    //    pressure and the runtime's value-check recursion.
    let depth = 2000;
    cases.push((
        "deeply_nested_record".into(),
        format!("module deep \"1\"\nassert x = {} : bool\n", nested_record(depth)),
    ));

    // 2. Extremely long identifier / token string — lexer and parser must not
    //    blow up on oversized tokens.
    let big_ident = "a".repeat(200_000);
    cases.push((
        "long_identifier".into(),
        format!("module {big_ident} \"1\"\nassert x = true : bool\n"),
    ));

    // 3. Many repeated `derive` chains.
    let mut deriv = String::from("module d \"1\"\n");
    for i in 0..5000 {
        deriv.push_str(&format!("derive d{i} = qadd(a, b) : quantity\n"));
    }
    cases.push(("many_derive_chains".into(), deriv));

    // 4. Contradict storm — many contradictory `contradict` statements.
    let mut storm = String::from(
        "module c \"1\"\nassert a = true : bool\nassert b = false : bool\n",
    );
    for i in 0..5000 {
        storm.push_str(&format!("contradict a b as kind{i}\n"));
    }
    cases.push(("contradict_storm".into(), storm));

    // 5. `call` requiring an ungranted capability — must be handled gracefully
    //    (return Err), never panic/abort.
    cases.push((
        "ungranted_capability".into(),
        "module cap \"1\"\nassert x = q(1 \"C\") : quantity\n\
         call c = tool.calc(x) : quantity cap \"tool:calculator\"\n"
            .into(),
    ));

    // 6. Forged / unknown receipt reference.
    cases.push((
        "forged_receipt".into(),
        "module r \"1\"\nassert x = q(1 \"C\") : quantity\n\
         derive y = tool.y(x) : quantity receipt missing\n"
            .into(),
    ));

    // 7. Cyclic derivation references (self-referential graph).
    cases.push((
        "cyclic_derivation".into(),
        "module cyc \"1\"\n\
         derive a = qadd(a, b) : quantity\n\
         derive b = qadd(a, b) : quantity\n"
            .into(),
    ));

    // 8. Malformed / truncated syntax barrage.
    cases.push((
        "malformed_syntax".into(),
        "module x \"1\"\nassert = : bool\n{[[[(((\n<>?/\\\nderive = = =\n".into(),
    ));

    cases
}

fn to_hex(data: &[u8]) -> String {
    let limit = data.len().min(128);
    let mut s = String::with_capacity(limit * 2 + 3);
    for b in &data[..limit] {
        s.push_str(&format!("{b:02x}"));
    }
    if data.len() > limit {
        s.push_str("...");
    }
    s
}

fn main() {
    let mut total = 0u64;
    let mut parse_panics = 0u64;
    let mut runtime_panics = 0u64;
    let mut findings: Vec<(String, Vec<u8>)> = Vec::new();

    // --- Randomized adversarial inputs --------------------------------------
    for i in 0..RANDOM_ITERS {
        // Deterministic, well-mixed seed per iteration.
        let mut rng = XorShift::new(0x9E37_79B9_7F4A_7C15u64 ^ (i.wrapping_add(1)).rotate_left(17));
        let len = (rng.next_u64() % 1024) as usize;
        let data = rng.next_bytes(len);

        total += 1;
        if check_parse(&data) {
            parse_panics += 1;
            record(&mut findings, "parse/random", &data);
        }
        if check_runtime(&data) {
            runtime_panics += 1;
            record(&mut findings, "runtime/random", &data);
        }
    }

    // --- Hand-crafted adversarial cases -------------------------------------
    for (label, src) in adversarial_cases() {
        let bytes = src.as_bytes().to_vec();
        total += 1;
        if check_parse(&bytes) {
            parse_panics += 1;
            record(&mut findings, &format!("parse/{label}"), &bytes);
        }
        if check_runtime(&bytes) {
            runtime_panics += 1;
            record(&mut findings, &format!("runtime/{label}"), &bytes);
        }
    }

    let panics = parse_panics + runtime_panics;

    // --- Summary ------------------------------------------------------------
    println!("fuzz smoke: {total} inputs, {panics} panics");
    if parse_panics > 0 {
        println!("  parser panics:  {parse_panics}");
    }
    if runtime_panics > 0 {
        println!("  runtime panics: {runtime_panics}");
    }

    if findings.is_empty() {
        println!("no panics observed — Axiom code withstood all adversarial input");
    } else {
        println!("--- findings (panicking inputs; up to {MAX_FINDINGS} shown) ---");
        for (label, data) in &findings {
            println!("[{}] {} bytes: {}", label, data.len(), to_hex(data));
        }
        println!(
            "These inputs panicked Axiom code. Document them under \
             'Known issues to fix in core' in fuzz/README.md and report them."
        );
    }
}
