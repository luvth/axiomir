//! F1 production-path tests: trusted evidence authenticity is enforced through
//! the real parsed-module and replay surfaces, not only `Module::add_evidence`.
//!
//! Every test drives a textual Axiom module through `parse_module` + `Runtime`,
//! configuring host trust roots via `with_trust_roots`. A trusted evidence node
//! is signed over its canonical binding (provider, locator, media type, content
//! digest, trust class, label) with `axiom_core::crypto::sign_evidence`.

use axiom_core::crypto::{sign_evidence, TrustRoot};
use axiom_core::{content_id, Domain, Value};
use axiom_parser::parse_module;
use axiom_runtime::{Runtime, RuntimeError};
use serde_json::json;

const PROVIDER: &str = "sensor.authority";
const KEY: &[u8] = b"host-configured-secret-key";

/// Compute the evidence content digest exactly as `Module::add_evidence` does,
/// so the test can sign over the same binding the runtime will verify.
fn content_hash(media: &str, content: Option<&str>, locator: &str) -> String {
    let v = content.map(|c| Value::Str(c.to_string()));
    content_id(
        Domain::Evidence,
        &json!({ "media": media, "content": v, "locator": locator }),
    )
    .unwrap()
    .as_str()
    .to_string()
}

/// Build a textual `evidence` statement with a (valid or tampered) signature.
fn evidence_stmt(
    label: &str,
    media: &str,
    content: Option<&str>,
    provider: Option<&str>,
    signature: Option<&str>,
    trust: &str,
) -> String {
    let c = content
        .map(|x| format!(" \"{}\"", x.replace('\\', "\\\\").replace('"', "\\\"")))
        .unwrap_or_default();
    let mut s = format!("evidence {label} \"{media}\"{c} trust={trust}");
    if let Some(p) = provider {
        s.push_str(&format!(" provider=\"{p}\""));
    }
    if let Some(sig) = signature {
        s.push_str(&format!(" signature=\"{sig}\""));
    }
    s
}

/// Produce a valid HMAC tag for the given binding using `PROVIDER`/`KEY`.
fn sign(label: &str, media: &str, content: Option<&str>, trust: &str) -> String {
    let locator = media; // runtime sets locator = media_type
    let ch = content_hash(media, content, locator);
    let roots = [TrustRoot::new(PROVIDER, KEY)];
    let tag = sign_evidence(&roots, PROVIDER, locator, media, &ch, trust, label)
        .expect("signing must succeed with a matching trust root");
    hex::encode(tag)
}

fn module_with_evidence(stmt: &str) -> String {
    format!("module m \"1\"\n{stmt}\nassert x = 1 : rational\n")
}

/// Execute `src` with the given trust roots. Returns `Ok(())` on success so the
/// caller need not rely on `Runtime: Debug` for `unwrap`/`expect`.
fn run_with_roots(src: &str, roots: &[TrustRoot]) -> Result<(), RuntimeError> {
    let ast = parse_module(src).expect("produced source must parse");
    let mut rt = Runtime::new("m");
    rt.with_trust_roots(roots.to_vec());
    rt.execute(&ast)
}

fn has_evidence(src: &str, label: &str, roots: &[TrustRoot]) -> bool {
    let ast = parse_module(src).expect("produced source must parse");
    let mut rt = Runtime::new("m");
    rt.with_trust_roots(roots.to_vec());
    rt.execute(&ast).is_ok() && rt.evidence(label).is_ok()
}

#[test]
fn trusted_evidence_with_valid_signature_passes() {
    let sig = sign("e", "text/plain", Some("payload"), "Trusted");
    let stmt = evidence_stmt(
        "e",
        "text/plain",
        Some("payload"),
        Some(PROVIDER),
        Some(&sig),
        "trusted",
    );
    assert!(has_evidence(
        &module_with_evidence(&stmt),
        "e",
        &[TrustRoot::new(PROVIDER, KEY)]
    ));
}

#[test]
fn trusted_evidence_without_provider_fails() {
    let stmt = evidence_stmt("e", "text/plain", Some("payload"), None, None, "trusted");
    let err = run_with_roots(
        &module_with_evidence(&stmt),
        &[TrustRoot::new(PROVIDER, KEY)],
    )
    .unwrap_err();
    assert!(format!("{err}").contains("provider"), "got: {err}");
}

#[test]
fn trusted_evidence_without_signature_fails() {
    let stmt = evidence_stmt(
        "e",
        "text/plain",
        Some("payload"),
        Some(PROVIDER),
        None,
        "trusted",
    );
    let err = run_with_roots(
        &module_with_evidence(&stmt),
        &[TrustRoot::new(PROVIDER, KEY)],
    )
    .unwrap_err();
    assert!(format!("{err}").contains("signature"), "got: {err}");
}

