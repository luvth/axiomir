use axiom_core::registry::{builtin_operations, BuiltinExecutor};
use axiom_core::*;
use axiom_encoding::{content_id, Domain};
use axiom_types::{num::Num, Type, Uncertainty, Value};

// Helper: fresh module with all builtins registered.
fn module_with_builtins() -> Module {
    let mut m = Module::new("test");
    for op in builtin_operations() {
        m.register_op(op);
    }
    m
}

// ---------------------------------------------------------------------------
// Property 1: verified derivation is complete
// ---------------------------------------------------------------------------

#[test]
fn verified_derivation_complete() {
    let mut m = module_with_builtins();

    let a = m
        .assert(
            "a",
            Type::Rational,
            Value::Num(Num::Int(2)),
            Uncertainty::Exact,
            vec![],
            None,
            None,
        )
        .unwrap();
    let b = m
        .assert(
            "b",
            Type::Rational,
            Value::Num(Num::Int(3)),
            Uncertainty::Exact,
            vec![],
            None,
            None,
        )
        .unwrap();

    let out = m
        .derive(
            "core.add",
            "1",
            &[a, b],
            None,
            "out",
            None,
            &BuiltinExecutor,
        )
        .unwrap();

    m.verify(out.clone()).unwrap();

    assert_eq!(m.claims[&out].status, ClaimStatus::Verified);
    assert!(m.claims[&out].derivation.is_some());
    assert!(m.check_verification_invariant().is_ok());
}

// ---------------------------------------------------------------------------
// Property 2: undischarged mandatory obligation blocks verify
// ---------------------------------------------------------------------------

#[test]
fn undischarged_obligation_blocks_verify() {
    let mut m = module_with_builtins();

    // core.div generates NumericBounds (Mandatory, stays Pending) plus TypeCompat.
    let a = m
        .assert(
            "a",
            Type::Rational,
            Value::Num(Num::Int(6)),
            Uncertainty::Exact,
            vec![],
            None,
            None,
        )
        .unwrap();
    let b = m
        .assert(
            "b",
            Type::Rational,
            Value::Num(Num::Int(2)),
            Uncertainty::Exact,
            vec![],
            None,
            None,
        )
        .unwrap();

    let out = m
        .derive(
            "core.div",
            "1",
            &[a, b],
            None,
            "out",
            None,
            &BuiltinExecutor,
        )
        .unwrap();

    // At least one mandatory obligation (NumericBounds) is still Pending.
    let has_pending_mandatory = m.claims[&out].obligations.iter().any(|oid| {
        let o = &m.obligations[oid];
        o.severity == Severity::Mandatory && o.state == ObligationState::Pending
    });
    assert!(
        has_pending_mandatory,
        "expected a pending mandatory obligation from core.div"
    );

    // verify must fail.
    let result = m.verify(out.clone());
    assert!(
        result.is_err(),
        "verify should have returned Err for undischarged obligation"
    );

    // The claim must not be Verified.
    assert_ne!(m.claims[&out].status, ClaimStatus::Verified);
}

// ---------------------------------------------------------------------------
// Property 3: assertion cannot silently verify
// ---------------------------------------------------------------------------

#[test]
fn assertion_cannot_silent_verify() {
    let mut m = module_with_builtins();

    let x = m
        .assert(
            "x",
            Type::Bool,
            Value::Bool(true),
            Uncertainty::Exact,
            vec![],
            None,
            None,
        )
        .unwrap();

    let result = m.verify(x.clone());
    assert!(
        result.is_err(),
        "verify on a bare assertion must return Err"
    );
    assert_eq!(
        m.claims[&x].status,
        ClaimStatus::Asserted,
        "asserted claim must remain Asserted after failed verify"
    );
}

// ---------------------------------------------------------------------------
// Property 4: receipt tampering detected
// ---------------------------------------------------------------------------

#[test]
fn receipt_tampering_detected() {
    let mut m = module_with_builtins();

    // We don't strictly need the evidence for add_receipt, but the task spec
    // shows an add_evidence call first — we include it for completeness.
    let _eid = m.add_evidence("e", "text/plain", None, TrustClass::Unverified, "loc");

    let ih = content_id(Domain::Receipt, &serde_json::json!({"x": 1})).unwrap();
    let r = m.add_receipt(
        "tool.x",
        "1",
        "p",
        0,
        ih,
        &Value::Num(Num::Int(1)),
        Type::Rational,
    );

    // Untampered receipt must verify.
    assert!(
        m.receipts[&r].verify_integrity(),
        "fresh receipt should pass integrity check"
    );

    // Tamper: change the output value.
    m.receipts.get_mut(&r).unwrap().output = Value::Num(Num::Int(2));

    // Now integrity must fail.
    assert!(
        !m.receipts[&r].verify_integrity(),
        "tampered receipt should fail integrity check"
    );
}

