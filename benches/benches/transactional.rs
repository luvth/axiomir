//! Transactional-execution benchmarks: a *full* execution (parse + execute from
//! scratch, the normal path) versus *re-execution* of an already-parsed AST.
//!
//! Both paths fully re-run the module's semantics (execution is deterministic and
//! re-runnable; re-executing does not silently reuse prior results). The point of
//! the comparison is to isolate the lexical/syntactic parse cost from the
//! semantic execution cost: `full` includes `parse_module`, `re_exec` executes a
//! pre-parsed `ModuleAst`. Re-execution is expected to be cheaper purely because
//! it skips lexing/parsing. Neither path performs incremental caching — that is
//! measured separately in `incremental`.

use axiom_benches::generators::{chain_source, small_source, wide_source};
use axiom_parser::parse_module;
use axiom_runtime::Runtime;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_transactional_small(c: &mut Criterion) {
    let src = small_source();
    let ast = parse_module(&src).unwrap();
    c.bench_function("transactional_small_full", |b| {
        b.iter(|| {
            let rt = run_source_bench(black_box(&src));
            black_box(rt);
        })
    });
    c.bench_function("transactional_small_re_exec", |b| {
        b.iter(|| {
            let mut rt = Runtime::new("bench");
            rt.execute(black_box(&ast)).unwrap();
            black_box(rt);
        })
    });
}

fn bench_transactional_chain(c: &mut Criterion) {
    for depth in [10usize, 100, 1000] {
        let src = chain_source(depth);
        let ast = parse_module(&src).unwrap();
        c.bench_with_input(
            BenchmarkId::new("transactional_chain_full", depth),
            &src,
            |b, s| {
                b.iter(|| {
                    let rt = run_source_bench(black_box(s));
                    black_box(rt);
                })
            },
        );
        c.bench_with_input(
            BenchmarkId::new("transactional_chain_re_exec", depth),
            &ast,
            |b, a| {
                b.iter(|| {
                    let mut rt = Runtime::new("bench");
                    rt.execute(black_box(a)).unwrap();
                    black_box(rt);
                })
            },
        );
    }
}

fn bench_transactional_wide(c: &mut Criterion) {
    for width in [10usize, 100, 1000] {
        let src = wide_source(width);
        let ast = parse_module(&src).unwrap();
        c.bench_with_input(
            BenchmarkId::new("transactional_wide_full", width),
            &src,
            |b, s| {
                b.iter(|| {
                    let rt = run_source_bench(black_box(s));
                    black_box(rt);
                })
            },
        );
        c.bench_with_input(
            BenchmarkId::new("transactional_wide_re_exec", width),
            &ast,
            |b, a| {
                b.iter(|| {
                    let mut rt = Runtime::new("bench");
                    rt.execute(black_box(a)).unwrap();
                    black_box(rt);
                })
            },
        );
    }
}

/// Local helper identical to `run_source` but named to avoid clashing with the
/// public re-export in readable call sites above.
fn run_source_bench(src: &str) -> axiom_runtime::Runtime {
    let ast = parse_module(src).unwrap();
    let mut rt = Runtime::new("bench");
    rt.execute(&ast).unwrap();
    rt
}

criterion_group!(
    benches,
    bench_transactional_small,
    bench_transactional_chain,
    bench_transactional_wide
);
criterion_main!(benches);