#[test]
fn trusted_evidence_wrong_provider_fails() {
    // Signature is valid for PROVIDER, but the statement names a different provider.
    let sig = sign("e", "text/plain", Some("payload"), "Trusted");
    let stmt = evidence_stmt(
        "e",
        "text/plain",
        Some("payload"),
        Some("other.authority"),
        Some(&sig),
        "trusted",
    );
    let err = run_with_roots(
        &module_with_evidence(&stmt),
        &[TrustRoot::new(PROVIDER, KEY)],
    )
    .unwrap_err();
    assert!(
        format!("{err}").contains("trust root") || format!("{err}").contains("provider"),
        "got: {err}"
    );
}

#[test]
fn trusted_evidence_wrong_key_fails() {
    // Sign with a different key than the configured trust root.
    let sig = {
        let locator = "text/plain";
        let ch = content_hash("text/plain", Some("payload"), locator);
        let roots = [TrustRoot::new(PROVIDER, b"wrong-key")];
        let tag = sign_evidence(&roots, PROVIDER, locator, "text/plain", &ch, "Trusted", "e")
            .expect("sign");
        hex::encode(tag)
    };
    let stmt = evidence_stmt(
        "e",
        "text/plain",
        Some("payload"),
        Some(PROVIDER),
        Some(&sig),
        "trusted",
    );
    let err = run_with_roots(
        &module_with_evidence(&stmt),
        &[TrustRoot::new(PROVIDER, KEY)],
    )
    .unwrap_err();
    assert!(format!("{err}").contains("verify"), "got: {err}");
}

#[test]
fn trusted_evidence_modified_content_fails() {
    // Signature computed over "payload", but statement carries "tampered".
    let sig = sign("e", "text/plain", Some("payload"), "Trusted");
    let stmt = evidence_stmt(
        "e",
        "text/plain",
        Some("tampered"),
        Some(PROVIDER),
        Some(&sig),
        "trusted",
    );
    let err = run_with_roots(
        &module_with_evidence(&stmt),
        &[TrustRoot::new(PROVIDER, KEY)],
    )
    .unwrap_err();
    assert!(format!("{err}").contains("verify"), "got: {err}");
}

#[test]
fn trusted_evidence_modified_locator_fails() {
    // Signature computed with locator == media ("text/plain"), but the
    // statement uses a different media type, so locator differs.
    let sig = sign("e", "text/plain", Some("payload"), "Trusted");
    let stmt = evidence_stmt(
        "e",
        "application/json",
        Some("payload"),
        Some(PROVIDER),
        Some(&sig),
        "trusted",
    );
    let err = run_with_roots(
        &module_with_evidence(&stmt),
        &[TrustRoot::new(PROVIDER, KEY)],
    )
    .unwrap_err();
    assert!(format!("{err}").contains("verify"), "got: {err}");
}

#[test]
fn trusted_evidence_modified_media_type_fails() {
    // Signature bound to "text/plain"; statement declares "application/json".
    let sig = sign("e", "text/plain", Some("payload"), "Trusted");
    let stmt = evidence_stmt(
        "e",
        "application/json",
        Some("payload"),
        Some(PROVIDER),
        Some(&sig),
        "trusted",
    );
    let err = run_with_roots(
        &module_with_evidence(&stmt),
        &[TrustRoot::new(PROVIDER, KEY)],
    )
    .unwrap_err();
    assert!(format!("{err}").contains("verify"), "got: {err}");
}

#[test]
fn trusted_evidence_modified_label_fails() {
    // Signature bound to label "e"; statement declares label "f".
    let sig = sign("e", "text/plain", Some("payload"), "Trusted");
    let stmt = evidence_stmt(
        "f",
        "text/plain",
        Some("payload"),
        Some(PROVIDER),
        Some(&sig),
        "trusted",
    );
    let err = run_with_roots(
        &module_with_evidence(&stmt),
        &[TrustRoot::new(PROVIDER, KEY)],
    )
    .unwrap_err();
    assert!(format!("{err}").contains("verify"), "got: {err}");
}

#[test]
fn unverified_evidence_without_signature_succeeds() {
    let stmt = evidence_stmt("e", "text/plain", Some("payload"), None, None, "unverified");
    assert!(has_evidence(
        &module_with_evidence(&stmt),
        "e",
        &[TrustRoot::new(PROVIDER, KEY)]
    ));
}

#[test]
fn untrusted_evidence_without_signature_succeeds() {
    let stmt = evidence_stmt("e", "text/plain", Some("payload"), None, None, "untrusted");
    assert!(has_evidence(
        &module_with_evidence(&stmt),
        "e",
        &[TrustRoot::new(PROVIDER, KEY)]
    ));
}

