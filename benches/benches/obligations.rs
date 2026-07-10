//! Obligation-discharge benchmarks.
//!
//! Modules are built from `obligations_source`: a chain of `div` derivations.
//! Each derivation (via the builtin `core.div` op) generates a *mandatory*
//! `numeric-bounds` obligation that is left `Pending`; a claim cannot verify
//! until that obligation is discharged.
//!
//! The measured block is the discharge + verify loop over every derived claim:
//! each pending mandatory obligation is discharged (by the trusted evidence
//! `e_proof`) and the claim is then verified. The block starts from a fresh
//! clone of the built module each iteration so the discharge is not "free" from
//! prior state; the clone cost is included (it is O(module size) and small
//! relative to the discharge work for the larger sizes).

use axiom_benches::generators::obligations_source;
use axiom_core::ObligationState;
use axiom_runtime::run_source;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

struct ObligPlan {
    module: axiom_core::Module,
    /// (claim id, obligation ids of that claim) for every derived `q{i}`.
    claims: Vec<(axiom_core::Id, Vec<axiom_core::Id>)>,
    evidence: axiom_core::Id,
}

fn build_plan(k: usize) -> ObligPlan {
    let rt = run_source(&obligations_source(k)).unwrap();
    let evidence = rt.evidence("e_proof").unwrap();
    let mut claims = Vec::new();
    for i in 0..k {
        let cid = rt.claim(&format!("q{i}")).unwrap();
        let obls = rt.module.claims[&cid].obligations.clone();
        claims.push((cid, obls));
    }
    ObligPlan {
        module: rt.module.clone(),
        claims,
        evidence,
    }
}

fn discharge_and_verify(plan: &ObligPlan) {
    let mut m = plan.module.clone();
    for (cid, obls) in &plan.claims {
        for oid in obls {
            // Only discharge obligations still pending (the mandatory numeric-bounds one).
            if m.obligations[oid].state == ObligationState::Pending {
                m.discharge(
                    oid.clone(),
                    plan.evidence.clone(),
                    ObligationState::Satisfied,
                )
                .unwrap();
            }
        }
        m.verify(cid.clone()).unwrap();
    }
    black_box(m);
}

fn bench_obligations(c: &mut Criterion) {
    for k in [10usize, 50, 200] {
        let plan = build_plan(k);
        // Sanity: before discharge nothing should have verified, after it should.
        let pre_verified = plan
            .module
            .claims
            .values()
            .filter(|c| c.status == axiom_core::ClaimStatus::Verified)
            .count();
        assert_eq!(pre_verified, 0, "no claim should verify before discharge");
        c.bench_with_input(
            BenchmarkId::new("discharge_and_verify", k),
            &plan,
            |b, plan| b.iter(|| discharge_and_verify(black_box(plan))),
        );
    }
}

criterion_group!(benches, bench_obligations);
criterion_main!(benches);