// ---------------------------------------------------------------------------
// Property 5: canonical hash stability (key-order independent)
// ---------------------------------------------------------------------------

#[test]
fn canonical_hash_stability() {
    let v1 = serde_json::json!({"b": 1, "a": 2});
    let v2 = serde_json::json!({"a": 2, "b": 1});

    let id1 = content_id(Domain::Claim, &v1).unwrap();
    let id2 = content_id(Domain::Claim, &v2).unwrap();
    assert_eq!(id1, id2, "content_id must be order-independent");

    let bytes1 = axiom_encoding::canonical_bytes(&v1).unwrap();
    let bytes2 = axiom_encoding::canonical_bytes(&v2).unwrap();
    assert_eq!(bytes1, bytes2, "canonical_bytes must be order-independent");
}

// ---------------------------------------------------------------------------
// Property 6: semantic_id is independent of label
// ---------------------------------------------------------------------------

#[test]
fn semantic_id_independent_of_label() {
    let mut m = module_with_builtins();

    let c1 = m
        .assert(
            "label_alpha",
            Type::Rational,
            Value::Num(Num::Int(7)),
            Uncertainty::Exact,
            vec![],
            None,
            None,
        )
        .unwrap();
    let c2 = m
        .assert(
            "label_beta",
            Type::Rational,
            Value::Num(Num::Int(7)),
            Uncertainty::Exact,
            vec![],
            None,
            None,
        )
        .unwrap();

    assert_eq!(
        m.claims[&c1].semantic_id, m.claims[&c2].semantic_id,
        "two claims with the same proposition must share semantic_id regardless of label"
    );
}

// ---------------------------------------------------------------------------
// Property 7: domain separation
// ---------------------------------------------------------------------------

#[test]
fn domain_separation() {
    let v = serde_json::json!({"x": 1});

    let claim_id = content_id(Domain::Claim, &v).unwrap();
    let evidence_id = content_id(Domain::Evidence, &v).unwrap();

    assert_ne!(
        claim_id, evidence_id,
        "same payload in different domains must produce distinct ids"
    );
}

// ---------------------------------------------------------------------------
// Property 8: context isolation and contradiction preserves claims
// ---------------------------------------------------------------------------

#[test]
fn context_isolation_and_contradiction_preserves_claims() {
    let mut m = module_with_builtins();

    // Introduce an assumption in the root context.
    let (aid, cid) = m
        .assume("h", Type::Bool, Value::Bool(true), "s", None)
        .unwrap();

    // Locate the assumption by finding the one whose .claim == cid.
    let assumption_id = m
        .assumptions
        .values()
        .find(|a| a.claim == cid)
        .map(|a| a.id.clone())
        .expect("assumption referencing cid must exist");
    assert_eq!(assumption_id, aid);

    // Branch from root, carrying the assumption.
    let root = m.root_context.clone();
    let ctx = m
        .branch(root, "branch-s", std::slice::from_ref(&aid))
        .unwrap();

    // The branch's inherited_claims must include the assumption's claim.
    assert!(
        m.contexts[&ctx].inherited_claims.contains(&cid),
        "branched context must inherit the assumption's claim"
    );

    // Create a second bool claim via assert (different label so different node id).
    let c2 = m
        .assert(
            "c2",
            Type::Bool,
            Value::Bool(true),
            Uncertainty::Exact,
            vec![],
            None,
            None,
        )
        .unwrap();

    // Build witness and record a contradiction.
    let witness = Witness {
        summary: "s".into(),
        detail: Value::Bool(true),
    };

    let contradiction_id = m
        .contradict(
            &[cid.clone(), c2.clone()],
            ContradictionKind::PropositionNegation,
            witness,
            None,
        )
        .unwrap();

    // Contradiction must be recorded.
    assert!(
        m.contradictions.contains_key(&contradiction_id),
        "contradiction must be stored in m.contradictions"
    );

    // Both claims must survive — contradiction never erases claims.
    assert!(
        m.claims.contains_key(&cid),
        "cid must still exist after contradiction"
    );
    assert!(
        m.claims.contains_key(&c2),
        "c2 must still exist after contradiction"
    );
}
