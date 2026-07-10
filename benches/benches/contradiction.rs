//! Contradiction-detection benchmarks.
//!
//! Modules are built from `contradictions_source`: `n` observed pairwise-disjoint
//! numeric intervals. `Runtime::detect_contradictions("root")` scans the context's
//! claims pairwise (O(n^2)) and records a first-class contradiction for every
//! disjoint pair, producing C(n,2) contradiction nodes.
//!
//! `detect_contradictions` mutates the module (it records contradiction nodes), so
//! each iteration resets the contradiction state before re-detecting. This isolates
//! the detection cost. A second bench measures collecting the full contradiction
//! set via `Runtime::contradictions()` on an already-detected module.

use axiom_benches::generators::contradictions_source;
use axiom_runtime::run_source;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn reset_contradictions(rt: &mut axiom_runtime::Runtime) {
    rt.module.contradictions.clear();
    for ctx in rt.module.contexts.values_mut() {
        ctx.contradictions.clear();
    }
}

fn bench_detect(c: &mut Criterion) {
    for n in [10usize, 30, 100] {
        let mut rt = run_source(&contradictions_source(n)).unwrap();
        // Expected contradiction count for the report.
        let expected = n * (n - 1) / 2;
        c.bench_function(&format!("detect_disjoint_intervals/{n}"), |b| {
            b.iter(|| {
                reset_contradictions(&mut rt);
                let count = rt.detect_contradictions("root").unwrap();
                black_box(count);
            })
        });
        eprintln!("contradiction bench n={n}: expected C(n,2)={expected} disjoint records");
    }
}

fn bench_list(c: &mut Criterion) {
    for n in [10usize, 30, 100] {
        let mut rt = run_source(&contradictions_source(n)).unwrap();
        rt.detect_contradictions("root").unwrap();
        c.bench_function(&format!("list_contradictions/{n}"), |b| {
            b.iter(|| {
                let all = rt.contradictions();
                black_box(all.len());
            })
        });
    }
}

criterion_group!(benches, bench_detect, bench_list);
criterion_main!(benches);
