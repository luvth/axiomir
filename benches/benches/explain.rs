//! Explanation-query benchmarks.
//!
//! `Runtime::explain(label)` reconstructs the structural explanation of a claim
//! (its derivation, inputs, obligations, assumptions, evidence, and any
//! contradictions it participates in) purely from graph semantics. Measured on
//! the leaf of a deep derivation chain: the explain cost is independent of chain
//! depth (it follows one derivation path), so this also characterises the fixed
//! overhead of building an `Explanation`.

use axiom_benches::generators::chain_source;
use axiom_runtime::run_source;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_explain_chain(c: &mut Criterion) {
    for depth in [10usize, 100, 1000] {
        let rt = run_source(&chain_source(depth)).unwrap();
        let leaf = format!("d{depth}");
        c.bench_with_input(BenchmarkId::new("explain_leaf", depth), &rt, |b, rt| {
            b.iter(|| {
                let exp = rt.explain(black_box(&leaf)).unwrap();
                black_box(exp);
            })
        });
    }
}

criterion_group!(benches, bench_explain_chain);
criterion_main!(benches);
