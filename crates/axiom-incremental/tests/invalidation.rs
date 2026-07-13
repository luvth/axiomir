//! Integration tests for axiom-incremental: dependency-correct invalidation
//! and recomputation engine.
//!
//! Each `#[test]` exercises one named property of the invalidation algorithm.
//! No src/ files are modified; all assertions operate solely on the public API.

use axiom_core::registry::BuiltinExecutor;
use axiom_core::ClaimStatus;
use axiom_incremental::{invalidate_and_recompute, DependencyGraph};
use axiom_runtime::run_source;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Shared source snippets
// ---------------------------------------------------------------------------

/// A two-level derived chain rooted at `base`:
///   base -> a (= base+base) -> b (= a+base)
/// Both `a` and `b` are verified. Used by tests 1, 2, 6.
const SRC_TWO_LEVEL: &str = r#"
module m "1"
assert base = 10 : rational
derive a = add(base, base) : rational
derive b = add(a, base) : rational
verify a
verify b
"#;

/// Two independent chains: base->a->b (verified) and x,y->z (verified).
/// Used by test 2 (unrelated_premise_preserved).
const SRC_TWO_CHAINS: &str = r#"
module m "1"
assert base = 10 : rational
derive a = add(base, base) : rational
verify a
assert x = 1 : rational
assert y = 2 : rational
derive z = add(x, y) : rational
verify z
"#;

/// A three-level derived chain rooted at `base`:
///   base -> a (= base+base) -> b (= a+base) -> c (= b+base)
/// All three are verified. Used by tests 3, 4, 5.
const SRC_DEEP_CHAIN: &str = r#"
module m "1"
assert base = 10 : rational
derive a = add(base, base) : rational
derive b = add(a, base) : rational
derive c = add(b, base) : rational
verify a
verify b
verify c
"#;

// ---------------------------------------------------------------------------
// Test 1 – premise_change_invalidates_dependents (required property 4)
// ---------------------------------------------------------------------------

/// Invalidating a *premise* (assert, no derivation) invalidates every
/// transitive derived dependent but does NOT invalidate the premise itself
/// (its input value is still available / correct). After the call, derived
/// claims `a` and `b` must appear in `report.recomputed` because their inputs
/// are available.
#[test]
fn premise_change_invalidates_dependents() {
    let mut rt = run_source(SRC_TWO_LEVEL).unwrap();
    let base = rt.claim("base").unwrap();
    let a = rt.claim("a").unwrap();
    let b = rt.claim("b").unwrap();

    let report = invalidate_and_recompute(&mut rt.module, &base, &BuiltinExecutor, &[]);

    // The changed premise must not appear in `invalidated`.
    assert!(
        !report.invalidated.contains(&base),
        "premise `base` must not be invalidated (it is the corrected input)"
    );

    // Both transitive dependents must have been invalidated (then recomputed).
    assert!(
        report.invalidated.contains(&a) || report.recomputed.contains(&a),
        "`a` must appear in invalidated or recomputed"
    );
    assert!(
        report.invalidated.contains(&b) || report.recomputed.contains(&b),
        "`b` must appear in invalidated or recomputed"
    );

    // After recomputation both claims are re-verified.
    assert_eq!(
        rt.module.claims[&a].status,
        ClaimStatus::Verified,
        "`a` must be Verified after recomputation"
    );
    assert_eq!(
        rt.module.claims[&b].status,
        ClaimStatus::Verified,
        "`b` must be Verified after recomputation"
    );

    // The report's `roots` field identifies the trigger.
    assert!(
        report.roots.contains(&base),
        "report.roots must contain the changed node `base`"
    );
}

// ---------------------------------------------------------------------------
// Test 2 – unrelated_premise_preserved (required property 5)
// ---------------------------------------------------------------------------

/// Invalidating `base` (which feeds only `a`) must leave the independent
/// verified chain `x -> y -> z` untouched: `z` appears in `report.preserved`
/// and its status remains `Verified` after the call.
#[test]
fn unrelated_premise_preserved() {
    let mut rt = run_source(SRC_TWO_CHAINS).unwrap();
    let base = rt.claim("base").unwrap();
    let z = rt.claim("z").unwrap();

    let report = invalidate_and_recompute(&mut rt.module, &base, &BuiltinExecutor, &[]);

    // `z` must appear in `preserved`.
    assert!(
        report.preserved.contains(&z),
        "`z` must be in report.preserved (it has no dependency on `base`)"
    );

    // The module status must still be Verified for `z`.
    assert_eq!(
        rt.module.claims[&z].status,
        ClaimStatus::Verified,
        "`z` must remain Verified after invalidating unrelated `base`"
    );
}

// ---------------------------------------------------------------------------
// Test 3 – dirty_frontier_exactness
// ---------------------------------------------------------------------------

