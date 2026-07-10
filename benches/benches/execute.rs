//! Execution / type-checking benchmarks: `axiom_runtime::run_source` on the same
//! module families used by the parsing benchmarks.
//!
//! `run_source` parses the source, executes every statement against the trusted
//! core (type checking each derivation's inputs, discharging auto-satisfied
//! obligations, verifying where requested), and returns the runtime. This is the
//! end-to-end "build + type check + execute" cost for a module.

use axiom_benches::generators::{chain_source, small_source, wide_source};
use axiom_runtime::run_source;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_execute_small(c: &mut Criterion) {
    let src = small_source();
    c.bench_function("execute_small", |b| {
        b.iter(|| {
            let rt = run_source(black_box(&src)).unwrap();
            black_box(rt);
        })
    });
}

fn bench_execute_chain(c: &mut Criterion) {
    for depth in [10usize, 100, 1000] {
        let src = chain_source(depth);
        c.bench_with_input(BenchmarkId::new("execute_chain", depth), &src, |b, s| {
            b.iter(|| {
                let rt = run_source(black_box(s)).unwrap();
                black_box(rt);
            })
        });
    }
}

fn bench_execute_wide(c: &mut Criterion) {
    for width in [10usize, 100, 1000] {
        let src = wide_source(width);
        c.bench_with_input(BenchmarkId::new("execute_wide", width), &src, |b, s| {
            b.iter(|| {
                let rt = run_source(black_box(s)).unwrap();
                black_box(rt);
            })
        });
    }
}

criterion_group!(
    benches,
    bench_execute_small,
    bench_execute_chain,
    bench_execute_wide
);
criterion_main!(benches);
