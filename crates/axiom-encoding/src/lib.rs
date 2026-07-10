//! Canonical encoding and content addressing for Axiom IR.
//!
//! Two responsibilities:
//!
//! 1. **Canonical JSON.** A deterministic serialization of `serde_json::Value`
//!    with recursively sorted object keys, no insignificant whitespace, and
//!    stable integer formatting. The same semantic module always produces the
//!    same canonical bytes.
//!
//! 2. **Domain-separated content addressing.** Every identifier is a hash over
//!    `domain_tag || 0x00 || canonical_bytes`, so structurally different object
//!    categories can never collide through identical raw serialization.

use axiom_types::Num;
use serde::Serialize;
use serde_json::{Map, Value as J};
use sha2::{Digest as _, Sha256};
use std::fmt;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum EncodingError {
    #[error("serialization failed: {0}")]
    Serialize(String),
}

/// Categories of addressable objects. The tag is prepended to every digest so
/// a claim digest can never equal an evidence digest with identical content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Domain {
    Claim,
    Evidence,
    Assumption,
    Context,
    Derivation,
    Obligation,
    Contradiction,
    Receipt,
    Module,
    Operation,
    Event,
    Extension,
}

impl Domain {
    pub fn tag(self) -> &'static str {
        match self {
            Domain::Claim => "claim",
            Domain::Evidence => "evidence",
            Domain::Assumption => "assumption",
            Domain::Context => "context",
            Domain::Derivation => "derivation",
            Domain::Obligation => "obligation",
            Domain::Contradiction => "contradiction",
            Domain::Receipt => "receipt",
            Domain::Module => "module",
            Domain::Operation => "operation",
            Domain::Event => "event",
            Domain::Extension => "extension",
        }
    }
}

/// A content-addressed identifier. The textual form is
/// `<domain>.1.<hex-digest>`, making the category explicit and greppable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id {
    domain: Domain,
    digest: [u8; 32],
}

impl Id {
    pub fn domain(&self) -> Domain {
        self.domain
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn as_str(&self) -> String {
        let hex: String = self.digest.iter().map(|b| format!("{b:02x}")).collect();
        format!("{}.1.{}", self.domain.tag(), hex)
    }

    pub fn parse(s: &str) -> Option<Id> {
        let (domain, rest) = s.split_once('.')?;
        let (ver, hex) = rest.split_once('.')?;
        if ver != "1" {
            return None;
        }
        let domain = match domain {
            "claim" => Domain::Claim,
            "evidence" => Domain::Evidence,
            "assumption" => Domain::Assumption,
            "context" => Domain::Context,
            "derivation" => Domain::Derivation,
            "obligation" => Domain::Obligation,
            "contradiction" => Domain::Contradiction,
            "receipt" => Domain::Receipt,
            "module" => Domain::Module,
            "operation" => Domain::Operation,
            "event" => Domain::Event,
            "extension" => Domain::Extension,
            _ => return None,
        };
        let bytes = hex::decode(hex).ok()?;
        if bytes.len() != 32 {
            return None;
        }
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&bytes);
        Some(Id { domain, digest })
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for Id {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Id {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Id::parse(&s).ok_or_else(|| serde::de::Error::custom("invalid axiom id"))
    }
}

/// Compute the canonical bytes of any serializable value.
pub fn canonical_bytes<T: Serialize>(v: &T) -> Result<Vec<u8>, EncodingError> {
    let val = serde_json::to_value(v).map_err(|e| EncodingError::Serialize(e.to_string()))?;
    let sorted = sort_value(&val);
    let bytes = serde_json::to_vec(&sorted).map_err(|e| EncodingError::Serialize(e.to_string()))?;
    Ok(bytes)
}

/// Compute a content-addressed [`Id`] for a value in a given [`Domain`].
pub fn content_id<T: Serialize>(domain: Domain, v: &T) -> Result<Id, EncodingError> {
    let bytes = canonical_bytes(v)?;
    Ok(hash_domain(domain, &bytes))
}

/// Hash canonical bytes under a domain tag (domain || 0x00 || bytes).
pub fn hash_domain(domain: Domain, canonical: &[u8]) -> Id {
    let mut hasher = Sha256::new();
    hasher.update(domain.tag().as_bytes());
    hasher.update([0u8]);
    hasher.update(canonical);
    let digest: [u8; 32] = hasher.finalize().into();
    Id { domain, digest }
}

/// Hash a list of already-canonical byte blobs (e.g. several inputs) together.
pub fn hash_concat(domain: Domain, parts: &[&[u8]]) -> Id {
    let mut hasher = Sha256::new();
    hasher.update(domain.tag().as_bytes());
    hasher.update([0u8]);
    for p in parts {
        hasher.update((p.len() as u64).to_le_bytes());
        hasher.update(p);
    }
    let digest: [u8; 32] = hasher.finalize().into();
    Id { domain, digest }
}

fn sort_value(v: &J) -> J {
    match v {
        J::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                out.insert(k.clone(), sort_value(&map[k]));
            }
            J::Object(out)
        }
        J::Array(arr) => J::Array(arr.iter().map(sort_value).collect()),
        other => other.clone(),
    }
}

/// Helper to render a [`Num`] as canonical JSON text (string form).
pub fn num_to_json(n: &Num) -> J {
    serde_json::to_value(n).expect("Num serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_order_independent() {
        let a: J = serde_json::from_str(r#"{"b":1,"a":2}"#).unwrap();
        let b: J = serde_json::from_str(r#"{"a":2,"b":1}"#).unwrap();
        assert_eq!(canonical_bytes(&a).unwrap(), canonical_bytes(&b).unwrap());
    }

    #[test]
    fn domain_separation() {
        let v: J = serde_json::json!({"x":1});
        let c = content_id(Domain::Claim, &v).unwrap();
        let e = content_id(Domain::Evidence, &v).unwrap();
        assert_ne!(c, e);
        assert_ne!(c.as_str(), e.as_str());
    }

    #[test]
    fn id_roundtrip() {
        let v: J = serde_json::json!({"x":1});
        let id = content_id(Domain::Claim, &v).unwrap();
        assert_eq!(Id::parse(&id.as_str()), Some(id));
    }
}
