//! Dependency-graph construction benchmarks.
//!
//! `axiom_incremental::DependencyGraph::build` computes forward and reverse
//! dependency edges over every claim (via `Claim::depends_on`). `reverse_dependents`
//! returns the same reverse map. These run over large graphs: a deep chain, a wide
//! fan-out, a many-context tree, and a set of parallel chains.

use axiom_benches::generators::{
    chain_source, many_contexts_source, parallel_chains_source, wide_source,
};
use axiom_incremental::{reverse_dependents, DependencyGraph};
use axiom_runtime::run_source;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_graph_chain(c: &mut Criterion) {
    for depth in [100usize, 1000] {
        let rt = run_source(&chain_source(depth)).unwrap();
        c.bench_with_input(
            BenchmarkId::new("graph_build_chain", depth),
            &rt,
            |b, rt| {
                b.iter(|| {
                    let g = DependencyGraph::build(black_box(&rt.module));
                    black_box(g);
                })
            },
        );
        c.bench_with_input(
            BenchmarkId::new("reverse_dependents_chain", depth),
            &rt,
            |b, rt| {
                b.iter(|| {
                    let r = reverse_dependents(black_box(&rt.module));
                    black_box(r);
                })
            },
        );
    }
}

fn bench_graph_wide(c: &mut Criterion) {
    for width in [100usize, 1000] {
        let rt = run_source(&wide_source(width)).unwrap();
        c.bench_with_input(BenchmarkId::new("graph_build_wide", width), &rt, |b, rt| {
            b.iter(|| {
                let g = DependencyGraph::build(black_box(&rt.module));
                black_box(g);
            })
        });
    }
}

fn bench_graph_contexts(c: &mut Criterion) {
    for n in [100usize, 500, 1000] {
        let rt = run_source(&many_contexts_source(n)).unwrap();
        c.bench_with_input(BenchmarkId::new("graph_build_contexts", n), &rt, |b, rt| {
            b.iter(|| {
                let g = DependencyGraph::build(black_box(&rt.module));
                black_box(g);
            })
        });
    }
}

fn bench_graph_parallel(c: &mut Criterion) {
    // n chains of length len: a large, mostly-independent dependency graph.
    let rt = run_source(&parallel_chains_source(64, 32)).unwrap();
    c.bench_function("graph_build_parallel_64x32", |b| {
        b.iter(|| {
            let g = DependencyGraph::build(black_box(&rt.module));
            black_box(g);
        })
    });
}

criterion_group!(
    benches,
    bench_graph_chain,
    bench_graph_wide,
    bench_graph_contexts,
    bench_graph_parallel
);
criterion_main!(benches);
