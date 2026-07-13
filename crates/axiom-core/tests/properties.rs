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
            &[],
        )
        .unwrap();

    m.verify(out.clone()).unwrap();

    assert_eq!(m.claims[&out].status, ClaimStatus::Verified);
    assert!(m.claims[&out].derivation.is_some());
    assert!(m.check_verification_invariant().is_ok());
}

// ---------------------------------------------------------------------------
// Property 2: a mandatory, undischarged SourceSupport obligation blocks verify,
// and is only cleared by a real discharge referencing trusted evidence.
// ---------------------------------------------------------------------------

#[test]
fn undischarged_obligation_blocks_verify() {
    let mut m = module_with_builtins();

    // A derived claim produced by a *risky* operation (division) carries a
    // mandatory, non-auto-satisfied numeric-bounds proof obligation. A bare
    // `verify` MUST be quarantined until that obligation is explicitly
    // discharged (spec §6.6). This is what makes obligation-gated verification
    // fundamental rather than decorative.
    let a = m
        .assert(
            "a",
            Type::Rational,
            Value::Num(Num::Int(10)),
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

    let c = m
        .derive(
            "core.div",
            "1",
            &[a, b],
            None,
            "c",
            None,
            &BuiltinExecutor,
            &[],
        )
        .unwrap();

    // A mandatory NumericBounds obligation is Pending on the derived claim.
    let has_pending_mandatory = m.claims[&c].obligations.iter().any(|oid| {
        let o = &m.obligations[oid];
        o.severity == Severity::Mandatory && o.state == ObligationState::Pending
    });
    assert!(
        has_pending_mandatory,
        "expected a pending mandatory numeric-bounds obligation from div"
    );

    // verify must fail while the obligation is undischarged.
    assert!(
        m.verify(c.clone()).is_err(),
        "verify should have returned Err for undischarged obligation"
    );
    assert_ne!(m.claims[&c].status, ClaimStatus::Verified);

    // Discharging the pending obligation by trusted evidence clears the gate
    // and lets the claim verify.
    let e_trusted = m.add_evidence(
        "et",
        "text/plain",
        None,
        TrustClass::Trusted,
        "loc2",
        "test-source",
        None,
    );
    let obl = m.claims[&c]
        .obligations
        .iter()
        .find(|oid| m.obligations[*oid].state == ObligationState::Pending)
        .cloned()
        .expect("expected a pending obligation to discharge");
    m.discharge(obl, e_trusted, ObligationState::Satisfied)
        .unwrap();
    m.verify(c.clone()).unwrap();
    assert_eq!(m.claims[&c].status, ClaimStatus::Verified);
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
// Property 4: receipt authenticity — naive and adversarial forgery both rejected
// ---------------------------------------------------------------------------

#[test]
fn receipt_tampering_detected() {
    let trust = TrustRoot::new("p", b"k");
    let roots = [trust.clone()];
    let mut m = module_with_builtins();

    let ih = content_id(Domain::Receipt, &serde_json::json!({"x": 1})).unwrap();
    let r = m.add_receipt(
        "tool.x",
        "1",
        "p",
        0,
        ih,
        &Value::Num(Num::Int(1)),
        Type::Rational,
        &trust,
    );

    // Untampered, authentically-signed receipt verifies.
    assert!(
        m.receipts[&r].verify_integrity(&roots),
        "fresh receipt should pass authenticity check"
    );

    // Naive tamper (change output, leave the tag) must fail.
    m.receipts.get_mut(&r).unwrap().output = Value::Num(Num::Int(2));
    assert!(
        !m.receipts[&r].verify_integrity(&roots),
        "tampered receipt should fail authenticity check"
    );

    // Adversarial forger who recomputes the payload hash but lacks the secret
    // still cannot forge a valid tag: verification must remain false.
    {
        let rec = m.receipts.get_mut(&r).unwrap();
        rec.integrity = content_id(
            Domain::Receipt,
            &serde_json::from_slice::<serde_json::Value>(&rec.signed_payload()).unwrap(),
        )
        .unwrap();
    }
    assert!(
        !m.receipts[&r].verify_integrity(&roots),
        "adversarially re-hashed but unsigned receipt must still fail"
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

// ---------------------------------------------------------------------------
// Evidence authenticity: forgery is rejected, valid signatures pass.
// ---------------------------------------------------------------------------

#[test]
fn evidence_authenticity_rejects_unsigned_high_trust() {
    use axiom_core::{crypto::TrustRoot, verify_evidence, EvidenceAuthError};

    let root = TrustRoot::new("src:lab", b"secret-key-material");
    let mut m = module_with_builtins();
    // A trusted evidence node with NO signature must fail authentication when a
    // trust root is configured. This is the enforced (not advisory) property
    // that makes a forged receipt impossible under the trust model.
    let ev = m.add_evidence(
        "e",
        "text/plain",
        None,
        TrustClass::Trusted,
        "loc",
        "src:lab",
        None,
    );
    let e = &m.evidence[&ev];
    assert_eq!(
        verify_evidence(
            std::slice::from_ref(&root),
            &e.provider,
            &e.acquisition.locator,
            &e.media_type,
            &e.content_hash.as_str(),
            e.trust,
            &e.label,
            &e.signature
        ),
        Err(EvidenceAuthError::NoSignature),
        "unsigned trusted evidence must NOT pass authentication"
    );
    assert!(
        !m.verify_evidence_authenticity(std::slice::from_ref(&root)),
        "unsigned trusted evidence must NOT pass authenticity"
    );
    let _ = ev;
}

#[test]
fn evidence_authenticity_accepts_valid_signature() {
    use axiom_core::crypto::{sign_evidence, TrustRoot};

    let root = TrustRoot::new("src:lab", b"secret-key-material");
    let mut m = module_with_builtins();
    let ev_id = m.add_evidence(
        "e",
        "text/plain",
        None,
        TrustClass::Trusted,
        "loc",
        "src:lab",
        None,
    );

    // The producer signs the evidence binding under the trust root. The binding
    // now includes media type and trust class, so a retyped node is rejected.
    let content_hash = m.evidence[&ev_id].content_hash.as_str().to_string();
    let sig = sign_evidence(
        std::slice::from_ref(&root),
        "src:lab",
        "loc",
        "text/plain",
        &content_hash,
        "Trusted",
        "e",
    );
    m.evidence.get_mut(&ev_id).unwrap().signature = sig.map(hex::encode);

    assert!(
        m.verify_evidence_authenticity(std::slice::from_ref(&root)),
        "correctly signed trusted evidence must pass authenticity"
    );

    // Tampering with the label binding invalidates the signature.
    let mut m2 = module_with_builtins();
    let e2 = m2.add_evidence(
        "e",
        "text/plain",
        None,
        TrustClass::Trusted,
        "loc",
        "src:lab",
        None,
    );
    let content_hash2 = m2.evidence[&e2].content_hash.as_str().to_string();
    let sig2 = sign_evidence(
        std::slice::from_ref(&root),
        "src:lab",
        "loc",
        "text/plain",
        &content_hash2,
        "Trusted",
        "e",
    );
    let mut ev = m2.evidence.get_mut(&e2).unwrap().clone();
    ev.label = "tampered".into();
    ev.signature = sig2.map(hex::encode);
    m2.evidence.insert(e2, ev);
    assert!(
        !m2.verify_evidence_authenticity(std::slice::from_ref(&root)),
        "tampered label binding must fail authenticity"
    );
}

#[test]
fn evidence_authenticity_permits_untrusted_without_signature() {
    use axiom_core::crypto::TrustRoot;
    let mut m = module_with_builtins();
    m.add_evidence(
        "e",
        "text/plain",
        None,
        TrustClass::Untrusted,
        "loc",
        "src:lab",
        None,
    );
    // Untrusted evidence never requires a signature.
    assert!(
        m.verify_evidence_authenticity(&[TrustRoot::new("src:lab", b"k")]),
        "untrusted evidence must pass regardless of signature"
    );
}
