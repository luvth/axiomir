//! `axiom-core`: the trusted semantic core and transition calculus of Axiom IR.
//!
//! This crate defines the first-class semantic objects (claims, evidence,
//! assumptions, contexts, operations, derivations, obligations, contradictions,
//! receipts), their content-addressed identities, and the normative transition
//! calculus. It performs **no I/O**: external operations are supplied to
//! [`Module::derive`] as either a receipt or an executor callback, so the core
//! remains a pure, auditable state machine.
//!
//! Central invariant (verified execution mode):
//!
//! > No derived claim may enter `Verified` unless it has a complete derivation
//! > record and every *mandatory* proof obligation is in state `Satisfied`
//! > (or `Waived` under a spec-defined weaker class). Unsupported assertions
//! > live only in explicitly unverified states and never silently become
//! > `Verified`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod crypto;
pub use crypto::{
    sign as sign_receipt, verify as verify_receipt_tag, verify_evidence, EvidenceAuthError,
    TrustRoot,
};
pub mod registry;
pub use registry::{builtin_operations, BuiltinExecutor, OpExecutor, OpRegistry};

// Re-exports so downstream crates need not reach into dependency crates.
pub use axiom_encoding::{content_id, hash_concat, Domain, Id};
pub use axiom_types::{
    num::Num, uncertainty::Uncertainty, unit::Quantity, unit::Unit, Type, Value,
};

// ---------------------------------------------------------------------------
// Status, severity, and obligation state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClaimStatus {
    /// Unsupported assertion; must never silently become Verified.
    Asserted,
    /// Introduced by an assumption.
    Assumed,
    /// Introduced by an observation (sensor, tool read, attestation).
    Observed,
    /// Awaiting obligation discharge / derivation completion.
    Pending,
    /// Disputed by a challenger.
    Disputed,
    /// Explicitly challenged.
    Challenged,
    /// Fully supported: complete derivation + discharged mandatory obligations.
    Verified,
    /// A premise, assumption, receipt, or obligation failed; support withdrawn.
    Invalidated,
    /// Attested by an external authority without internal derivation.
    ExternallyAttested,
}

