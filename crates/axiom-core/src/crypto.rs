//! Cryptographic receipt authenticity for Axiom IR.
//!
//! A receipt is **authentic** only if its tag is a keyed HMAC-SHA256 over its
//! signed payload, computed under the trusted secret of its provider. This is
//! not a checksum: forgery requires the provider's secret, which an untrusted
//! module author does not hold. A self-consistent but unsigned/forged receipt
//! is rejected because its tag cannot be reproduced without the trusted key.
//!
//! Trust roots are configured by the host (the verifier), never by the module.
//! The model that produced the receipt holds the signing secret; the host that
//! replays it holds only the verifying secret, configured as a [`TrustRoot`].

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::Sha256;

use crate::TrustClass;

type HmacSha256 = Hmac<Sha256>;

/// A host-configured verifying key for a single receipt provider.
///
/// `secret` is the HMAC key shared with (or published by) the provider named in
/// `provider`. The host supplies these; a module cannot invent a trust root.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustRoot {
    pub provider: String,
    pub secret: Vec<u8>,
}

impl TrustRoot {
    pub fn new(provider: &str, secret: &[u8]) -> Self {
        TrustRoot {
            provider: provider.to_string(),
            secret: secret.to_vec(),
        }
    }

    /// The trust root used by the reference runtime's fixture calculator. The
    /// secret is a fixed demo value; a production deployment MUST configure its
    /// own per-provider secrets and MUST NOT ship this one.
    pub fn demo() -> Self {
        TrustRoot::new(
            "fixture:calculator",
            b"axiom-demo-trust-root-do-not-use-in-production",
        )
    }
}

/// Produce the HMAC-SHA256 tag over `payload` under `secret`.
pub fn sign(secret: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(secret).expect("hmac accepts variable-length keys");
    mac.update(payload);
    mac.finalize().into_bytes().to_vec()
}

/// Verify `tag` over `payload` under the trusted secret for `provider`.
///
/// Returns `false` if no trust root matches `provider`, or the tag does not
/// verify. Constant-time comparison is provided by `hmac::Mac::verify_slice`.
pub fn verify(trust_roots: &[TrustRoot], provider: &str, payload: &[u8], tag: &[u8]) -> bool {
    let Some(root) = trust_roots.iter().find(|r| r.provider == provider) else {
        return false;
    };
    let mut mac = match HmacSha256::new_from_slice(&root.secret) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(payload);
    mac.verify_slice(tag).is_ok()
}

/// Domain/version separator for the evidence authenticity binding. Changing the
/// version makes signatures from a prior format invalid, preventing cross-format
/// substitution. Distinct from the receipt binding so an evidence tag can never
/// be confused with a receipt tag.
pub const EVIDENCE_BINDING_VERSION: &str = "axiom-evidence-v1";

/// Canonical binding that an evidence signature commits to. Binds the
/// *version/domain separator* (prevents cross-format/cross-domain substitution),
/// the *provider* (who vouches), the *locator* (which artifact), the
/// *media type* (what serialization the artifact uses), the *content digest*
/// (what the artifact says), the *trust class* (which trust tier it claims),
/// and the *label* (which evidence slot it fills). A signature over this tuple
/// makes a forged, swapped, or retyped evidence node detectable: changing any
/// field (or dropping/raising the trust tier) invalidates the tag.
pub fn evidence_binding(
    provider: &str,
    locator: &str,
    media_type: &str,
    content_hash: &str,
    trust: &str,
    label: &str,
) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": EVIDENCE_BINDING_VERSION,
        "provider": provider,
        "locator": locator,
        "media_type": media_type,
        "content_hash": content_hash,
        "trust": trust,
        "label": label,
    }))
    .unwrap()
}

/// Produce a signature (HMAC-SHA256 tag) committing an evidence node to its
/// canonical binding, under the trusted secret for `provider`.
pub fn sign_evidence(
    trust_roots: &[TrustRoot],
    provider: &str,
    locator: &str,
    media_type: &str,
    content_hash: &str,
    trust: &str,
    label: &str,
) -> Option<Vec<u8>> {
    let root = trust_roots.iter().find(|r| r.provider == provider)?;
    let payload = evidence_binding(provider, locator, media_type, content_hash, trust, label);
    Some(sign(&root.secret, &payload))
}

/// Verify an evidence signature against a configured trust root.
///
/// Returns `false` if `sig` is absent, no trust root matches `provider`, or the
/// tag does not verify over the canonical binding (which now includes the media
/// type and trust class, so a retyped or re-tiered node is also rejected).
#[allow(clippy::too_many_arguments)]
pub fn verify_evidence_signature(
    trust_roots: &[TrustRoot],
    provider: &str,
    locator: &str,
    media_type: &str,
    content_hash: &str,
    trust: &str,
    label: &str,
    sig: &Option<String>,
) -> bool {
    let Some(sig) = sig else {
        return false;
    };
    let Ok(tag) = hex::decode(sig) else {
        return false;
    };
    let payload = evidence_binding(provider, locator, media_type, content_hash, trust, label);
    verify(trust_roots, provider, &payload, &tag)
}

/// The precise reason trusted-evidence authentication failed. Returned by
/// [`verify_evidence`] so callers (execution, replay, SDK validation) can emit
/// a specific, actionable error instead of a generic "forgery" message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceAuthError {
    /// `trust = trusted` but no `provider` was declared.
    NoProvider,
    /// `trust = trusted` but no `signature` was declared.
    NoSignature,
    /// No host-configured trust root matches the evidence's provider.
    NoTrustRoot,
    /// The signature is present but does not verify over the canonical binding.
    BadSignature,
}

/// Authoritative trusted-evidence authentication used by execution, replay, and
/// SDK validation — a single shared implementation to avoid security-semantic
/// drift between paths.
///
/// Untrusted / unverified evidence is always accepted (its trust class is the
/// gate). For `trust = trusted`, requires a provider, a signature, a matching
/// configured trust root, and a signature that verifies over the exact
/// canonical binding; otherwise returns the specific [`EvidenceAuthError`].
#[allow(clippy::too_many_arguments)]
pub fn verify_evidence(
    trust_roots: &[TrustRoot],
    provider: &str,
    locator: &str,
    media_type: &str,
    content_hash: &str,
    trust: TrustClass,
    label: &str,
    signature: &Option<String>,
) -> Result<(), EvidenceAuthError> {
    // Only trusted evidence must authenticate. Callers are expected to gate on
    // `trust == TrustClass::Trusted` before calling; we defensively accept all
    // other tiers here as well.
    if trust != TrustClass::Trusted {
        return Ok(());
    }
    if provider.is_empty() {
        return Err(EvidenceAuthError::NoProvider);
    }
    if trust_roots.iter().all(|t| t.provider != provider) {
        return Err(EvidenceAuthError::NoTrustRoot);
    }
    if signature.is_none() {
        return Err(EvidenceAuthError::NoSignature);
    }
    let trust_str = format!("{trust:?}");
    if verify_evidence_signature(
        trust_roots,
        provider,
        locator,
        media_type,
        content_hash,
        &trust_str,
        label,
        signature,
    ) {
        Ok(())
    } else {
        Err(EvidenceAuthError::BadSignature)
    }
}
