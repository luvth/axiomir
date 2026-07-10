//! Replay benchmarks (scenario 12).
//!
//! Setup produces receipts by executing a module that invokes an external tool
//! (`call sum = tool.calculator(...) : rational cap "tool:calculator"`) using the
//! live builtin tool with the capability granted. The resulting receipts are then
//! reused for replay.
//!
//! Three paths are compared:
//!
//! * **cold replay** — a fresh runtime enters `replay_mode(receipts)`, then
//!   `execute` runs purely from the preloaded receipts (no external call);
//! * **warm replay** — the *same* runtime is executed a second time, with the
//!   receipts already present in its module (execute is a deterministic re-run);
//! * **live run** (reference) — a fresh runtime with the live builtin tool and the
//!   capability granted, executing the external tool directly.
//!
//! Cold and warm execute the same semantic work (replay never re-invokes the
//! external operation), so they are expected to be similar; the real saving versus
//! `live` is that replay skips the external tool call entirely. The comparison is
//! reported honestly rather than overstated.

use axiom_benches::generators::replay_source;
use axiom_core::Receipt;
use axiom_parser::parse_module;
use axiom_runtime::Runtime;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Execute the replay fixture with the live builtin tool and the required
/// capability. `run_source` cannot do this (it uses a replay-only external with
/// no capabilities), so we build the runtime explicitly here.
fn live_execute(src: &str) -> axiom_runtime::Runtime {
    let ast = parse_module(src).unwrap();
    let mut rt = Runtime::new("bench");
    rt.with_builtin_tool().grant("tool:calculator");
    rt.execute(&ast).unwrap();
    rt
}

struct ReplayFixture {
    ast: axiom_parser::ast::ModuleAst,
    receipts: Vec<Receipt>,
}

fn build_fixture() -> ReplayFixture {
    let src = replay_source();
    let ast = parse_module(&src).unwrap();
    let mut rt = Runtime::new("bench");
    rt.with_builtin_tool().grant("tool:calculator");
    rt.execute(&ast).unwrap();
    let receipts: Vec<Receipt> = rt.module.receipts.values().cloned().collect();
    assert!(!receipts.is_empty(), "expected at least one receipt");
    // Replaying must also verify the claim, proving the receipt reconstructs it.
    let mut cold = Runtime::new("bench");
    cold.replay_mode(receipts.clone()).grant("tool:calculator");
    cold.execute(&ast).unwrap();
    assert!(
        cold.verified_claims()
            .iter()
            .any(|id| *id == cold.claim("sum").unwrap()),
        "replay should verify `sum`"
    );
    ReplayFixture { ast, receipts }
}

fn bench_replay(c: &mut Criterion) {
    let fix = build_fixture();

    c.bench_function("cold_replay", |b| {
        b.iter(|| {
            let mut rt = Runtime::new("bench");
            rt.replay_mode(fix.receipts.clone())
                .grant("tool:calculator");
            rt.execute(black_box(&fix.ast)).unwrap();
        })
    });

    // Warm: execute once up front, then measure the second (deterministic re-run).
    let mut warm = Runtime::new("bench");
    warm.replay_mode(fix.receipts.clone())
        .grant("tool:calculator");
    warm.execute(&fix.ast).unwrap();
    c.bench_function("warm_replay", |b| {
        b.iter(|| {
            warm.execute(black_box(&fix.ast)).unwrap();
        })
    });

    // Live reference: external tool invoked each time.
    c.bench_function("live_run", |b| {
        b.iter(|| {
            let rt = live_execute(black_box(&replay_source()));
            black_box(rt);
        })
    });
}

criterion_group!(benches, bench_replay);
criterion_main!(benches);
