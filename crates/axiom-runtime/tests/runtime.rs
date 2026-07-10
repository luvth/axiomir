// Integration tests for axiom-runtime.
// Tests cover: deterministic replay, receipt tampering, capability gating,
// context-local verification, contradiction preservation, and quarantine.

use axiom_core::ClaimStatus;
use axiom_parser::parse_module;
use axiom_runtime::Runtime;

// ---------------------------------------------------------------------------
// Shared source strings
// ---------------------------------------------------------------------------

const SRC_DEMO3: &str = r#"module demo3 "1"
assert a = 2 : rational
assert b = 3 : rational
call sum = tool.calculator(a, b) : rational cap "tool:calculator"
"#;

const SRC_DEMO2B: &str = r#"module demo2b "1"
observe low = q(15.0 "C") : quantity
observe high = q(28.0 "C") : quantity
contradict low high as disjoint-interval
branch optimistic from root
derive mid = qadd(low, high) : quantity ctx optimistic
verify mid
"#;

const SRC_DEMO2: &str = r#"module demo2 "1"
observe low = q(15.0 "C") : quantity
observe high = q(28.0 "C") : quantity
contradict low high as disjoint-interval
"#;

// ---------------------------------------------------------------------------
// Test 1 — deterministic_replay_identical_digest (required property 3)
// ---------------------------------------------------------------------------

