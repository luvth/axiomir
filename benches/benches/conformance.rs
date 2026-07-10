//! Conformance-execution benchmarks (scenario 13).
//!
//! Every `*.axiom` fixture under `conformance/valid/` is parsed and executed via
//! `run_source`. Two views are provided: per-fixture execution cost (one bench per
//! file, so the distribution across fixtures is visible) and a single "execute all"
//! bench that runs the whole valid corpus in one iteration (total corpus cost).
//!
//! This measures the runtime's ability to execute the conformance corpus; it does
//! not re-check the expected JSON (that is the conformance harness's job).

use axiom_parser::parse_module;
use axiom_runtime::Runtime;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::path::Path;

/// Execute one conformance fixture. Fixtures that invoke an external tool
/// (`call`) need the live builtin tool plus the relevant capability, so we run
/// every fixture through a runtime that grants `tool:calculator` and installs
/// the builtin tool. For the pure (non-`call`) fixtures this is identical to
/// `run_source`, which uses the replay-only external executor.
fn exec_fixture(src: &str) -> axiom_runtime::Runtime {
    let ast = parse_module(src).unwrap();
    let mut rt = Runtime::new("fixture");
    rt.with_builtin_tool().grant("tool:calculator");
    rt.execute(&ast).unwrap();
    rt
}

fn load_fixtures() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../conformance/valid");
    let mut out = Vec::new();
    if !dir.exists() {
        eprintln!("conformance dir not found: {}", dir.display());
        return out;
    }
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "axiom"))
        .collect();
    entries.sort();
    for p in entries {
        let src = std::fs::read_to_string(&p).unwrap();
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        out.push((name, src));
    }
    out
}

fn bench_per_fixture(c: &mut Criterion) {
    let fixtures = load_fixtures();
    assert!(!fixtures.is_empty(), "no conformance fixtures found");
    for (name, src) in &fixtures {
        c.bench_with_input(BenchmarkId::new("fixture", name), src, |b, s| {
            b.iter(|| {
                let rt = exec_fixture(black_box(s));
                black_box(rt);
            })
        });
    }
}

fn bench_all(c: &mut Criterion) {
    let fixtures = load_fixtures();
    c.bench_function("execute_all_valid", |b| {
        b.iter(|| {
            for (_, src) in &fixtures {
                let rt = exec_fixture(black_box(src));
                black_box(rt);
            }
        })
    });
}

criterion_group!(benches, bench_per_fixture, bench_all);
criterion_main!(benches);