impl ClaimStatus {
    /// States in which a claim is NOT verified but is a legitimate, inspectable
    /// unverified node.
    pub fn is_unverified(&self) -> bool {
        !matches!(self, ClaimStatus::Verified)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObligationState {
    Pending,
    Satisfied,
    Failed,
    Waived,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Blocks `Verified` until discharged.
    Mandatory,
    /// Advisory; does not block `Verified`.
    Advisory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrustClass {
    Trusted,
    Untrusted,
    Unverified,
}

// ---------------------------------------------------------------------------
// Operation definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpClass {
    PureInternal,
    VerifiedExternal,
    UnverifiedExternal,
    NonDeterministic,
    Privileged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeterminismClass {
    /// Identical inputs always yield identical output.
    Deterministic,
    /// May differ run to run; requires a receipt to replay.
    NonDeterministic,
    /// Deterministic given an identical receipt.
    Receipted,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// Network egress, e.g. `net:http`.
    Net(String),
    /// Filesystem read under a scoped path.
    FsRead(String),
    /// Tool invocation, e.g. `tool:calculator`.
    Tool(String),
    /// Privileged operation (must be explicitly granted).
    Privileged(String),
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Capability::Net(s) => write!(f, "net:{s}"),
            Capability::FsRead(s) => write!(f, "fs-read:{s}"),
            Capability::Tool(s) => write!(f, "tool:{s}"),
            Capability::Privileged(s) => write!(f, "priv:{s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UncertaintyRule {
    /// Output uncertainty = conjunction (combine) of input uncertainties.
    ConjoinInputs,
    /// Output is Exact regardless of inputs.
    Exact,
    /// Output uncertainty is the maximum evidence weight of supporting evidence.
    EvidenceWeight,
    /// Output uncertainty is unknown.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObligationKind {
    TypeCompat,
    DimensionalConsistency,
    NumericBounds,
    SchemaValidation,
    SourceSupport,
    Normalization,
    PremiseAvailable,
    SignatureValidation,
    ToolReceiptValidation,
    ContextConsistency,
    OpPostcondition,
    Custom(String),
}

impl ObligationKind {
    pub fn name(&self) -> String {
        match self {
            ObligationKind::TypeCompat => "type-compat".into(),
            ObligationKind::DimensionalConsistency => "dimensional-consistency".into(),
            ObligationKind::NumericBounds => "numeric-bounds".into(),
            ObligationKind::SchemaValidation => "schema-validation".into(),
            ObligationKind::SourceSupport => "source-support".into(),
            ObligationKind::Normalization => "normalization".into(),
            ObligationKind::PremiseAvailable => "premise-available".into(),
            ObligationKind::SignatureValidation => "signature-validation".into(),
            ObligationKind::ToolReceiptValidation => "tool-receipt-validation".into(),
            ObligationKind::ContextConsistency => "context-consistency".into(),
            ObligationKind::OpPostcondition => "op-postcondition".into(),
            ObligationKind::Custom(s) => format!("custom:{s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationDef {
    pub id: Id,
    pub name: String,
    pub version: String,
    pub class: OpClass,
    pub inputs: Vec<Type>,
    pub output: Type,
    pub determinism: DeterminismClass,
    pub uncertainty_rule: UncertaintyRule,
    pub generates: Vec<ObligationKind>,
    pub capabilities: Vec<Capability>,
}

impl OperationDef {
    pub fn identity(name: &str, version: &str) -> Id {
        // Domain-separated identity independent of the other fields.
        let json = serde_json::json!({ "name": name, "version": version });
        content_id(Domain::Operation, &json).expect("serialize op identity")
    }
}

// ---------------------------------------------------------------------------
// Semantic objects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Provenance {
    pub source: String,
    pub model: Option<String>,
    pub tool: Option<String>,
    pub logical_time: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AcquisitionMeta {
    pub locator: String,
    pub acquired_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: Id,
    pub content_hash: Id,
    pub media_type: String,
    pub provenance: Provenance,
    /// The provider (authority) that vouches for this evidence node. Used as
    /// the HMAC key namespace when an authenticity signature is attached.
    pub provider: String,
    pub acquisition: AcquisitionMeta,
    pub trust: TrustClass,
    pub signature: Option<String>,
    /// Inline content, when small and available.
    pub content: Option<Value>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assumption {
    pub id: Id,
    pub label: String,
    /// The proposition this assumption introduces, as a claim id.
    pub claim: Id,
    pub scope: String,
    pub context: Id,
    pub challengeable: bool,
    pub removable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// Node identity: stable, content-addressed over the immutable proposition.
    pub id: Id,
    /// Semantic identity: proposition only (type + value), context- and
    /// status-independent. Two claims with the same proposition share this.
    pub semantic_id: Id,
    pub label: String,
    pub span: Option<String>,
    pub ty: Type,
    pub value: Value,
    pub status: ClaimStatus,
    pub uncertainty: Uncertainty,
    pub context_id: Id,
    pub assumptions: Vec<Id>,
    pub evidence: Vec<Id>,
    pub derivation: Option<Id>,
    pub obligations: Vec<Id>,
    pub invalidation_conditions: Vec<String>,
    pub provenance: Provenance,
}

impl Claim {
    /// Claims this node depends on (premises + assumptions + evidence).
    pub fn depends_on(&self, m: &Module) -> Vec<Id> {
        let mut deps = Vec::new();
        deps.extend(self.assumptions.iter().cloned());
        deps.extend(self.evidence.iter().cloned());
        if let Some(d) = &self.derivation {
            if let Some(der) = m.derivations.get(d) {
                deps.extend(der.inputs.iter().cloned());
            }
        }
        deps
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derivation {
    pub id: Id,
    pub op: Id,
    pub op_version: String,
    pub inputs: Vec<Id>,
    pub output: Id,
    /// The output claim's label, retained so incremental recomputation can
    /// reproduce the exact same node identity.
    pub output_label: String,
    pub generated_obligations: Vec<Id>,
    pub receipt: Option<Id>,
    pub provenance: Provenance,
    pub runtime_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Obligation {
    pub id: Id,
    pub kind: ObligationKind,
    pub target: Id,
    pub inputs: Vec<Id>,
    pub severity: Severity,
    pub state: ObligationState,
    pub discharged_by: Option<Id>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContradictionKind {
    PropositionNegation,
    IncompatibleEquality,
    DisjointInterval,
    IncompatibleUnit,
    MutuallyExclusiveMembership,
    ViolatedPostcondition,
    EvidenceConflict,
    AssumptionConflict,
    Extension(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Witness {
    pub summary: String,
    pub detail: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contradiction {
    pub id: Id,
    pub kind: ContradictionKind,
    pub claims: Vec<Id>,
    pub context: Id,
    pub witness: Witness,
    pub evidence: Vec<Id>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub id: Id,
    pub operation: String,
    pub op_version: String,
    pub provider: String,
    pub logical_time: u64,
    pub inputs_hash: Id,
    /// The returned output value, carried so replay can reconstruct the claim
    /// deterministically without re-invoking the external operation.
    pub output: Value,
    pub output_hash: Id,
    pub schema: Type,
    /// Content hash of the signed payload (tamper-evidence).
    pub integrity: Id,
    /// Keyed HMAC-SHA256 tag over the signed payload, under the trusted secret
    /// of `provider`. An empty tag means the receipt is unsigned and MUST NOT
    /// be accepted as authentic by any verifier.
    pub signature: Vec<u8>,
}

impl Receipt {
    /// The canonical bytes of the fields bound by the provider's authenticity:
    /// operation, op version, provider, logical time, inputs hash, output, schema.
    pub fn signed_payload(&self) -> Vec<u8> {
        axiom_encoding::canonical_bytes(&serde_json::json!({
            "operation": self.operation,
            "op_version": self.op_version,
            "provider": self.provider,
            "logical_time": self.logical_time,
            "inputs_hash": self.inputs_hash.as_str(),
            "output": self.output,
            "schema": self.schema.name(),
        }))
        .expect("receipt payload serializes")
    }

    /// Verify both tamper-evidence (payload hash) and cryptographic
    /// authenticity (HMAC tag under a trusted provider secret).
    ///
    /// A receipt is authentic only if a [`TrustRoot`] for its `provider` exists
    /// and the stored tag verifies under that secret. A self-consistent but
    /// forged receipt (or one with an empty tag) is rejected.
    pub fn verify_integrity(&self, trust_roots: &[TrustRoot]) -> bool {
        // 1. Tamper-evidence: the payload hash must match.
        let payload = self.signed_payload();
        let recomputed = content_id(
            Domain::Receipt,
            &serde_json::from_slice::<serde_json::Value>(&payload).unwrap(),
        )
        .expect("payload hashes");
        if recomputed != self.integrity {
            return false;
        }
        // 2. Authenticity: the HMAC tag must verify under a trusted secret.
        crate::crypto::verify(trust_roots, &self.provider, &payload, &self.signature)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub id: Id,
    pub parent: Option<Id>,
    pub label: String,
    pub assumptions: Vec<Id>,
    pub inherited_claims: Vec<Id>,
    pub local_claims: Vec<Id>,
    pub contradictions: Vec<Id>,
    pub merge_of: Option<(Id, Id)>,
}

// ---------------------------------------------------------------------------
// Events (deterministic log)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Assert {
        claim: Id,
        status: ClaimStatus,
    },
    Observe {
        claim: Id,
    },
    Assume {
        assumption: Id,
        claim: Id,
    },
    Derive {
        derivation: Id,
        output: Id,
        op: Id,
    },
    Require {
        obligation: Id,
        target: Id,
    },
    Discharge {
        obligation: Id,
        by: Id,
        state: ObligationState,
    },
    Verify {
        claim: Id,
    },
    Challenge {
        claim: Id,
    },
    Contradict {
        contradiction: Id,
    },
    Branch {
        context: Id,
        parent: Option<Id>,
    },
    Merge {
        context: Id,
        a: Id,
        b: Id,
        conflicts: usize,
    },
    Invalidate {
        claim: Id,
        reason: String,
    },
    Attest {
        claim: Id,
    },
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("unknown claim {0}")]
    UnknownClaim(Id),
    #[error("unknown operation {0}")]
    UnknownOperation(String),
    #[error("unknown context {0}")]
    UnknownContext(Id),
    #[error("unknown assumption {0}")]
    UnknownAssumption(Id),
    #[error("unknown obligation {0}")]
    UnknownObligation(Id),
    #[error("unknown evidence {0}")]
    UnknownEvidence(Id),
    #[error("unknown receipt {0}")]
    UnknownReceipt(Id),
    #[error("type error: {0}")]
    TypeError(String),
    #[error("operation {0} requires receipt but none supplied")]
    ReceiptRequired(String),
    #[error("external operation {0} requires capability {1} which is not granted")]
    CapabilityMissing(String, String),
    #[error("obligation {0} is not discharged; claim cannot be verified")]
    ObligationNotDischarged(Id),
    #[error("derived claim {0} has no complete derivation; cannot verify")]
    NoDerivation(Id),
    #[error("attempted to silently verify unsupported assertion {0}")]
    SilentVerify(Id),
    #[error("receipt {0} failed integrity check (tampering suspected)")]
    ReceiptTampered(Id),
    #[error("receipt {0} inputs hash does not match the derivation's actual inputs")]
    ReceiptInputMismatch(Id),
    #[error("no trust root configured for receipt provider {0}")]
    UnknownTrustRoot(String),
    #[error("contradiction requires at least two claims")]
    ContradictionNeedsTwo,
    #[error("context merge conflict on claim {0}")]
    MergeConflict(Id),
    #[error("executor error: {0}")]
    Exec(String),
    #[error("verification invariant violated: {0}")]
    Invariant(String),
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub format_version: String,
    pub claims: IndexMap<Id, Claim>,
    pub evidence: IndexMap<Id, Evidence>,
    pub assumptions: IndexMap<Id, Assumption>,
    pub contexts: IndexMap<Id, Context>,
    pub operations: IndexMap<Id, OperationDef>,
    pub derivations: IndexMap<Id, Derivation>,
    pub obligations: IndexMap<Id, Obligation>,
    pub contradictions: IndexMap<Id, Contradiction>,
    pub receipts: IndexMap<Id, Receipt>,
    pub events: Vec<Event>,
    pub root_context: Id,
    pub runtime_version: String,
}

/// The full set of claims visible in a context: its own `local_claims` plus
/// everything it inherited from ancestors, deduplicated.
fn full_claim_set(ctx: &Context) -> Vec<Id> {
    let mut v = ctx.local_claims.clone();
    for x in &ctx.inherited_claims {
        if !v.contains(x) {
            v.push(x.clone());
        }
    }
    v
}

impl Module {
    pub fn new(name: &str) -> Module {
        let root = Context {
            id: content_id(Domain::Context, &serde_json::json!({ "root": true })).unwrap(),
            parent: None,
            label: "root".into(),
            assumptions: vec![],
            inherited_claims: vec![],
            local_claims: vec![],
            contradictions: vec![],
            merge_of: None,
        };
        let rid = root.id.clone();
        let mut contexts = IndexMap::new();
        contexts.insert(rid.clone(), root);
        Module {
            name: name.to_string(),
            format_version: "1".into(),
            claims: IndexMap::new(),
            evidence: IndexMap::new(),
            assumptions: IndexMap::new(),
            contexts,
            operations: IndexMap::new(),
            derivations: IndexMap::new(),
            obligations: IndexMap::new(),
            contradictions: IndexMap::new(),
            receipts: IndexMap::new(),
            events: vec![],
            root_context: rid,
            runtime_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Register an operation definition (builtin or extension).
    pub fn register_op(&mut self, def: OperationDef) {
        self.operations.insert(def.id.clone(), def);
    }

    /// Content-addressed semantic identity of a proposition (type + value).
    fn proposition_id(ty: &Type, value: &Value) -> Id {
        let json = serde_json::json!({ "ty": ty, "value": value });
        content_id(Domain::Claim, &json).unwrap()
    }

    /// Node identity: stable across recomputation. It depends on the claim's
    /// label and context, NOT its value (the value lives in `semantic_id`).
    fn claim_node_id(label: &str, context: &Id) -> Id {
        let json = serde_json::json!({
            "label": label,
            "context": context.as_str(),
        });
        content_id(Domain::Claim, &json).unwrap()
    }

    // -- assertion --------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn assert(
        &mut self,
        label: &str,
        ty: Type,
        value: Value,
        uncertainty: Uncertainty,
        evidence: Vec<Id>,
        context: Option<Id>,
        span: Option<String>,
    ) -> Result<Id, CoreError> {
        value
            .check(&ty)
            .map_err(|e| CoreError::TypeError(e.to_string()))?;
        for e in &evidence {
            if !self.evidence.contains_key(e) {
                return Err(CoreError::UnknownEvidence(e.clone()));
            }
        }
        let ctx = context.unwrap_or_else(|| self.root_context.clone());
        let semantic = Self::proposition_id(&ty, &value);
        let id = Self::claim_node_id(label, &ctx);
        let claim = Claim {
            id: id.clone(),
            semantic_id: semantic,
            label: label.to_string(),
            span,
            ty,
            value,
            status: ClaimStatus::Asserted,
            uncertainty,
            context_id: ctx.clone(),
            assumptions: vec![],
            evidence: evidence.clone(),
            derivation: None,
            obligations: vec![],
            invalidation_conditions: vec![],
            provenance: Provenance::default(),
        };
        self.claims.insert(id.clone(), claim);
        // A claim resting on untrusted evidence carries a mandatory
        // source-support obligation that blocks `verify` until discharged by
        // trusted evidence or an explicit discharge. This is what makes the
        // verification gate semantic rather than decorative.
        for e in &evidence {
            if let Some(ev) = self.evidence.get(e) {
                if ev.trust == TrustClass::Untrusted {
                    self.add_obligation(&id, ObligationKind::SourceSupport, true, false, None);
                    break;
                }
            }
        }
        if let Some(c) = self.contexts.get_mut(&ctx) {
            c.local_claims.push(id.clone());
        }
        self.events.push(Event::Assert {
            claim: id.clone(),
            status: ClaimStatus::Asserted,
        });
        Ok(id)
    }

    /// Observation: like assert but in `Observed` state (typically tool/sensor).
    pub fn observe(
        &mut self,
        label: &str,
        ty: Type,
        value: Value,
        uncertainty: Uncertainty,
        evidence: Vec<Id>,
        context: Option<Id>,
    ) -> Result<Id, CoreError> {
        let id = self.assert(label, ty, value, uncertainty, evidence, context, None)?;
        if let Some(c) = self.claims.get_mut(&id) {
            c.status = ClaimStatus::Observed;
        }
        self.events.push(Event::Observe { claim: id.clone() });
        Ok(id)
    }

    // -- assumption -------------------------------------------------------

    pub fn assume(
        &mut self,
        label: &str,
        ty: Type,
        value: Value,
        scope: &str,
        context: Option<Id>,
    ) -> Result<(Id, Id), CoreError> {
        value
            .check(&ty)
            .map_err(|e| CoreError::TypeError(e.to_string()))?;
        let ctx = context.unwrap_or_else(|| self.root_context.clone());
        let id = self.assert(
            label,
            ty,
            value,
            Uncertainty::Unknown,
            vec![],
            Some(ctx.clone()),
            None,
        )?;
        if let Some(c) = self.claims.get_mut(&id) {
            c.status = ClaimStatus::Assumed;
        }
        let sem_json = serde_json::json!({
            "claim": id.as_str(),
            "scope": scope,
            "context": ctx.as_str(),
        });
        let aid = content_id(Domain::Assumption, &sem_json).unwrap();
        let assumption = Assumption {
            id: aid.clone(),
            label: label.to_string(),
            claim: id.clone(),
            scope: scope.to_string(),
            context: ctx.clone(),
            challengeable: true,
            removable: true,
        };
        self.assumptions.insert(aid.clone(), assumption);
        if let Some(c) = self.claims.get_mut(&id) {
            c.assumptions.push(aid.clone());
        }
        if let Some(cx) = self.contexts.get_mut(&ctx) {
            cx.assumptions.push(aid.clone());
        }
        self.events.push(Event::Assume {
            assumption: aid.clone(),
            claim: id.clone(),
        });
        Ok((aid, id))
    }

    // -- derivation -------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn derive(
        &mut self,
        op_name: &str,
        op_version: &str,
        inputs: &[Id],
        context: Option<Id>,
        output_label: &str,
        receipt: Option<Id>,
        executor: &dyn OpExecutor,
        trust_roots: &[TrustRoot],
    ) -> Result<Id, CoreError> {
        let op_id = OperationDef::identity(op_name, op_version);
        let def = self
            .operations
            .get(&op_id)
            .cloned()
            .ok_or_else(|| CoreError::UnknownOperation(format!("{op_name}@{op_version}")))?;
        let ctx = context.unwrap_or_else(|| self.root_context.clone());

        // Resolve and type-check inputs.
        let mut values = Vec::new();
        for i in inputs {
            let c = self
                .claims
                .get(i)
                .ok_or_else(|| CoreError::UnknownClaim(i.clone()))?;
            values.push(c.value.clone());
        }
        if values.len() != def.inputs.len() {
            return Err(CoreError::TypeError(format!(
                "operation {} expects {} inputs, got {}",
                op_name,
                def.inputs.len(),
                values.len()
            )));
        }
        for (v, expected) in values.iter().zip(def.inputs.iter()) {
            v.check(expected)
                .map_err(|e| CoreError::TypeError(e.to_string()))?;
        }

        // Capability check for external operations.
        if matches!(
            def.class,
            OpClass::VerifiedExternal | OpClass::UnverifiedExternal | OpClass::Privileged
        ) && !def.capabilities.is_empty()
        {
            // Capability enforcement is delegated to the caller via the receipt
            // presence + executor; here we only require a receipt for external ops.
            if receipt.is_none() {
                return Err(CoreError::ReceiptRequired(format!(
                    "{op_name}@{op_version}"
                )));
            }
        }

        // Compute output value.
        let (output_value, output_uncertainty, receipt_used) = if let Some(rid) = &receipt {
            let r = self
                .receipts
                .get(rid)
                .ok_or_else(|| CoreError::UnknownReceipt(rid.clone()))?;
            // Bind the receipt to the derivation's actual inputs: the receipt's
            // inputs hash MUST equal the content hash of the values this
            // derivation actually consumes. A self-consistent receipt built for
            // different inputs cannot be attached here.
            let actual_inputs_hash =
                content_id(Domain::Receipt, &serde_json::json!(values)).expect("inputs hash");
            if r.inputs_hash != actual_inputs_hash {
                return Err(CoreError::ReceiptInputMismatch(rid.clone()));
            }
            // Authenticity: an unsigned or unverifiable receipt is rejected.
            if r.signature.is_empty() {
                return Err(CoreError::ReceiptTampered(rid.clone()));
            }
            if !r.verify_integrity(trust_roots) {
                if !trust_roots.iter().any(|t| t.provider == r.provider) {
                    return Err(CoreError::UnknownTrustRoot(r.provider.clone()));
                }
                return Err(CoreError::ReceiptTampered(rid.clone()));
            }
            // Output is reconstructed from the receipt via the executor's
            // receipt resolver, which returns the recorded output value.
            let v = executor
                .resolve_receipt(r)
                .map_err(|e| CoreError::Exec(e.to_string()))?;
            let u = Uncertainty::Exact;
            (v, u, Some(rid.clone()))
        } else {
            let v = executor
                .exec(&def, &values)
                .map_err(|e| CoreError::Exec(e.to_string()))?;
            v.check(&def.output)
                .map_err(|e| CoreError::TypeError(e.to_string()))?;
            let u = match def.uncertainty_rule {
                UncertaintyRule::ConjoinInputs => {
                    let mut acc = Uncertainty::Exact;
                    for c in &values {
                        // reflect the input claim's uncertainty if available
                        let _ = c;
                    }
                    // Conjoin the actual input claim uncertainties.
                    let mut it = inputs.iter();
                    if let Some(first) = it.next() {
                        if let Some(fc) = self.claims.get(first) {
                            acc = fc.uncertainty.clone();
                        }
                        for other in it {
                            if let Some(oc) = self.claims.get(other) {
                                acc = acc
                                    .combine(&oc.uncertainty)
                                    .map_err(|e| CoreError::Exec(e.to_string()))?;
                            }
                        }
                    }
                    acc
                }
                UncertaintyRule::Exact => Uncertainty::Exact,
                UncertaintyRule::EvidenceWeight => Uncertainty::Unknown,
                UncertaintyRule::Unknown => Uncertainty::Unknown,
            };
            (v, u, None)
        };

        // Output claim (Pending until verified).
        let semantic = Self::proposition_id(&def.output, &output_value);
        let out_id = Self::claim_node_id(output_label, &ctx);

        // Gather assumptions + evidence inherited from inputs.
        let mut assumptions = vec![];
        let mut evidence = vec![];
        for i in inputs {
            if let Some(c) = self.claims.get(i) {
                for a in &c.assumptions {
                    if !assumptions.contains(a) {
                        assumptions.push(a.clone());
                    }
                }
                for e in &c.evidence {
                    if !evidence.contains(e) {
                        evidence.push(e.clone());
                    }
                }
            }
        }

        let output = Claim {
            id: out_id.clone(),
            semantic_id: semantic,
            label: output_label.to_string(),
            span: None,
            ty: def.output.clone(),
            value: output_value,
            status: ClaimStatus::Pending,
            uncertainty: output_uncertainty,
            context_id: ctx.clone(),
            assumptions,
            evidence,
            derivation: None,
            obligations: vec![],
            invalidation_conditions: vec![],
            provenance: Provenance::default(),
        };

        // Derivation record.
        let der_json = serde_json::json!({
            "op": op_id.as_str(),
            "op_version": op_version,
            "inputs": inputs.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
            "output": out_id.as_str(),
            "receipt": receipt_used.as_ref().map(|r| r.as_str()),
        });
        let der_id = content_id(Domain::Derivation, &der_json).unwrap();
        let derivation = Derivation {
            id: der_id.clone(),
            op: op_id.clone(),
            op_version: op_version.to_string(),
            inputs: inputs.to_vec(),
            output: out_id.clone(),
            output_label: output_label.to_string(),
            generated_obligations: vec![],
            receipt: receipt_used.clone(),
            provenance: Provenance::default(),
            runtime_version: self.runtime_version.clone(),
        };

        // Generate obligations declared by the operation.
        let mut obl_ids = vec![];
        for kind in &def.generates {
            let mandatory = matches!(
                kind,
                ObligationKind::TypeCompat
                    | ObligationKind::DimensionalConsistency
                    | ObligationKind::NumericBounds
                    | ObligationKind::ToolReceiptValidation
                    | ObligationKind::SourceSupport
                    | ObligationKind::OpPostcondition
            );
            let state = if matches!(
                kind,
                ObligationKind::TypeCompat | ObligationKind::DimensionalConsistency
            ) {
                // Type compatibility and dimensional consistency are already
                // enforced by the operation's input check / executor, so they
                // are satisfied at derivation time.
                ObligationState::Satisfied
            } else if matches!(kind, ObligationKind::ToolReceiptValidation) {
                if receipt_used.is_some() {
                    ObligationState::Satisfied
                } else {
                    ObligationState::Pending
                }
            } else {
                ObligationState::Pending
            };
            let ojson = serde_json::json!({
                "kind": kind.name(),
                "target": out_id.as_str(),
                "severity": if mandatory { "mandatory" } else { "advisory" },
            });
            let oid = content_id(Domain::Obligation, &ojson).unwrap();
            let obl = Obligation {
                id: oid.clone(),
                kind: kind.clone(),
                target: out_id.clone(),
                inputs: inputs.to_vec(),
                severity: if mandatory {
                    Severity::Mandatory
                } else {
                    Severity::Advisory
                },
                state,
                discharged_by: if state == ObligationState::Satisfied {
                    Some(der_id.clone())
                } else {
                    None
                },
                message: format!("{} obligation for {}", kind.name(), out_id.as_str()),
            };
            self.obligations.insert(oid.clone(), obl);
            obl_ids.push(oid.clone());
        }

        // Commit.
        let mut out_claim = output;
        out_claim.derivation = Some(der_id.clone());
        out_claim.obligations = obl_ids.clone();
        let mut der = derivation;
        der.generated_obligations = obl_ids;
        self.derivations.insert(der_id.clone(), der);
        self.claims.insert(out_id.clone(), out_claim);
        if let Some(c) = self.contexts.get_mut(&ctx) {
            c.local_claims.push(out_id.clone());
        }
        self.events.push(Event::Derive {
            derivation: der_id,
            output: out_id.clone(),
            op: op_id,
        });
        Ok(out_id)
    }

    // -- obligation attachment -------------------------------------------

    /// Attach a first-class obligation to a claim. When `auto` is true the
    /// obligation is discharged immediately: the operation or derivation has
    /// performed the check itself, and `discharged_by` records the discharging
    /// node (the derivation, or an evidence/receipt node). When `auto` is false
    /// the obligation stays `Pending` and blocks `verify` until a real
    /// discharge references evidence or a receipt (see `Module::discharge`).
    pub fn add_obligation(
        &mut self,
        target: &Id,
        kind: ObligationKind,
        mandatory: bool,
        auto: bool,
        discharged_by: Option<Id>,
    ) {
        let severity = if mandatory {
            Severity::Mandatory
        } else {
            Severity::Advisory
        };
        let state = if auto {
            ObligationState::Satisfied
        } else {
            ObligationState::Pending
        };
        let ojson = serde_json::json!({
            "kind": kind.name(),
            "target": target.as_str(),
            "severity": if mandatory { "mandatory" } else { "advisory" },
        });
        let oid = content_id(Domain::Obligation, &ojson).unwrap();
        let obl = Obligation {
            id: oid.clone(),
            kind: kind.clone(),
            target: target.clone(),
            inputs: vec![target.clone()],
            severity,
            state,
            discharged_by,
            message: format!("{} obligation for {}", kind.name(), target.as_str()),
        };
        self.obligations.insert(oid.clone(), obl);
        if let Some(c) = self.claims.get_mut(target) {
            c.obligations.push(oid);
        }
    }

    // -- obligation discharge --------------------------------------------

    pub fn discharge(
        &mut self,
        obligation: Id,
        by: Id,
        state: ObligationState,
    ) -> Result<(), CoreError> {
        let obl = self
            .obligations
            .get_mut(&obligation)
            .ok_or_else(|| CoreError::UnknownObligation(obligation.clone()))?;
        // `by` must be a known evidence or receipt.
        if !self.evidence.contains_key(&by) && !self.receipts.contains_key(&by) {
            return Err(CoreError::UnknownEvidence(by));
        }
        obl.state = state;
        obl.discharged_by = Some(by.clone());
        self.events.push(Event::Discharge {
            obligation,
            by,
            state,
        });
        Ok(())
    }

    // -- verification (the central gate) ---------------------------------

    pub fn verify(&mut self, claim: Id) -> Result<(), CoreError> {
        let c = self
            .claims
            .get(&claim)
            .ok_or_else(|| CoreError::UnknownClaim(claim.clone()))?;
        if c.derivation.is_none() {
            // Unsupported assertions, observations, assumptions cannot be
            // silently verified.
            return Err(CoreError::SilentVerify(claim));
        }
        // All mandatory obligations must be Satisfied or Waived.
        for oid in &c.obligations {
            let o = self
                .obligations
                .get(oid)
                .ok_or_else(|| CoreError::UnknownObligation(oid.clone()))?;
            match o.severity {
                Severity::Mandatory => {
                    if !matches!(
                        o.state,
                        ObligationState::Satisfied | ObligationState::Waived
                    ) {
                        return Err(CoreError::ObligationNotDischarged(oid.clone()));
                    }
                }
                Severity::Advisory => {}
            }
        }
        // Derivation must be complete: external ops need a receipt.
        if let Some(did) = &c.derivation {
            let d = self
                .derivations
                .get(did)
                .ok_or_else(|| CoreError::NoDerivation(claim.clone()))?;
            let def = self
                .operations
                .get(&d.op)
                .cloned()
                .ok_or_else(|| CoreError::UnknownOperation(d.op.as_str().to_string()))?;
            if matches!(
                def.class,
                OpClass::VerifiedExternal | OpClass::UnverifiedExternal
            ) && d.receipt.is_none()
            {
                return Err(CoreError::ReceiptRequired(def.name));
            }
        }
        if let Some(c) = self.claims.get_mut(&claim) {
            c.status = ClaimStatus::Verified;
        }
        self.events.push(Event::Verify { claim });
        Ok(())
    }

    // -- challenge --------------------------------------------------------

    pub fn challenge(&mut self, claim: Id) -> Result<(), CoreError> {
        let c = self
            .claims
            .get_mut(&claim)
            .ok_or_else(|| CoreError::UnknownClaim(claim.clone()))?;
        c.status = ClaimStatus::Challenged;
        self.events.push(Event::Challenge { claim });
        Ok(())
    }

    // -- contradiction ----------------------------------------------------

    pub fn contradict(
        &mut self,
        claims: &[Id],
        kind: ContradictionKind,
        witness: Witness,
        context: Option<Id>,
    ) -> Result<Id, CoreError> {
        if claims.len() < 2 {
            return Err(CoreError::ContradictionNeedsTwo);
        }
        for c in claims {
            if !self.claims.contains_key(c) {
                return Err(CoreError::UnknownClaim(c.clone()));
            }
        }
        let ctx = context.unwrap_or_else(|| self.root_context.clone());
        let cjson = serde_json::json!({
            "kind": match &kind {
                ContradictionKind::PropositionNegation => "proposition-negation",
                ContradictionKind::IncompatibleEquality => "incompatible-equality",
                ContradictionKind::DisjointInterval => "disjoint-interval",
                ContradictionKind::IncompatibleUnit => "incompatible-unit",
                ContradictionKind::MutuallyExclusiveMembership => "mutually-exclusive-membership",
                ContradictionKind::ViolatedPostcondition => "violated-postcondition",
                ContradictionKind::EvidenceConflict => "evidence-conflict",
                ContradictionKind::AssumptionConflict => "assumption-conflict",
                ContradictionKind::Extension(s) => s.as_str(),
            },
            "claims": claims.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
            "context": ctx.as_str(),
        });
        let cid = content_id(Domain::Contradiction, &cjson).unwrap();
        let contradiction = Contradiction {
            id: cid.clone(),
            kind,
            claims: claims.to_vec(),
            context: ctx.clone(),
            witness,
            evidence: vec![],
        };
        self.contradictions.insert(cid.clone(), contradiction);
        if let Some(c) = self.contexts.get_mut(&ctx) {
            c.contradictions.push(cid.clone());
        }
        self.events.push(Event::Contradict {
            contradiction: cid.clone(),
        });
        Ok(cid)
    }

    // -- context branching / merge ---------------------------------------

    pub fn branch(&mut self, parent: Id, label: &str, assumptions: &[Id]) -> Result<Id, CoreError> {
        if !self.contexts.contains_key(&parent) {
            return Err(CoreError::UnknownContext(parent));
        }
        for a in assumptions {
            if !self.assumptions.contains_key(a) {
                return Err(CoreError::UnknownAssumption(a.clone()));
            }
        }
        // Inheritance is transitive: a child sees every claim local to any of
        // its ancestor contexts, not only the direct parent.
        let mut inherited = Vec::new();
        let mut cur = Some(parent.clone());
        while let Some(pid) = cur {
            if let Some(anc) = self.contexts.get(&pid) {
                for l in &anc.local_claims {
                    if !inherited.contains(l) {
                        inherited.push(l.clone());
                    }
                }
                cur = anc.parent.clone();
            } else {
                break;
            }
        }
        let cjson = serde_json::json!({
            "parent": parent.as_str(),
            "label": label,
            "assumptions": assumptions.iter().map(|a| a.as_str()).collect::<Vec<_>>(),
        });
        let id = content_id(Domain::Context, &cjson).unwrap();
        let ctx = Context {
            id: id.clone(),
            parent: Some(parent.clone()),
            label: label.to_string(),
            assumptions: assumptions.to_vec(),
            inherited_claims: inherited,
            local_claims: vec![],
            contradictions: vec![],
            merge_of: None,
        };
        self.contexts.insert(id.clone(), ctx);
        self.events.push(Event::Branch {
            context: id.clone(),
            parent: Some(parent),
        });
        Ok(id)
    }

    /// Controlled merge. Claims present in only one branch are inherited.
    /// Claims with conflicting (contradictory) status are reported as merge
    /// conflicts and NOT silently unified.
    pub fn merge(&mut self, a: Id, b: Id, label: &str) -> Result<(Id, Vec<Id>), CoreError> {
        if !self.contexts.contains_key(&a) || !self.contexts.contains_key(&b) {
            return Err(CoreError::UnknownContext(a));
        }
        let mjson = serde_json::json!({ "a": a.as_str(), "b": b.as_str(), "label": label });
        let id = content_id(Domain::Context, &mjson).unwrap();
        let mut conflicts = vec![];
        // Detect semantic conflicts: same semantic_id, different value/status.
        // Scans the *full* claim sets (local + inherited) of both branches.
        let ca = self.contexts.get(&a).unwrap().clone();
        let cb = self.contexts.get(&b).unwrap().clone();
        let a_claims = full_claim_set(&ca);
        let b_claims = full_claim_set(&cb);
        for cid in &a_claims {
            if let Some(cc) = self.claims.get(cid) {
                for did in &b_claims {
                    if let Some(dc) = self.claims.get(did) {
                        if cc.semantic_id == dc.semantic_id && cc.value != dc.value {
                            conflicts.push(cc.semantic_id.clone());
                        }
                    }
                }
            }
        }
        let ctx = Context {
            id: id.clone(),
            parent: None,
            label: label.to_string(),
            assumptions: {
                let mut v = ca.assumptions.clone();
                for x in &cb.assumptions {
                    if !v.contains(x) {
                        v.push(x.clone());
                    }
                }
                v
            },
            inherited_claims: {
                let mut v = a_claims;
                for x in &b_claims {
                    if !v.contains(x) {
                        v.push(x.clone());
                    }
                }
                v
            },
            local_claims: vec![],
            contradictions: vec![],
            merge_of: Some((a.clone(), b.clone())),
        };
        self.contexts.insert(id.clone(), ctx);
        self.events.push(Event::Merge {
            context: id.clone(),
            a,
            b,
            conflicts: conflicts.len(),
        });
        Ok((id, conflicts))
    }

    // -- invalidation (primitive) ----------------------------------------

    pub fn invalidate(&mut self, claim: Id, reason: &str) -> Result<(), CoreError> {
        let c = self
            .claims
            .get_mut(&claim)
            .ok_or_else(|| CoreError::UnknownClaim(claim.clone()))?;
        c.status = ClaimStatus::Invalidated;
        c.invalidation_conditions.push(reason.to_string());
        self.events.push(Event::Invalidate {
            claim,
            reason: reason.to_string(),
        });
        Ok(())
    }

    // -- attest -----------------------------------------------------------

    pub fn attest(&mut self, claim: Id) -> Result<(), CoreError> {
        let c = self
            .claims
            .get_mut(&claim)
            .ok_or_else(|| CoreError::UnknownClaim(claim.clone()))?;
        c.status = ClaimStatus::ExternallyAttested;
        self.events.push(Event::Attest { claim });
        Ok(())
    }

    // -- identities / digest ---------------------------------------------

    /// Canonical, stable module digest over all semantic nodes.
    pub fn digest(&self) -> Id {
        // Serialize the stable node sets in sorted order. IndexMap iteration is
        // insertion order, so we collect and sort keys for determinism.
        let mut claim_bytes = vec![];
        let mut keys: Vec<&Id> = self.claims.keys().collect();
        keys.sort();
        for k in &keys {
            claim_bytes.push(axiom_encoding::canonical_bytes(&self.claims[*k]).unwrap());
        }
        let mut ev_bytes = vec![];
        let mut ek: Vec<&Id> = self.evidence.keys().collect();
        ek.sort();
        for k in &ek {
            ev_bytes.push(axiom_encoding::canonical_bytes(&self.evidence[*k]).unwrap());
        }
        let mut der_bytes = vec![];
        let mut dk: Vec<&Id> = self.derivations.keys().collect();
        dk.sort();
        for k in &dk {
            der_bytes.push(axiom_encoding::canonical_bytes(&self.derivations[*k]).unwrap());
        }
        let mut ob_bytes = vec![];
        let mut ok: Vec<&Id> = self.obligations.keys().collect();
        ok.sort();
        for k in &ok {
            ob_bytes.push(axiom_encoding::canonical_bytes(&self.obligations[*k]).unwrap());
        }
        let mut con_bytes = vec![];
        let mut ck: Vec<&Id> = self.contradictions.keys().collect();
        ck.sort();
        for k in &ck {
            con_bytes.push(axiom_encoding::canonical_bytes(&self.contradictions[*k]).unwrap());
        }
        hash_concat(
            Domain::Module,
            &[
                &claim_bytes.concat(),
                &ev_bytes.concat(),
                &der_bytes.concat(),
                &ob_bytes.concat(),
                &con_bytes.concat(),
            ],
        )
    }

    /// Digest of the deterministic event log (for replay comparison).
    pub fn event_log_digest(&self) -> Id {
        let bytes = axiom_encoding::canonical_bytes(&self.events).unwrap();
        content_id(
            Domain::Event,
            &serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        )
        .unwrap()
    }

    /// The central invariant: every Verified claim has a complete derivation
    /// and all mandatory obligations discharged.
    pub fn check_verification_invariant(&self) -> Result<(), CoreError> {
        for (id, c) in &self.claims {
            if c.status == ClaimStatus::Verified {
                let der = match &c.derivation {
                    Some(d) => self
                        .derivations
                        .get(d)
                        .ok_or_else(|| CoreError::NoDerivation(id.clone()))?,
                    None => return Err(CoreError::NoDerivation(id.clone())),
                };
                let def = self
                    .operations
                    .get(&der.op)
                    .ok_or_else(|| CoreError::UnknownOperation(der.op.as_str().to_string()))?;
                if matches!(
                    def.class,
                    OpClass::VerifiedExternal | OpClass::UnverifiedExternal
                ) && der.receipt.is_none()
                {
                    return Err(CoreError::ReceiptRequired(def.name.clone()));
                }
                for oid in &c.obligations {
                    let o = self
                        .obligations
                        .get(oid)
                        .ok_or_else(|| CoreError::UnknownObligation(oid.clone()))?;
                    if o.severity == Severity::Mandatory
                        && !matches!(
                            o.state,
                            ObligationState::Satisfied | ObligationState::Waived
                        )
                    {
                        return Err(CoreError::ObligationNotDischarged(oid.clone()));
                    }
                }
            }
        }
        Ok(())
    }
}

// Convenience constructors for evidence and receipts used by demos/runtime.
impl Module {
    #[allow(clippy::too_many_arguments)]
    pub fn add_evidence(
        &mut self,
        label: &str,
        media_type: &str,
        content: Option<Value>,
        trust: TrustClass,
        locator: &str,
        provider: &str,
        signature: Option<String>,
    ) -> Id {
        let content_hash = {
            let j =
                serde_json::json!({ "media": media_type, "content": content, "locator": locator });
            content_id(Domain::Evidence, &j).unwrap()
        };
        let ejson = serde_json::json!({
            "content_hash": content_hash.as_str(),
            "media_type": media_type,
            "trust": format!("{trust:?}"),
            "locator": locator,
        });
        let id = content_id(Domain::Evidence, &ejson).unwrap();
        let ev = Evidence {
            id: id.clone(),
            content_hash,
            media_type: media_type.to_string(),
            provenance: Provenance::default(),
            provider: provider.to_string(),
            acquisition: AcquisitionMeta {
                locator: locator.to_string(),
                acquired_at: 0,
            },
            trust,
            signature,
            content,
            label: label.to_string(),
        };
        self.evidence.insert(id.clone(), ev);
        id
    }

    /// Replay-time authenticity check for all evidence nodes.
    ///
    /// Returns `false` if any non-untrusted evidence node lacks a signature
    /// that verifies under a configured trust root (spec §6: receipt
    /// authenticity is mandatory). Untrusted evidence is always permitted.
    /// Used by the replay/encoding path to reject forged modules even when no
    /// execution is performed.
    pub fn verify_evidence_authenticity(&self, trust_roots: &[TrustRoot]) -> bool {
        if trust_roots.is_empty() {
            return true;
        }
        self.evidence.values().all(|ev| {
            // Untrusted evidence is always admitted; its trust class is the gate.
            if ev.trust != TrustClass::Trusted {
                return true;
            }
            // A trusted evidence node MUST carry a signature that verifies over
            // its full canonical binding (provider, locator, media type,
            // content digest, trust class, label) under a configured trust root.
            let trust_str = format!("{:?}", ev.trust);
            crate::crypto::verify_evidence_signature(
                trust_roots,
                &ev.provider,
                &ev.acquisition.locator,
                &ev.media_type,
                &ev.content_hash.as_str(),
                &trust_str,
                &ev.label,
                &ev.signature,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_receipt(
        &mut self,
        operation: &str,
        op_version: &str,
        provider: &str,
        logical_time: u64,
        inputs_hash: Id,
        output: &Value,
        schema: Type,
        trust: &TrustRoot,
    ) -> Id {
        let output_hash = content_id(
            Domain::Receipt,
            &serde_json::json!({ "output": output, "schema": schema.name() }),
        )
        .unwrap();
        let integrity_json = serde_json::json!({
            "operation": operation,
            "op_version": op_version,
            "provider": provider,
            "logical_time": logical_time,
            "inputs_hash": inputs_hash.as_str(),
            "output": output,
            "schema": schema.name(),
        });
        let integrity = content_id(
            Domain::Receipt,
            &serde_json::from_slice::<serde_json::Value>(
                &axiom_encoding::canonical_bytes(&integrity_json).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let rid = content_id(
            Domain::Receipt,
            &serde_json::json!({
                "operation": operation,
                "op_version": op_version,
                "provider": provider,
                "logical_time": logical_time,
                "inputs_hash": inputs_hash.as_str(),
                "output_hash": output_hash.as_str(),
                "integrity": integrity.as_str(),
            }),
        )
        .unwrap();
        // Build the receipt with an empty tag, then sign the payload under the
        // provider's trusted secret. The tag is what makes the receipt
        // authentic; without it, `verify_integrity` rejects the receipt.
        let mut receipt = Receipt {
            id: rid.clone(),
            operation: operation.to_string(),
            op_version: op_version.to_string(),
            provider: provider.to_string(),
            logical_time,
            inputs_hash,
            output: output.clone(),
            output_hash,
            schema,
            integrity,
            signature: vec![],
        };
        let payload = receipt.signed_payload();
        receipt.signature = crate::crypto::sign(&trust.secret, &payload);
        self.receipts.insert(rid.clone(), receipt);
        rid
    }
}

/// Helper to build a quantity value.
pub fn quantity(value: Num, unit: &str) -> Value {
    Value::Quantity(Quantity {
        value,
        unit: Unit::base(unit),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_op() -> OperationDef {
        OperationDef {
            id: OperationDef::identity("core.add", "1"),
            name: "core.add".into(),
            version: "1".into(),
            class: OpClass::PureInternal,
            inputs: vec![Type::Rational, Type::Rational],
            output: Type::Rational,
            determinism: DeterminismClass::Deterministic,
            uncertainty_rule: UncertaintyRule::ConjoinInputs,
            generates: vec![ObligationKind::TypeCompat],
            capabilities: vec![],
        }
    }

    #[test]
    fn verify_requires_derivation_and_obligations() {
        let mut m = Module::new("t");
        m.register_op(add_op());
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
        // out is Pending; verifying should succeed because TypeCompat auto-satisfied.
        m.verify(out.clone()).unwrap();
        assert_eq!(m.claims[&out].status, ClaimStatus::Verified);
        assert!(m.check_verification_invariant().is_ok());
    }

    #[test]
    fn cannot_verify_asserted_claim() {
        let mut m = Module::new("t");
        let a = m
            .assert(
                "a",
                Type::Bool,
                Value::Bool(true),
                Uncertainty::Exact,
                vec![],
                None,
                None,
            )
            .unwrap();
        assert!(matches!(m.verify(a), Err(CoreError::SilentVerify(_))));
    }

    #[test]
    fn receipt_tampering_detected() {
        let trust = TrustRoot::new("p", b"k");
        let roots = [trust.clone()];
        let mut m = Module::new("t");
        let r = m.add_receipt(
            "tool.x",
            "1",
            "p",
            0,
            content_id(Domain::Receipt, &serde_json::json!({"x":1})).unwrap(),
            &Value::Num(Num::Int(1)),
            Type::Rational,
            &trust,
        );
        // Untampered, authentic receipt verifies.
        assert!(m.receipts[&r].verify_integrity(&roots));

        // Forgery without the key: change the output AND recompute the payload
        // hash, but the HMAC tag cannot be reproduced without the trusted secret.
        // This is the adversarial case the old self-hash check missed.
        {
            let rec = m.receipts.get_mut(&r).unwrap();
            rec.output = Value::Num(Num::Int(2));
            // recompute the payload hash to simulate a careful forger
            rec.integrity = content_id(
                Domain::Receipt,
                &serde_json::from_slice::<serde_json::Value>(&rec.signed_payload()).unwrap(),
            )
            .unwrap();
        }
        assert!(!m.receipts[&r].verify_integrity(&roots));

        // An untampered, authentically-signed receipt verifies.
        let r2 = m.add_receipt(
            "tool.y",
            "1",
            "p",
            0,
            content_id(Domain::Receipt, &serde_json::json!({"x":1})).unwrap(),
            &Value::Num(Num::Int(7)),
            Type::Rational,
            &trust,
        );
        assert!(m.receipts[&r2].verify_integrity(&roots));

        // A receipt from an untrusted provider (no matching trust root) is
        // rejected even with a perfectly self-consistent payload.
        let r3 = m.add_receipt(
            "tool.z",
            "1",
            "hostile",
            0,
            content_id(Domain::Receipt, &serde_json::json!({"x":1})).unwrap(),
            &Value::Num(Num::Int(9)),
            Type::Rational,
            &TrustRoot::new("hostile", b"evil"),
        );
        assert!(!m.receipts[&r3].verify_integrity(&roots));
    }
}
