use axiom_parser::parse_module;
use axiom_runtime::Runtime;

const PIPE: &str = r#"
module demo "1"

assert a = 2 : rational
assert b = 3 : rational

derive s = core.add(a, b) : rational
verify s
"#;

#[test]
fn verified_pipeline_and_determinism() {
    let ast = parse_module(PIPE).unwrap();
    let mut rt = Runtime::new("demo");
    rt.execute(&ast).unwrap();
    assert_eq!(rt.verified_claims().len(), 1);

    let mut rt2 = Runtime::new("demo");
    rt2.execute(&ast).unwrap();
    assert_eq!(rt.digest(), rt2.digest(), "digest must be deterministic");
    assert_eq!(rt.event_log_digest(), rt2.event_log_digest());
}

#[test]
fn obligation_gate_blocks_verify() {
    // core.div generates a mandatory NumericBounds obligation that stays
    // Pending. Verification must be refused (the claim stays unverified), but
    // the failed verify is quarantined rather than silently verifying or
    // aborting the whole module, so valid portions are preserved.
    use axiom_core::ClaimStatus;
    let src = r#"
module demo "1"
assert a = 10 : rational
assert b = 2 : rational
derive q = core.div(a, b) : rational
verify q
"#;
    let ast = parse_module(src).unwrap();
    let mut rt = Runtime::new("demo");
    rt.execute(&ast).unwrap();
    let q = rt.claim("q").unwrap();
    assert_ne!(
        rt.module.claims[&q].status,
        ClaimStatus::Verified,
        "mandatory pending obligation must block verification"
    );
    assert!(
        !rt.verify_failures.is_empty(),
        "the blocked verify must be recorded as a non-fatal failure"
    );
}

#[test]
fn obligation_can_be_discharged_then_verified() {
    // Discharging the mandatory NumericBounds obligation lets verify succeed.
    use axiom_core::{ClaimStatus, ObligationState};
    let src = r#"
module demo "1"
assert a = 10 : rational
assert b = 2 : rational
derive q = core.div(a, b) : rational
require numeric-bounds on q
discharge q by a as satisfied
verify q
"#;
    let ast = parse_module(src).unwrap();
    let mut rt = Runtime::new("demo");
    rt.execute(&ast).unwrap();
    let q = rt.claim("q").unwrap();
    assert_eq!(
        rt.module.claims[&q].status,
        ClaimStatus::Verified,
        "after discharging the obligation, verify must succeed"
    );
    let discharged = rt.module.claims[&q]
        .obligations
        .iter()
        .any(|oid| rt.module.obligations[oid].state == ObligationState::Satisfied);
    assert!(discharged, "the obligation must be recorded as satisfied");
}

#[test]
fn external_receipt_and_offline_replay() {
    let src = r#"
module demo "1"
assert a = 4 : rational
assert b = 5 : rational
call total = tool.calculator(a, b) : rational cap "tool:calculator"
verify total
"#;
    let ast = parse_module(src).unwrap();

    // Live execution with the capability granted and a real executor.
    let mut rt = Runtime::new("demo");
    rt.with_builtin_tool();
    rt.grant("tool:calculator");
    rt.execute(&ast).unwrap();
    let live_digest = rt.digest();
    let live_events = rt.event_log_digest();
    let receipts = rt.module.receipts.values().cloned().collect::<Vec<_>>();
    assert!(!receipts.is_empty());

    // Offline replay: capability removed, executor is replay-only, receipts supplied.
    let mut rt2 = Runtime::new("demo");
    rt2.replay_mode(receipts);
    rt2.execute(&ast).unwrap();
    assert_eq!(
        rt2.digest(),
        live_digest,
        "replay digest must match live digest"
    );
    assert_eq!(rt2.event_log_digest(), live_events);
}

#[test]
fn replay_with_tampered_receipt_fails() {
    let src = r#"
module demo "1"
assert a = 4 : rational
assert b = 5 : rational
call total = tool.calculator(a, b) : rational cap "tool:calculator"
verify total
"#;
    let ast = parse_module(src).unwrap();
    let mut rt = Runtime::new("demo");
    rt.with_builtin_tool();
    rt.grant("tool:calculator");
    rt.execute(&ast).unwrap();
    let mut receipts = rt.module.receipts.values().cloned().collect::<Vec<_>>();
    // Tamper with the returned output value.
    receipts[0].output = axiom_types::Value::Num(axiom_types::Num::Int(999));
    let mut rt2 = Runtime::new("demo");
    rt2.replay_mode(receipts);
    assert!(
        rt2.execute(&ast).is_err(),
        "replay with tampered receipt must fail"
    );
}

#[test]
fn contradiction_is_preserved() {
    let src = r#"
module demo "1"
observe low = q(21.5 "C") : quantity evidence []
observe high = q(25.0 "C") : quantity evidence []
contradict low high as evidence-conflict
"#;
    let ast = parse_module(src).unwrap();
    let mut rt = Runtime::new("demo");
    rt.execute(&ast).unwrap();
    assert_eq!(rt.contradictions().len(), 1);
    // Both observations remain inspectable; neither was erased.
    assert_eq!(rt.module.claims.len(), 2);
}