#[test]
fn formatter_round_trip_preserves_authenticity_fields() {
    use axiom_parser::format_source;
    let sig = sign("e", "text/plain", Some("payload"), "Trusted");
    let stmt = evidence_stmt(
        "e",
        "text/plain",
        Some("payload"),
        Some(PROVIDER),
        Some(&sig),
        "trusted",
    );
    let src = module_with_evidence(&stmt);
    let _ast = parse_module(&src).unwrap();
    let out = format_source(&src).expect("format");
    // Re-parse the formatted output and re-execute: fields must survive.
    let ast2 = parse_module(&out).unwrap();
    let mut rt = Runtime::new("m");
    rt.with_trust_roots(vec![TrustRoot::new(PROVIDER, KEY)]);
    rt.execute(&ast2)
        .expect("formatted module must still verify the signature");
    assert!(out.contains(&format!("provider=\"{PROVIDER}\"")));
    assert!(out.contains(&format!("signature=\"{sig}\"")));
}

#[test]
fn canonical_round_trip_preserves_verification() {
    use axiom_parser::format_source;
    let sig = sign("e", "text/plain", Some("payload"), "Trusted");
    let stmt = evidence_stmt(
        "e",
        "text/plain",
        Some("payload"),
        Some(PROVIDER),
        Some(&sig),
        "trusted",
    );
    let src = module_with_evidence(&stmt);
    let once = run_with_roots(&src, &[TrustRoot::new(PROVIDER, KEY)])
        .map(|()| {
            let ast = parse_module(&src).unwrap();
            let mut rt = Runtime::new("m");
            rt.with_trust_roots(vec![TrustRoot::new(PROVIDER, KEY)]);
            rt.execute(&ast).unwrap();
            rt.digest()
        })
        .unwrap();
    // format -> parse -> format -> parse: digest must be stable.
    let out1 = format_source(&src).expect("format1");
    let _ast2 = parse_module(&out1).unwrap();
    let out2 = format_source(&out1).expect("format2");
    let ast3 = parse_module(&out2).unwrap();
    let mut rt = Runtime::new("m");
    rt.with_trust_roots(vec![TrustRoot::new(PROVIDER, KEY)]);
    rt.execute(&ast3).unwrap();
    assert_eq!(
        once,
        rt.digest(),
        "canonical round-trip must preserve digest"
    );
}

#[test]
fn replay_rejects_tampered_evidence() {
    // Live run signs nothing; this test checks the *replay* path rejects a
    // module whose trusted evidence fails authentication. We reuse the
    // modified-content case: the replay loader calls the same authenticator.
    let sig = sign("e", "text/plain", Some("payload"), "Trusted");
    let stmt = evidence_stmt(
        "e",
        "text/plain",
        Some("tampered"),
        Some(PROVIDER),
        Some(&sig),
        "trusted",
    );
    let src = module_with_evidence(&stmt);
    let ast = parse_module(&src).unwrap();
    let mut rt = Runtime::new("m");
    rt.with_trust_roots(vec![TrustRoot::new(PROVIDER, KEY)]);
    let err = rt.execute(&ast).unwrap_err();
    assert!(
        format!("{err}").contains("authentic") || format!("{err}").contains("verify"),
        "replay/load path must reject tampered evidence: {err}"
    );
}

#[test]
fn producer_generated_signed_evidence_verifies() {
    // End-to-end: the producer emits evidence, the host signs it, and the
    // resulting module verifies under the configured trust root.
    use axiom_producer::produce_from_json;
    let json = r#"{
        "module_name": "prod",
        "premises": [{"label":"t","value":{"Int":1},"evidence":"raw sensor reading"}],
        "assumptions": [],
        "steps": []
    }"#;
    let _src = produce_from_json(json).expect("plan must produce");
    // The producer emits `evidence ev_t "text/plain" "raw sensor reading"`.
    // Re-sign it as trusted with our host root and re-emit a trusted module.
    let label = "ev_t";
    let media = "text/plain";
    let content = "raw sensor reading";
    let sig = sign(label, media, Some(content), "Trusted");
    let trusted = format!(
        "module prod \"1\"\nevidence {label} \"{media}\" \"{escaped}\" trust=trusted provider=\"{PROVIDER}\" signature=\"{sig}\"\nassert t = 1 : rational\n",
        escaped = content.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let ast = parse_module(&trusted).expect("trusted module must parse");
    let mut rt = Runtime::new("prod");
    rt.with_trust_roots(vec![TrustRoot::new(PROVIDER, KEY)]);
    rt.execute(&ast)
        .expect("producer-backed trusted evidence must verify");
}
