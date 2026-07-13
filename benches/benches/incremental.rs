//! Incremental-recomputation benchmarks (scenarios 9 and 10).
//!
//! Scenario 9 — **invalidation**: build a module, change a premise, then run
//! `invalidate_and_recompute`. Measured on deep chains and wide fan-out graphs;
//! the affected set size (invalidated + recomputed claims) is reported.
//!
//! Scenario 10 — **incremental recompute vs full re-execution**: on a module of
//! many parallel chains sharing a common root, change a premise and compare:
//!
//! * *incremental* = `invalidate_and_recompute` (recomputes only the claims that
//!   transitively depend on the changed premise);
//! * *full* = `full_recompute` (re-runs every derivation from scratch).
//!
//! Two cases are reported honestly:
//!
//! * changing a *private* premise of one chain — incremental recomputes only that
//!   chain, full recomputes everything (incremental wins);
//! * changing the *shared root* — incremental must recompute everything too
//!   (incremental ≈ full; no free lunch).
//!
//! A manual `std::time` comparison at the end prints both wall-clock costs and the
//! affected-set sizes so the relationship is explicit rather than inferred from
//! separate criterion runs.

use axiom_benches::generators::{chain_source, parallel_chains_source, wide_source};
use axiom_core::registry::BuiltinExecutor;
use axiom_core::Value;
use axiom_incremental::{full_recompute, invalidate_and_recompute};
use axiom_runtime::run_source;
use axiom_types::Num;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Instant;

fn change_premise(m: &mut axiom_core::Module, id: &axiom_core::Id) {
    m.claims.get_mut(id).unwrap().value = Value::Num(Num::Int(2));
}

fn bench_invalidate_chain(c: &mut Criterion) {
    for depth in [100usize, 1000] {
        let rt = run_source(&chain_source(depth)).unwrap();
        let base = rt.claim("base").unwrap();
        let module = rt.module.clone();
        c.bench_with_input(
            BenchmarkId::new("invalidate_chain", depth),
            &module,
            |b, module| {
                b.iter(|| {
                    let mut m = module.clone();
                    change_premise(&mut m, &base);
                    let report = invalidate_and_recompute(&mut m, &base, &BuiltinExecutor, &[]);
                    black_box(report);
                })
            },
        );
        // Report the affected set size for the chain (changing the root touches all).
        let mut m = module.clone();
        change_premise(&mut m, &base);
        let report = invalidate_and_recompute(&mut m, &base, &BuiltinExecutor, &[]);
        eprintln!(
            "invalidate_chain depth={depth}: invalidated={} recomputed={} preserved={} (changes={})",
            report.invalidated.len(),
            report.recomputed.len(),
            report.preserved.len(),
            report.changes.len(),
        );
    }
}

fn bench_invalidate_wide(c: &mut Criterion) {
    for width in [100usize, 1000] {
        let rt = run_source(&wide_source(width)).unwrap();
        let base = rt.claim("base").unwrap();
        let module = rt.module.clone();
        c.bench_with_input(
            BenchmarkId::new("invalidate_wide", width),
            &module,
            |b, module| {
                b.iter(|| {
                    let mut m = module.clone();
                    change_premise(&mut m, &base);
                    let report = invalidate_and_recompute(&mut m, &base, &BuiltinExecutor, &[]);
                    black_box(report);
                })
            },
        );
        let mut m = module.clone();
        change_premise(&mut m, &base);
        let report = invalidate_and_recompute(&mut m, &base, &BuiltinExecutor, &[]);
        eprintln!(
            "invalidate_wide width={width}: invalidated={} recomputed={} preserved={} (changes={})",
            report.invalidated.len(),
            report.recomputed.len(),
            report.preserved.len(),
            report.changes.len(),
        );
    }
}

fn bench_incremental_vs_full(c: &mut Criterion, label: &str, premise: &str, n: usize, len: usize) {
    let rt = run_source(&parallel_chains_source(n, len)).unwrap();
    let pid = rt.claim(premise).unwrap();
    let module = rt.module.clone();
    let total_derivations = n * len;

    c.bench_with_input(
        BenchmarkId::new("incremental", format!("{label}_{n}x{len}")),
        &module,
        |b, module| {
            b.iter(|| {
                let mut m = module.clone();
                change_premise(&mut m, &pid);
                let report = invalidate_and_recompute(&mut m, &pid, &BuiltinExecutor, &[]);
                black_box(report);
            })
        },
    );
    c.bench_with_input(
        BenchmarkId::new("full", format!("{label}_{n}x{len}")),
        &module,
        |b, module| {
            b.iter(|| {
                let mut m = module.clone();
                change_premise(&mut m, &pid);
                let count = full_recompute(&mut m, &BuiltinExecutor, &[]);
                black_box(count);
            })
        },
    );

    // One-shot manual comparison printing absolute numbers and affected sizes.
    let iters = 20;
    let mut inc_report = None;
    let start = Instant::now();
    for _ in 0..iters {
        let mut m = module.clone();
        change_premise(&mut m, &pid);
        inc_report = Some(invalidate_and_recompute(
            &mut m,
            &pid,
            &BuiltinExecutor,
            &[],
        ));
    }
    let inc_elapsed = start.elapsed();
    let mut full_count = 0;
    let start = Instant::now();
    for _ in 0..iters {
        let mut m = module.clone();
        change_premise(&mut m, &pid);
        full_count = full_recompute(&mut m, &BuiltinExecutor, &[]);
    }
    let full_elapsed = start.elapsed();
    let inc = inc_report.unwrap();
    eprintln!(
        "incremental_vs_full [{label}] {n}x{len}: total_derivations={total_derivations} \
         incremental recomputed={} (affected={}), full recomputed={full_count} \
         | {iters} iters: incremental={:?} full={:?}",
        inc.recomputed.len(),
        inc.invalidated.len() + inc.recomputed.len(),
        inc_elapsed,
        full_elapsed,
    );
}

fn bench_incremental(c: &mut Criterion) {
    // Private premise of chain 0: incremental wins.
    bench_incremental_vs_full(c, "private", "b0", 64, 32);
    // Shared root: incremental must recompute everything too.
    bench_incremental_vs_full(c, "shared", "base", 64, 32);
}

criterion_group!(
    benches,
    bench_invalidate_chain,
    bench_invalidate_wide,
    bench_incremental
);
criterion_main!(benches);
