//! Structural explanation engine. Explanations are derived from graph
//! semantics, never from natural-language prose.

use crate::error::RuntimeError;
use crate::runtime::Runtime;
use axiom_core::{ClaimStatus, Id, Type, Uncertainty, Value};

#[derive(Debug, Clone)]
pub struct DerivationInfo {
    pub op: String,
    pub op_version: String,
    pub inputs: Vec<Id>,
    pub receipt: Option<Id>,
    pub output: Id,
}

#[derive(Debug, Clone)]
pub struct ObligationInfo {
    pub kind: String,
    pub severity: String,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct Explanation {
    pub claim: Id,
    pub label: String,
    pub status: ClaimStatus,
    pub ty: Type,
    pub value: Value,
    pub context: Id,
    pub uncertainty: Uncertainty,
    pub derivation: Option<DerivationInfo>,
    pub obligations: Vec<ObligationInfo>,
    pub assumptions: Vec<Id>,
    pub evidence: Vec<Id>,
    pub contradicts: Vec<Id>,
    pub invalidation_conditions: Vec<String>,
}

impl Runtime {
    pub fn explain(&self, label: &str) -> Result<Explanation, RuntimeError> {
        let id = self.claim(label)?;
        let c = self
            .module
            .claims
            .get(&id)
            .ok_or_else(|| RuntimeError::UnknownLabel(label.into()))?;
        let mut derivation = None;
        if let Some(did) = &c.derivation {
            if let Some(d) = self.module.derivations.get(did) {
                let opname = self
                    .module
                    .operations
                    .get(&d.op)
                    .map(|o| o.name.clone())
                    .unwrap_or_else(|| d.op.as_str());
                derivation = Some(DerivationInfo {
                    op: opname,
                    op_version: d.op_version.clone(),
                    inputs: d.inputs.clone(),
                    receipt: d.receipt.clone(),
                    output: d.output.clone(),
                });
            }
        }
        let mut obligations = vec![];
        for oid in &c.obligations {
            if let Some(o) = self.module.obligations.get(oid) {
                obligations.push(ObligationInfo {
                    kind: o.kind.name(),
                    severity: if o.severity == axiom_core::Severity::Mandatory {
                        "mandatory"
                    } else {
                        "advisory"
                    }
                    .into(),
                    state: format!("{:?}", o.state),
                });
            }
        }
        let contradicts = self
            .module
            .contradictions
            .iter()
            .filter(|(_, con)| con.claims.contains(&id))
            .map(|(cid, _)| cid.clone())
            .collect();
        Ok(Explanation {
            claim: id,
            label: c.label.clone(),
            status: c.status,
            ty: c.ty.clone(),
            value: c.value.clone(),
            context: c.context_id.clone(),
            uncertainty: c.uncertainty.clone(),
            derivation,
            obligations,
            assumptions: c.assumptions.clone(),
            evidence: c.evidence.clone(),
            contradicts,
            invalidation_conditions: c.invalidation_conditions.clone(),
        })
    }

    /// All contradiction witnesses in the module.
    pub fn contradictions(&self) -> Vec<axiom_core::Contradiction> {
        self.module.contradictions.values().cloned().collect()
    }

    /// Structural diff between two claims' provenance (used for the
    /// competing-models demonstration). Returns the set of differing inputs.
    pub fn provenance_diff(&self, a: &str, b: &str) -> Result<Vec<String>, RuntimeError> {
        let ia = self.claim(a)?;
        let ib = self.claim(b)?;
        let ca = &self.module.claims[&ia];
        let cb = &self.module.claims[&ib];
        let mut diffs = vec![];
        if ca.value != cb.value {
            diffs.push(format!(
                "value: {} vs {}",
                ca.value.value_display(),
                cb.value.value_display()
            ));
        }
        if ca.ty != cb.ty {
            diffs.push("type".into());
        }
        if ca.assumptions != cb.assumptions {
            diffs.push("assumptions".into());
        }
        if ca.evidence != cb.evidence {
            diffs.push("evidence".into());
        }
        Ok(diffs)
    }
}

/// Small helper to render a value compactly for explanations.
trait ValueDisplay {
    fn value_display(&self) -> String;
}

impl ValueDisplay for Value {
    fn value_display(&self) -> String {
        match self {
            Value::Bool(b) => b.to_string(),
            Value::Num(n) => n.display(),
            Value::Str(s) => format!("\"{}\"", s),
            Value::Sym(s) => format!("'{}", s),
            Value::Quantity(q) => q.display(),
            Value::Interval { lo, hi } => format!("[{}, {}]", lo.display(), hi.display()),
            Value::Relation(_) => "relation".into(),
            Value::Record(_) => "record".into(),
            Value::Extension { .. } => "extension".into(),
        }
    }
}
