//! Dataset generators for the Axiom IR benchmark suite.
//!
//! Every generator returns source text for the Axiom textual language. Bench
//! targets parse and execute that text. Keeping generators as pure string
//! builders (no dependency on the runtime) makes them trivially shareable and
//! keeps this lib dependency-free.
//!
//! The labelled shapes deliberately cover the dependency topologies the spec
//! cares about:
//!
//! * **chains** — a single deep linear derivation path (`chain_source`);
//! * **wide fan-out** — one shared premise feeding many independent derivations
//!   (`wide_source`);
//! * **parallel chains** — many independent chains sharing a common root, used to
//!   show incremental recomputation beating full re-execution (`parallel_chains_source`);
//! * **obligation-heavy** — sequential `div` derivations each generating a
//!   mandatory `numeric-bounds` obligation (`obligations_source`);
//! * **disjoint intervals** — many pairwise-disjoint intervals for contradiction
//!   detection (`contradictions_source`);
//! * **many contexts** and **dense evidence** — branch/evidence-heavy modules for
//!   graph construction and encoding stress (`many_contexts_source`,
//!   `dense_evidence_source`).

#![allow(dead_code)]

use std::fmt::Write;

/// Deep linear dependency chain of `depth` derivations rooted at a premise.
pub fn chain_source(depth: usize) -> String {
    let mut s = String::new();
    s.push_str("module bench_chain \"1\"\n");
    s.push_str("assert base = 1 : rational\n");
    for i in 1..=depth {
        let prev = if i == 1 {
            "base".to_string()
        } else {
            format!("d{}", i - 1)
        };
        writeln!(s, "derive d{i} = add({prev}, base) : rational").unwrap();
    }
    writeln!(s, "verify d1").unwrap();
    s
}

/// Wide fan-out: one shared premise feeding `width` independent derivations.
pub fn wide_source(width: usize) -> String {
    let mut s = String::new();
    s.push_str("module bench_wide \"1\"\n");
    s.push_str("assert base = 1 : rational\n");
    for k in 0..width {
        writeln!(s, "assert c{k} = 1 : rational").unwrap();
    }
    for k in 0..width {
        writeln!(s, "derive w{k} = add(base, c{k}) : rational").unwrap();
    }
    writeln!(s, "verify w0").unwrap();
    s
}

/// `n` independent chains of length `len`, all sharing a common `base` premise.
/// Changing one chain's internal premise (`b{k}`) only invalidates that chain,
/// which is what lets incremental recomputation beat a full re-execution.
pub fn parallel_chains_source(n: usize, len: usize) -> String {
    let mut s = String::new();
    s.push_str("module bench_parallel \"1\"\n");
    s.push_str("assert base = 1 : rational\n");
    for k in 0..n {
        writeln!(s, "assert b{k} = 1 : rational").unwrap();
        writeln!(s, "derive b{k}_1 = add(base, b{k}) : rational").unwrap();
        for j in 2..=len {
            writeln!(s, "derive b{k}_{j} = add(b{k}_{}, b{k}) : rational", j - 1).unwrap();
        }
    }
    writeln!(s, "verify b0_1").unwrap();
    s
}

/// `k` independent `div` derivations off a single premise `a`. Each derivation
/// (via the builtin `core.div` op) generates a *mandatory* `numeric-bounds`
/// obligation that is left `Pending`; a claim cannot verify until that
/// obligation is discharged. A single trusted evidence item `e_proof` is
/// supplied so the obligations can be discharged at the core level.
///
/// The derivations are independent (`div(a, c{i})`, not a chain) so the exact
/// rational denominator never grows with `k`; this keeps the workload bounded by
/// the number of obligations rather than by i128 denominator capacity.
pub fn obligations_source(k: usize) -> String {
    let mut s = String::new();
    s.push_str("module bench_obligations \"1\"\n");
    s.push_str("evidence e_proof \"text/plain\" \"discharge evidence\" trust=trusted\n");
    s.push_str("assert a = 100 : rational\n");
    for i in 0..k {
        writeln!(s, "assert c{i} = 2 : rational").unwrap();
        writeln!(s, "derive q{i} = div(a, c{i}) : rational").unwrap();
    }
    s
}

/// `n` observed pairwise-disjoint numeric intervals. `detect_contradictions`
/// is O(n^2) pairwise; every pair is disjoint, so it produces C(n,2) records.
pub fn contradictions_source(n: usize) -> String {
    let mut s = String::new();
    s.push_str("module bench_contradictions \"1\"\n");
    for i in 0..n {
        writeln!(
            s,
            "observe x{i} = interval({}, {}) : interval",
            2 * i,
            2 * i
        )
        .unwrap();
    }
    s
}

/// Source whose execution invokes an external tool via `call`, used by the
/// replay benchmarks. The tool requires the `tool:calculator` capability.
pub fn replay_source() -> String {
    "module bench_replay \"1\"\n\
     assert a = 2 : rational\n\
     assert b = 3 : rational\n\
     call sum = tool.calculator(a, b) : rational cap \"tool:calculator\"\n\
     verify sum\n"
        .to_string()
}

/// Many sibling contexts branching from `root`, each carrying one derivation.
pub fn many_contexts_source(n: usize) -> String {
    let mut s = String::new();
    s.push_str("module bench_contexts \"1\"\n");
    s.push_str("assert base = 1 : rational\n");
    for k in 0..n {
        writeln!(s, "branch ctx{k} from root").unwrap();
        writeln!(s, "derive d{k} = add(base, base) : rational ctx ctx{k}").unwrap();
    }
    s
}

/// Dense evidence: `n` evidence items each referenced by its own assertion.
pub fn dense_evidence_source(n: usize) -> String {
    let mut s = String::new();
    s.push_str("module bench_evidence \"1\"\n");
    for i in 0..n {
        writeln!(
            s,
            "evidence e{i} \"text/plain\" \"evidence {i}\" trust=trusted"
        )
        .unwrap();
    }
    for i in 0..n {
        writeln!(s, "assert a{i} = {i} : rational evidence [e{i}]").unwrap();
    }
    s
}

/// Small hand-written module, handy as a quick smoke source.
pub fn small_source() -> String {
    "module bench \"1\"\nassert a = 2 : rational\nderive out = add(a, a) : rational\nverify out\n"
        .to_string()
}
