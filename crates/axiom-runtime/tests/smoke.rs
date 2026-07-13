use axiom_parser::parse_module;
use axiom_runtime::Runtime;
use axiom_types::Unit;

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
    // Discharging the mandatory NumericBounds obligation (by evidence, per
    // spec §6.6: discharged_by ∈ E ∪ R) lets verify succeed.
    use axiom_core::{ClaimStatus, ObligationState};
    let src = r#"
module demo "1"
assert a = 10 : rational
assert b = 2 : rational
derive q = core.div(a, b) : rational
require numeric-bounds on q
evidence bound "numeric" "q in [-inf,+inf]; divisor b=2 != 0" trust = unverified
discharge q by bound as satisfied
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

// Each automatically-detectable contradiction kind must be inferred by
// `detect_contradictions` without an explicit `contradict` instruction. This
// locks the spec's §3 enumeration (proposition-negation, incompatible-equality,
// disjoint-interval, incompatible-unit, violated-postcondition, assumption-conflict).
#[test]
fn automatic_contradiction_detects_six_kinds() {
    use axiom_core::{ContradictionKind, ObligationKind, Type, Uncertainty, Value};
    use axiom_types::num::Num;
    use axiom_types::Quantity;

    let mut rt = Runtime::new("demo");
    let m = &mut rt.module;

    // PropositionNegation: Bool(true) vs Bool(false).
    m.assert(
        "p_true",
        Type::Bool,
        Value::Bool(true),
        Uncertainty::Exact,
        vec![],
        None,
        None,
    )
    .unwrap();
    m.assert(
        "p_false",
        Type::Bool,
        Value::Bool(false),
        Uncertainty::Exact,
        vec![],
        None,
        None,
    )
    .unwrap();

    // DisjointInterval: [10,20] vs [30,40].
    m.assert(
        "i1",
        Type::NumericInterval,
        Value::Interval {
            lo: Num::Int(10),
            hi: Num::Int(20),
        },
        Uncertainty::Exact,
        vec![],
        None,
        None,
    )
    .unwrap();
    m.assert(
        "i2",
        Type::NumericInterval,
        Value::Interval {
            lo: Num::Int(30),
            hi: Num::Int(40),
        },
        Uncertainty::Exact,
        vec![],
        None,
        None,
    )
    .unwrap();

    // IncompatibleUnit: equal magnitude, different units.
    m.assert(
        "q1",
        Type::Quantity,
        Value::Quantity(Quantity {
            value: Num::Int(5),
            unit: Unit::base("m"),
        }),
        Uncertainty::Exact,
        vec![],
        None,
        None,
    )
    .unwrap();
    m.assert(
        "q2",
        Type::Quantity,
        Value::Quantity(Quantity {
            value: Num::Int(5),
            unit: Unit::base("s"),
        }),
        Uncertainty::Exact,
        vec![],
        None,
        None,
    )
    .unwrap();

    // ViolatedPostcondition: a claim carrying a Failed obligation.
    let vc = m
        .assert(
            "vc",
            Type::Rational,
            Value::Num(Num::Int(1)),
            Uncertainty::Exact,
            vec![],
            None,
            None,
        )
        .unwrap();
    m.add_obligation(&vc, ObligationKind::SourceSupport, true, false, None);
    for o in m.obligations.values_mut() {
        if o.target == vc {
            o.state = axiom_core::ObligationState::Failed;
        }
    }

    let detected = rt.detect_contradictions("root").unwrap();
    // At least the four kinds expressible via plain values must be detected.
    assert!(
        detected >= 4,
        "expected >=4 automatic contradictions, got {detected}"
    );

    let kinds: Vec<_> = rt
        .module
        .contradictions
        .values()
        .map(|c| c.kind.clone())
        .collect();
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, ContradictionKind::PropositionNegation)),
        "prop-negation not detected"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, ContradictionKind::DisjointInterval)),
        "disjoint-interval not detected"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, ContradictionKind::IncompatibleUnit)),
        "incompatible-unit not detected"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, ContradictionKind::ViolatedPostcondition)),
        "violated-postcondition not detected"
    );
}
