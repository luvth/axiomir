//! Formatting benchmarks: idempotent format round-trip (`axiom_parser::format_source`)
//! over the same module families used by the parsing benchmarks.
//!
//! `format_source` parses then renders canonical text. The bench measures the
//! parse+format cost; the idempotence property (formatting the result again
//! yields identical text) is asserted once in setup, not inside the timed loop.

use axiom_benches::generators::{chain_source, small_source, wide_source};
use axiom_parser::format_source;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_format_small(c: &mut Criterion) {
    let src = small_source();
    // Idempotence check (outside the timed loop).
    let f1 = format_source(&src).unwrap();
    let f2 = format_source(&f1).unwrap();
    assert_eq!(f1, f2, "format_source must be idempotent");
    c.bench_function("format_small", |b| {
        b.iter(|| {
            let out = format_source(black_box(&src)).unwrap();
            black_box(out);
        })
    });
}

fn bench_format_chain(c: &mut Criterion) {
    for depth in [10usize, 100, 1000] {
        let src = chain_source(depth);
        let f1 = format_source(&src).unwrap();
        let f2 = format_source(&f1).unwrap();
        assert_eq!(f1, f2, "format_source must be idempotent (chain {depth})");
        c.bench_with_input(BenchmarkId::new("format_chain", depth), &src, |b, s| {
            b.iter(|| {
                let out = format_source(black_box(s)).unwrap();
                black_box(out);
            })
        });
    }
}

fn bench_format_wide(c: &mut Criterion) {
    for width in [10usize, 100, 1000] {
        let src = wide_source(width);
        let f1 = format_source(&src).unwrap();
        let f2 = format_source(&f1).unwrap();
        assert_eq!(f1, f2, "format_source must be idempotent (wide {width})");
        c.bench_with_input(BenchmarkId::new("format_wide", width), &src, |b, s| {
            b.iter(|| {
                let out = format_source(black_box(s)).unwrap();
                black_box(out);
            })
        });
    }
}

criterion_group!(
    benches,
    bench_format_small,
    bench_format_chain,
    bench_format_wide
);
criterion_main!(benches);