#[test]
fn deterministic_replay_identical_digest() {
    // (a) Live run: capability granted, builtin tool installed.
    let ast = parse_module(SRC_DEMO3).expect("parse SRC_DEMO3");
    let mut live = Runtime::new("demo3");
    live.grant("tool:calculator");
    live.with_builtin_tool();
    live.execute(&ast).expect("live execute");

    let digest_live = live.digest();
    let receipts: Vec<axiom_core::Receipt> = live.module.receipts.values().cloned().collect();
    assert!(
        !receipts.is_empty(),
        "at least one receipt must be captured"
    );

    // (b) Offline replay: receipts supplied, no capability, default ReplayOnlyExecutor.
    let ast2 = parse_module(SRC_DEMO3).expect("parse SRC_DEMO3 for replay");
    let mut rep = Runtime::new("demo3");
    rep.replay_mode(receipts);
    rep.execute(&ast2).expect("replay execute");

    let digest_replay = rep.digest();

    // (c) The offline replay must produce an identical module digest.
    assert_eq!(
        digest_live, digest_replay,
        "offline replay must reproduce the identical module digest"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — receipt_tampering_rejected_on_replay
// ---------------------------------------------------------------------------

#[test]
fn receipt_tampering_rejected_on_replay() {
    // Build a live run to get a real receipt.
    let ast = parse_module(SRC_DEMO3).expect("parse SRC_DEMO3");
    let mut live = Runtime::new("demo3");
    live.grant("tool:calculator");
    live.with_builtin_tool();
    live.execute(&ast).expect("live execute");

    let mut tampered: Vec<axiom_core::Receipt> = live.module.receipts.values().cloned().collect();
    assert!(!tampered.is_empty(), "need at least one receipt to tamper");

    // Mutate the first receipt's output to a different value.
    tampered[0].output = axiom_types::Value::Num(axiom_types::Num::Int(999));

    let ast2 = parse_module(SRC_DEMO3).expect("parse SRC_DEMO3 for tamper test");
    let mut rep2 = Runtime::new("demo3");
    rep2.replay_mode(tampered);

    let result = rep2.execute(&ast2);
    assert!(
        result.is_err(),
        "replay with a tampered receipt must be rejected (integrity check failed)"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — capability_required_for_external (required property 11)
// ---------------------------------------------------------------------------

#[test]
fn capability_required_for_external() {
    let ast = parse_module(SRC_DEMO3).expect("parse SRC_DEMO3");

    // Without any granted capability or receipts the call must fail.
    // The default Runtime uses ReplayOnlyExecutor, which refuses live calls
    // and has no receipts, so execute returns Err.
    let mut rt_no_cap = Runtime::new("demo3");
    let result_no_cap = rt_no_cap.execute(&ast);
    assert!(
        result_no_cap.is_err(),
        "execute without capability or receipts must return Err"
    );

    let err = result_no_cap.unwrap_err();
    // The error is either CapabilityDenied or ReplayMissingReceipt; both signal
    // that the external call was correctly blocked.
    let is_blocked = matches!(
        err,
        axiom_runtime::RuntimeError::CapabilityDenied(_)
            | axiom_runtime::RuntimeError::ReplayMissingReceipt(_)
    );
    assert!(
        is_blocked,
        "error must be CapabilityDenied or ReplayMissingReceipt, got: {:?}",
        err
    );

    // With capability + builtin tool the same source must succeed.
    let ast2 = parse_module(SRC_DEMO3).expect("parse SRC_DEMO3 for success case");
    let mut rt_granted = Runtime::new("demo3");
    rt_granted.grant("tool:calculator");
    rt_granted.with_builtin_tool();
    rt_granted
        .execute(&ast2)
        .expect("execute with capability must succeed");
}

// ---------------------------------------------------------------------------
// Test 4 — context_local_verification (required property 6)
// ---------------------------------------------------------------------------

#[test]
fn context_local_verification() {
    let ast = parse_module(SRC_DEMO2B).expect("parse SRC_DEMO2B");
    let mut rt = Runtime::new("demo2b");
    rt.execute(&ast).expect("execute demo2b");

    // `mid` is derived inside the `optimistic` branch and explicitly verified.
    // It must reach Verified status even though the global module has a contradiction.
    let mid_id = rt
        .claim("mid")
        .expect("claim 'mid' must be resolvable after execution");
    let mid_status = rt
        .module
        .claims
        .get(&mid_id)
        .expect("claim 'mid' must exist in module")
        .status;
    assert_eq!(
        mid_status,
        ClaimStatus::Verified,
        "'mid' must be Verified inside its local context"
    );

    // The global contradiction (low vs high) must still be recorded.
    let contradictions = rt.contradictions();
    assert!(
        !contradictions.is_empty(),
        "the global contradiction must still be present after context-local verification"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — contradiction_preserves_both_branches
// ---------------------------------------------------------------------------

#[test]
fn contradiction_preserves_both_branches() {
    let ast = parse_module(SRC_DEMO2).expect("parse SRC_DEMO2");
    let mut rt = Runtime::new("demo2");
    rt.execute(&ast).expect("execute demo2");

    let contradictions = rt.contradictions();

    // At least one contradiction must be recorded.
    assert!(
        !contradictions.is_empty(),
        "at least one contradiction must be recorded"
    );

    // Every contradiction must have a non-empty witness summary and at least
    // two claims referenced.
    for c in &contradictions {
        assert!(
            !c.witness.summary.is_empty(),
            "contradiction witness.summary must not be empty"
        );
        assert!(
            c.claims.len() >= 2,
            "contradiction must reference at least two claims"
        );
    }

    // Both `low` and `high` must still exist in the module (neither erased).
    let low_id = rt.claim("low").expect("label 'low' must exist");
    let high_id = rt.claim("high").expect("label 'high' must exist");
    assert!(
        rt.module.claims.contains_key(&low_id),
        "'low' claim must not be erased by a contradiction"
    );
    assert!(
        rt.module.claims.contains_key(&high_id),
        "'high' claim must not be erased by a contradiction"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — unverified_stays_quarantined_on_undischarged
// ---------------------------------------------------------------------------

#[test]
fn unverified_stays_quarantined_on_undischarged() {
    // A module with an external call but no capability and no receipts.
    // The ReplayOnlyExecutor (default) refuses live calls → execute returns Err.
    // Alternatively, if the runtime quarantines the claim, verify_failures is
    // non-empty. Either outcome demonstrates the property: the claim is never
    // silently verified.
    let src = SRC_DEMO3;
    let ast = parse_module(src).expect("parse SRC_DEMO3");

    // Plain Runtime: no capability, no replay receipts.
    let mut rt = Runtime::new("demo3");
    let result = rt.execute(&ast);

    if let Err(e) = &result {
        // The runtime correctly rejected execution — the call was blocked
        // (capability denied or replay missing). Property satisfied.
        let is_blocked = matches!(
            e,
            axiom_runtime::RuntimeError::CapabilityDenied(_)
                | axiom_runtime::RuntimeError::ReplayMissingReceipt(_)
        );
        assert!(
            is_blocked,
            "execute error must be a blocking error, not a silent wrong answer; got: {:?}",
            e
        );
    } else {
        // If the runtime did not return Err, the derived claim must be
        // quarantined (not Verified), and verify_failures must be non-empty.
        assert!(
            !rt.verify_failures.is_empty(),
            "if execute succeeds, verify_failures must record the undischarged call"
        );
        // The `sum` claim (the one derived from the external call) must NOT be Verified.
        if let Ok(sum_id) = rt.claim("sum") {
            let sum_status = rt.module.claims[&sum_id].status;
            assert_ne!(
                sum_status,
                ClaimStatus::Verified,
                "the externally-derived claim must not be silently verified"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Module-size resource cap (closes "no default node/claim-count cap" gap).
// ---------------------------------------------------------------------------

#[test]
fn oversized_module_rejected() {
    // Generate a module with one statement more than the hard cap.
    const CAP: usize = 100_000;
    let mut src = String::from("module big \"1\"\n");
    for i in 0..(CAP + 1) {
        src.push_str(&format!("assert a{i} = 1 : rational\n"));
    }
    let ast = axiom_parser::parse_module(&src).expect("oversized module should still parse");
    let mut rt = Runtime::new("big");
    let result = rt.execute(&ast);
    assert!(
        result.is_err(),
        "module exceeding the statement cap must be rejected"
    );
    assert!(
        format!("{:?}", result).contains("ModuleTooLarge")
            || format!("{}", result.unwrap_err()).contains("too large"),
        "error must indicate the module is too large"
    );
}

#[test]
fn near_cap_module_accepted() {
    // A module just under the cap must execute successfully.
    const CAP: usize = 100_000;
    let mut src = String::from("module ok \"1\"\n");
    for i in 0..(CAP - 1) {
        src.push_str(&format!("assert a{i} = 1 : rational\n"));
    }
    let ast = axiom_parser::parse_module(&src).expect("module should parse");
    let mut rt = Runtime::new("ok");
    assert!(
        rt.execute(&ast).is_ok(),
        "module under the cap must execute"
    );
}
