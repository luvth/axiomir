//! Canonical-encoding and hashing benchmarks.
//!
//! Three distinct operations are exercised over already-built modules:
//!
//! * `axiom_encoding::canonical_bytes` on every claim (recursive canonical JSON);
//! * `axiom_encoding::content_id` (domain-separated SHA-256) on every claim;
//! * `axiom_core::Module::digest` — the full stable module digest over all claims,
//!   evidence, derivations, obligations and contradictions.
//!
//! All three are pure, deterministic, and machine-independent; absolute wall-clock
//! numbers still depend on the CPU (see README).

use axiom_benches::generators::{chain_source, wide_source};
use axiom_encoding::{canonical_bytes, content_id, Domain};
use axiom_runtime::run_source;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_canonical_claims_chain(c: &mut Criterion) {
    for depth in [10usize, 100, 1000] {
        let rt = run_source(&chain_source(depth)).unwrap();
        let claims: Vec<_> = rt.module.claims.values().cloned().collect();
        c.bench_with_input(
            BenchmarkId::new("canonical_bytes_chain", depth),
            &claims,
            |b, claims| {
                b.iter(|| {
                    for cl in claims {
                        let bytes = canonical_bytes(black_box(cl)).unwrap();
                        black_box(bytes);
                    }
                })
            },
        );
    }
}

fn bench_content_id_chain(c: &mut Criterion) {
    for depth in [10usize, 100, 1000] {
        let rt = run_source(&chain_source(depth)).unwrap();
        let claims: Vec<_> = rt.module.claims.values().cloned().collect();
        c.bench_with_input(
            BenchmarkId::new("content_id_chain", depth),
            &claims,
            |b, claims| {
                b.iter(|| {
                    for cl in claims {
                        let id = content_id(Domain::Claim, black_box(cl)).unwrap();
                        black_box(id);
                    }
                })
            },
        );
    }
}

fn bench_module_digest(c: &mut Criterion) {
    for depth in [10usize, 100, 1000] {
        let rt = run_source(&chain_source(depth)).unwrap();
        c.bench_with_input(
            BenchmarkId::new("module_digest_chain", depth),
            &rt,
            |b, rt| {
                b.iter(|| {
                    let d = rt.module.digest();
                    black_box(d);
                })
            },
        );
    }
    for width in [10usize, 100, 1000] {
        let rt = run_source(&wide_source(width)).unwrap();
        c.bench_with_input(
            BenchmarkId::new("module_digest_wide", width),
            &rt,
            |b, rt| {
                b.iter(|| {
                    let d = rt.module.digest();
                    black_box(d);
                })
            },
        );
    }
}

criterion_group!(
    benches,
    bench_canonical_claims_chain,
    bench_content_id_chain,
    bench_module_digest
);
criterion_main!(benches);