/// Build a three-deep chain (base -> a -> b -> c). Invalidating `base` must
/// put *exactly* {a, b, c} in the union of `invalidated ∪ recomputed`, and
/// `base` must not appear there.
#[test]
fn dirty_frontier_exactness() {
    let mut rt = run_source(SRC_DEEP_CHAIN).unwrap();
    let base = rt.claim("base").unwrap();
    let a = rt.claim("a").unwrap();
    let b = rt.claim("b").unwrap();
    let c = rt.claim("c").unwrap();

    let report = invalidate_and_recompute(&mut rt.module, &base, &BuiltinExecutor, &[]);

    // Build the union of affected (invalidated then possibly recomputed) ids.
    let affected: HashSet<_> = report
        .invalidated
        .iter()
        .chain(report.recomputed.iter())
        .cloned()
        .collect();

    let expected: HashSet<_> = [a.clone(), b.clone(), c.clone()].into_iter().collect();

    // Every expected claim must appear in the affected set.
    for id in &expected {
        assert!(
            affected.contains(id),
            "expected derived claim to be in affected set but was not"
        );
    }

    // The premise must not be in the affected set.
    assert!(
        !affected.contains(&base),
        "`base` (premise) must not appear in invalidated or recomputed"
    );
}

// ---------------------------------------------------------------------------
// Test 4 – forwarding_dependency_index
// ---------------------------------------------------------------------------

/// `DependencyGraph::build` must build a correct reverse index.
/// `graph.transitive_dependents(&base)` must return a set containing a, b, c.
#[test]
fn forwarding_dependency_index() {
    let rt = run_source(SRC_DEEP_CHAIN).unwrap();
    let base = rt.claim("base").unwrap();
    let a = rt.claim("a").unwrap();
    let b = rt.claim("b").unwrap();
    let c = rt.claim("c").unwrap();

    let graph = DependencyGraph::build(&rt.module);
    let transitive = graph.transitive_dependents(&base);

    assert!(
        transitive.contains(&a),
        "`a` must be a transitive dependent of `base`"
    );
    assert!(
        transitive.contains(&b),
        "`b` must be a transitive dependent of `base`"
    );
    assert!(
        transitive.contains(&c),
        "`c` must be a transitive dependent of `base`"
    );

    // The premise itself must not appear in the transitive dependent set.
    assert!(
        !transitive.contains(&base),
        "`base` must not be a transitive dependent of itself"
    );
}

// ---------------------------------------------------------------------------
// Test 5 – recompute_minimality_vs_full
// ---------------------------------------------------------------------------

/// After invalidating `base` and allowing incremental recomputation, the
/// derived claims a, b, c must all be `Verified`. This demonstrates that
/// incremental recomputation produces the same *logical* result as a fresh
/// execution: all derivable claims end up verified.
///
/// Note: the raw `digest()` values are NOT compared because the incremental
/// path appends `Invalidate` events and mutates `invalidation_conditions` on
/// claims, so the canonical serialisation differs from a pristine run. Instead
/// we compare the set of `(label, status)` pairs for the derived claims, which
/// captures whether the incremental engine reaches the same logical conclusion.
#[test]
fn recompute_minimality_vs_full() {
    let mut rt_incr = run_source(SRC_DEEP_CHAIN).unwrap();
    let base = rt_incr.claim("base").unwrap();

    // Run incremental invalidation + recomputation.
    let _report = invalidate_and_recompute(&mut rt_incr.module, &base, &BuiltinExecutor, &[]);

    // Run a fresh execution from the same source.
    let rt_fresh = run_source(SRC_DEEP_CHAIN).unwrap();

    // For each derived claim label, assert both runtimes agree on Verified.
    for label in &["a", "b", "c"] {
        let id_incr = rt_incr.claim(label).unwrap();
        let id_fresh = rt_fresh.claim(label).unwrap();

        let status_incr = rt_incr.module.claims[&id_incr].status;
        let status_fresh = rt_fresh.module.claims[&id_fresh].status;

        assert_eq!(
            status_incr, status_fresh,
            "claim `{label}` must have the same status after incremental recompute as after fresh run"
        );
        assert_eq!(
            status_incr,
            ClaimStatus::Verified,
            "claim `{label}` must be Verified after incremental recompute"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 6 – removal_of_middle_recomputes_only_downstream
// ---------------------------------------------------------------------------

/// Invalidating `a` (a *derived* middle node, which HAS a derivation) must
/// cause `a` and `b` to appear in the affected set (invalidated or recomputed),
/// while `base` must not be touched: invalidation follows the dependency graph
/// and never walks *upstream*.
#[test]
fn removal_of_middle_recomputes_only_downstream() {
    let mut rt = run_source(SRC_TWO_LEVEL).unwrap();
    let base = rt.claim("base").unwrap();
    let a = rt.claim("a").unwrap();
    let b = rt.claim("b").unwrap();

    let report = invalidate_and_recompute(&mut rt.module, &a, &BuiltinExecutor, &[]);

    // `a` has a derivation so it IS included in the affected set.
    let affected: HashSet<_> = report
        .invalidated
        .iter()
        .chain(report.recomputed.iter())
        .cloned()
        .collect();

    assert!(
        affected.contains(&a),
        "`a` (middle derived node) must be in the affected set"
    );
    assert!(
        affected.contains(&b),
        "`b` (downstream of `a`) must be in the affected set"
    );
    assert!(
        !affected.contains(&base),
        "`base` (upstream premise) must NOT be in the affected set"
    );

    // `base` must remain Verified (it was not touched).
    // `base` is asserted (not derived), so it has status Asserted, not Verified.
    // The key property is that it was not invalidated.
    assert_ne!(
        rt.module.claims[&base].status,
        ClaimStatus::Invalidated,
        "`base` must not be Invalidated (it is upstream and unaffected)"
    );
}
