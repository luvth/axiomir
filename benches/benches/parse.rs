//! Parsing benchmarks: small module, deep chains (depths 10/100/1000) and
//! wide fan-out graphs (widths 10/100/1000).
//!
//! Measures only lexical + syntactic parsing (`axiom_parser::parse_module`);
//! no semantic analysis, type checking, or execution is performed here.

use axiom_benches::generators::{chain_source, small_source, wide_source};
use axiom_parser::parse_module;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_parse_small(c: &mut Criterion) {
    let src = small_source();
    c.bench_function("parse_small", |b| {
        b.iter(|| {
            let ast = parse_module(black_box(&src)).unwrap();
            black_box(ast);
        })
    });
}

fn bench_parse_chain(c: &mut Criterion) {
    for depth in [10usize, 100, 1000] {
        let src = chain_source(depth);
        c.bench_with_input(BenchmarkId::new("parse_chain", depth), &src, |b, s| {
            b.iter(|| {
                let ast = parse_module(black_box(s)).unwrap();
                black_box(ast);
            })
        });
    }
}

fn bench_parse_wide(c: &mut Criterion) {
    for width in [10usize, 100, 1000] {
        let src = wide_source(width);
        c.bench_with_input(BenchmarkId::new("parse_wide", width), &src, |b, s| {
            b.iter(|| {
                let ast = parse_module(black_box(s)).unwrap();
                black_box(ast);
            })
        });
    }
}

criterion_group!(
    benches,
    bench_parse_small,
    bench_parse_chain,
    bench_parse_wide
);
criterion_main!(benches);
