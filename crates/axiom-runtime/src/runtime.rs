//! The Axiom runtime: executes a parsed module against the trusted core,
//! manages external receipts and capabilities, supports deterministic offline
//! replay, and detects contradictions.

use crate::convert::{contradiction_kind, convert_expr, convert_type, convert_uncertainty};
use crate::error::RuntimeError;
use crate::external::{ExternalExecutor, ReplayOnlyExecutor, ToolCalculator};
use axiom_core::registry::{builtin_operations, BuiltinExecutor};
use axiom_core::{
    Capability, ClaimStatus, DeterminismClass, Id, Module, Obligation, ObligationKind,
    ObligationState, OpClass, OperationDef, Severity, TrustClass, TrustRoot, Type, Uncertainty,
    UncertaintyRule, Value, Witness,
};
use axiom_encoding::{content_id, Domain};
use axiom_parser::ast::{Stmt, TypeExpr};
use std::collections::{BTreeSet, HashMap};

pub struct Runtime {
    pub module: Module,
    claim_labels: HashMap<String, Id>,
    context_labels: HashMap<String, Id>,
    evidence_labels: HashMap<String, Id>,
    receipt_labels: HashMap<String, Id>,
    caps: BTreeSet<String>,
    external: Box<dyn ExternalExecutor>,
    replay: bool,
    preloaded: Vec<axiom_core::Receipt>,
    /// Host-configured verifying keys for receipt authenticity. A receipt is
    /// accepted as authentic only if a trust root for its provider is present
    /// and its HMAC tag verifies under that root's secret.
    trust_roots: Vec<TrustRoot>,
    /// When true, `execute` re-detects contradictions in every context after
    /// execution (spec §5 step 6). Off by default; enable via
    /// [`Runtime::auto_contradictions`].
    auto_contradictions: bool,
    /// Non-fatal verification failures recorded during execution. A claim that
    /// cannot be verified is left unverified (quarantined); this is a legitimate
    /// state, not a fatal error, so valid portions of the module are preserved.
    pub verify_failures: Vec<String>,
}

/// Hard cap on the number of statements a single module may declare. A hostile
/// module can declare arbitrarily many statements; beyond this bound execution
/// fails closed rather than exhausting memory/CPU. Works together with the
/// lexer's token cap to bound resource use.
const MAX_MODULE_STMTS: usize = 100_000;

impl Default for Runtime {
    fn default() -> Self {
        Runtime::new("module")
    }
}

impl Runtime {
    pub fn new(name: &str) -> Runtime {
        let mut m = Module::new(name);
        for op in builtin_operations() {
            m.register_op(op);
        }
        let root = m.root_context.clone();
        let mut ctx_labels = HashMap::new();
        ctx_labels.insert("root".to_string(), root);
        Runtime {
            module: m,
            claim_labels: HashMap::new(),
            context_labels: ctx_labels,
            evidence_labels: HashMap::new(),
            receipt_labels: HashMap::new(),
            caps: BTreeSet::new(),
            external: Box::new(ReplayOnlyExecutor),
            replay: false,
            preloaded: vec![],
            trust_roots: vec![TrustRoot::demo()],
            auto_contradictions: false,
            verify_failures: vec![],
        }
    }

    /// Resolve an operation name from source to a registered canonical name.
    /// An unqualified builtin (e.g. `add`) resolves to its `core.` namespaced
    /// definition (`core.add`); explicit names and extension ops are used as-is.
    /// Returns `None` if no registered operation matches.
    pub fn resolve_op(&self, op: &str) -> Option<String> {
        let exact = OperationDef::identity(op, "1");
        if self.module.operations.contains_key(&exact) {
            return Some(op.to_string());
        }
        let core = format!("core.{op}");
        let core_id = OperationDef::identity(&core, "1");
        if self.module.operations.contains_key(&core_id) {
            return Some(core);
        }
        None
    }

    /// Grant a capability token (e.g. "tool:calculator").
    pub fn grant(&mut self, cap: &str) -> &mut Self {
        self.caps.insert(cap.to_string());
        self
    }

    /// Install a live external executor (defaults to replay-only).
    pub fn with_external(&mut self, ext: Box<dyn ExternalExecutor>) -> &mut Self {
        self.external = ext;
        self
    }

    /// Enable replay mode: external calls use preloaded receipts only.
    pub fn replay_mode(&mut self, receipts: Vec<axiom_core::Receipt>) -> &mut Self {
        self.replay = true;
        self.preloaded = receipts.clone();
        for r in receipts {
            self.module.receipts.insert(r.id.clone(), r);
        }
        self
    }

    pub fn with_builtin_tool(&mut self) -> &mut Self {
        self.external = Box::new(ToolCalculator);
        self
    }

    /// Configure the verifying keys used to authenticate receipts during
    /// execution and replay. Hosts MUST supply the real per-provider secrets;
    /// the reference runtime installs only the fixture calculator's demo root.
    pub fn with_trust_roots(&mut self, roots: Vec<TrustRoot>) -> &mut Self {
        self.trust_roots = roots;
        self
    }

    /// Append a single trust root.
    pub fn add_trust_root(&mut self, root: TrustRoot) -> &mut Self {
        self.trust_roots.push(root);
        self
    }

    /// Enable automatic contradiction detection across all contexts after
    /// execution (spec §5 step 6). When disabled, contradiction detection runs
    /// only on explicit `contradict` statements or direct `detect_contradictions`
    /// calls.
    pub fn enable_auto_contradictions(&mut self) -> &mut Self {
        self.auto_contradictions = true;
        self
    }

    // -- resolvers -------------------------------------------------------

    /// Resolve a claim label to its content-addressed id. Public so tooling
    /// (CLI, SDK, conformance) can address nodes by their source symbol.
    pub fn claim(&self, label: &str) -> Result<Id, RuntimeError> {
        self.claim_labels
            .get(label)
            .cloned()
            .ok_or_else(|| RuntimeError::UnknownLabel(label.to_string()))
    }

    /// All claim labels declared so far, useful for inspection tooling.
    pub fn claim_labels(&self) -> Vec<String> {
        self.claim_labels.keys().cloned().collect()
    }

    pub fn evidence(&self, label: &str) -> Result<Id, RuntimeError> {
        self.evidence_labels
            .get(label)
            .cloned()
            .ok_or_else(|| RuntimeError::UnknownEvidenceLabel(label.to_string()))
    }

    pub fn context(&self, label: &str) -> Result<Id, RuntimeError> {
        self.context_labels
            .get(label)
            .cloned()
            .ok_or_else(|| RuntimeError::UnknownContextLabel(label.to_string()))
    }

    fn trust_of(s: &str) -> axiom_core::TrustClass {
        match s {
            "trusted" => axiom_core::TrustClass::Trusted,
            "untrusted" => axiom_core::TrustClass::Untrusted,
            _ => axiom_core::TrustClass::Unverified,
        }
    }

    // -- execution -------------------------------------------------------

    pub fn execute(&mut self, ast: &axiom_parser::ast::ModuleAst) -> Result<(), RuntimeError> {
        self.module.name = ast.name.clone();
        self.module.format_version = ast.version.clone();

        // Resource bound: reject modules whose statement count exceeds the cap.
        // This closes the "no default node/claim-count cap" gap; the lexer's
        // token cap bounds input size during parsing.
        if ast.stmts.len() > MAX_MODULE_STMTS {
            return Err(RuntimeError::ModuleTooLarge(format!(
                "{} > {}",
                ast.stmts.len(),
                MAX_MODULE_STMTS
            )));
        }

        // Phase 1: dependency-free declarations execute immediately. Evidence has
        // no dependencies, so it is bound first; assertions/observations/assumptions
        // may reference evidence and therefore execute afterwards. This keeps module
        // semantics order-independent while honouring the evidence->claim dependency.
        let mut pending: Vec<&axiom_parser::ast::Stmt> = vec![];
        for stmt in &ast.stmts {
            if matches!(stmt, axiom_parser::ast::Stmt::Evidence { .. }) {
                self.exec_stmt(stmt)?;
            }
        }
        for stmt in &ast.stmts {
            match stmt {
                axiom_parser::ast::Stmt::Module { .. } => {}
                axiom_parser::ast::Stmt::Evidence { .. } => {}
                axiom_parser::ast::Stmt::Assert { .. }
                | axiom_parser::ast::Stmt::Observe { .. }
                | axiom_parser::ast::Stmt::Assume { .. } => {
                    self.exec_stmt(stmt)?;
                }
                _ => pending.push(stmt),
            }
        }

        // Phase 2: dependency-bearing statements execute in dependency order via a
        // worklist, so forward references are legal and declaration order is
        // semantically irrelevant. A statement becomes ready when every label it
        // references has been bound. If no remaining statement can make progress,
        // the module contains a cycle or a genuinely missing label.
        let mut remaining: Vec<&axiom_parser::ast::Stmt> = pending;
        while !remaining.is_empty() {
            let mut progressed = false;
            let mut idx = 0;
            while idx < remaining.len() {
                if self.stmt_ready(remaining[idx]) {
                    let s = remaining.remove(idx);
                    self.exec_stmt(s)?;
                    progressed = true;
                    break;
                }
                idx += 1;
            }
            if !progressed {
                let bad = remaining.into_iter().next().unwrap();
                return Err(self.unready_error(bad));
            }
        }

        self.module
            .check_verification_invariant()
            .map_err(|e| RuntimeError::Invariant(e.to_string()))?;

        // Authoritative trusted-evidence authentication on the module-loading
        // path (shared with execution via `verify_evidence`). A trusted evidence
        // node whose provider/signature/trust-root does not verify is rejected
        // here, so forged modules are caught even before any derivation runs.
        if !self.module.verify_evidence_authenticity(&self.trust_roots) {
            return Err(RuntimeError::EvidenceForgery(
                "module contains trusted evidence that fails authentication".into(),
            ));
        }

        if self.auto_contradictions {
            let ctxs: Vec<Id> = self.module.contexts.keys().cloned().collect();
            for ctx in ctxs {
                if let Some(label) = self.module.contexts.get(&ctx).map(|c| c.label.clone()) {
                    let _ = self.detect_contradictions(&label);
                }
            }
        }
        Ok(())
    }

    /// Whether all labels referenced by `stmt` are already bound, so it can execute.
    fn stmt_ready(&self, stmt: &axiom_parser::ast::Stmt) -> bool {
        use axiom_parser::ast::Stmt::*;
        match stmt {
            Derive {
                inputs, receipt, ..
            } => {
                inputs.iter().all(|l| self.claim_labels.contains_key(l))
                    && receipt
                        .as_ref()
                        .is_none_or(|r| self.receipt_labels.contains_key(r))
            }
            Require { target, .. } => self.claim_labels.contains_key(target),
            Discharge { obligation, by, .. } => {
                self.claim_labels.contains_key(obligation)
                    && (self.evidence_labels.contains_key(by)
                        || self.receipt_labels.contains_key(by)
                        || self.claim_labels.contains_key(by))
            }
            Verify { target, .. }
            | Challenge { target, .. }
            | Invalidate { target, .. }
            | Attest { target, .. } => self.claim_labels.contains_key(target),
            Contradict { a, b, .. } => {
                self.claim_labels.contains_key(a) && self.claim_labels.contains_key(b)
            }
            Branch { parent, .. } => self.context_labels.contains_key(parent),
            Merge { a, b, .. } => {
                self.context_labels.contains_key(a) && self.context_labels.contains_key(b)
            }
            Call { inputs, .. } => inputs.iter().all(|l| self.claim_labels.contains_key(l)),
            _ => true,
        }
    }

    fn unready_error(&self, stmt: &axiom_parser::ast::Stmt) -> RuntimeError {
        use axiom_parser::ast::Stmt::*;
        match stmt {
            Derive { label, inputs, .. } => {
                let missing = inputs
                    .iter()
                    .find(|l| !self.claim_labels.contains_key(*l))
                    .cloned();
                RuntimeError::UnknownLabel(missing.unwrap_or_else(|| label.clone()))
            }
            Require { target, .. } => RuntimeError::UnknownLabel(target.clone()),
            Verify { target, .. }
            | Challenge { target, .. }
            | Invalidate { target, .. }
            | Attest { target, .. } => RuntimeError::UnknownLabel(target.clone()),
            Contradict { a, b, .. } => {
                if !self.claim_labels.contains_key(a) {
                    RuntimeError::UnknownLabel(a.clone())
                } else {
                    RuntimeError::UnknownLabel(b.clone())
                }
            }
            Branch { parent, .. } => RuntimeError::UnknownContextLabel(parent.clone()),
            Merge { a, .. } => RuntimeError::UnknownContextLabel(a.clone()),
            Call { label, inputs, .. } => {
                let missing = inputs
                    .iter()
                    .find(|l| !self.claim_labels.contains_key(*l))
                    .cloned();
                RuntimeError::UnknownLabel(missing.unwrap_or_else(|| label.clone()))
            }
            _ => RuntimeError::UnknownLabel("<unresolved>".into()),
        }
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        match stmt {
            Stmt::Module { .. } => Ok(()),
            Stmt::Evidence {
                label,
                media,
                content,
                trust,
                provider,
                signature,
                ..
            } => {
                let v = content.as_ref().map(|c| Value::Str(c.clone()));
                let trust_class = Self::trust_of(trust);
                let provider = provider.clone().unwrap_or_default();
                let id = self.module.add_evidence(
                    label,
                    media,
                    v,
                    trust_class,
                    media,
                    &provider,
                    signature.clone(),
                );
                self.evidence_labels.insert(label.clone(), id.clone());
                // Enforced authenticity (spec §6): when evidence claims
                // `trusted`, the host must supply a provider, a signature, and a
                // matching configured TrustRoot, and the signature MUST verify
                // over the exact canonical evidence binding. Untrusted/unverified
                // evidence is admitted without a signature.
                if trust_class == TrustClass::Trusted {
                    let ev = &self.module.evidence[&id];
                    let result = axiom_core::verify_evidence(
                        &self.trust_roots,
                        &ev.provider,
                        &ev.acquisition.locator,
                        &ev.media_type,
                        &ev.content_hash.as_str(),
                        ev.trust,
                        &ev.label,
                        &ev.signature,
                    );
                    match result {
                        Err(axiom_core::EvidenceAuthError::NoProvider) => {
                            return Err(RuntimeError::EvidenceForgery(format!(
                                "{label}: trusted evidence requires a provider"
                            )));
                        }
                        Err(axiom_core::EvidenceAuthError::NoSignature) => {
                            return Err(RuntimeError::EvidenceForgery(format!(
                                "{label}: trusted evidence requires a signature"
                            )));
                        }
                        Err(axiom_core::EvidenceAuthError::NoTrustRoot) => {
                            return Err(RuntimeError::EvidenceForgery(format!(
                                "{label}: no trust root for provider '{}'",
                                ev.provider
                            )));
                        }
                        Err(axiom_core::EvidenceAuthError::BadSignature) => {
                            return Err(RuntimeError::EvidenceForgery(format!(
                                "{label}: evidence signature does not verify"
                            )));
                        }
                        Ok(()) => {}
                    }
                }
                Ok(())
            }
            Stmt::Assert {
                label,
                value,
                ty,
                evidence,
                uncertainty,
                ctx,
                ..
            } => self.exec_assert(label, value, ty, evidence, uncertainty, ctx, false),
            Stmt::Observe {
                label,
                value,
                ty,
                evidence,
                uncertainty,
                ..
            } => self.exec_assert(label, value, ty, evidence, uncertainty, &None, true),
            Stmt::Assume {
                label,
                value,
                ty,
                scope,
                ctx,
                ..
            } => {
                let t = convert_type(ty)?;
                let v = convert_expr(value)?;
                let cid = match ctx {
                    Some(c) => self.context(c)?,
                    None => self.module.root_context.clone(),
                };
                let (_aid, claim_id) = self.module.assume(label, t, v, scope, Some(cid))?;
                self.claim_labels.insert(label.clone(), claim_id);
                Ok(())
            }
            Stmt::Derive {
                label,
                op,
                inputs,
                ty,
                receipt,
                ctx,
                ..
            } => {
                let input_ids: Vec<Id> = inputs
                    .iter()
                    .map(|i| self.claim(i))
                    .collect::<Result<_, _>>()?;
                convert_type(ty)?; // validate the declared type exists
                let op_name = self
                    .resolve_op(op)
                    .ok_or_else(|| RuntimeError::UnknownOperation(op.clone()))?;
                let ctx_id = match ctx {
                    Some(c) => Some(self.context(c)?),
                    None => None,
                };
                let rid = match receipt {
                    Some(rl) => Some(
                        self.receipt_labels
                            .get(rl)
                            .cloned()
                            .ok_or_else(|| RuntimeError::UnknownReceiptLabel(rl.clone()))?,
                    ),
                    None => None,
                };
                let out = self.module.derive(
                    &op_name,
                    "1",
                    &input_ids,
                    ctx_id,
                    label,
                    rid,
                    &BuiltinExecutor,
                    &self.trust_roots,
                )?;
                self.claim_labels.insert(label.to_string(), out);
                Ok(())
            }
            Stmt::Require { kind, target, .. } => {
                let cid = self.claim(target)?;
                let okind = if kind == "custom" {
                    ObligationKind::Custom(target.clone())
                } else {
                    // Map a few well-known kinds; default to Custom.
                    match kind.as_str() {
                        "type-compat" => ObligationKind::TypeCompat,
                        "dimensional-consistency" => ObligationKind::DimensionalConsistency,
                        "numeric-bounds" => ObligationKind::NumericBounds,
                        "source-support" => ObligationKind::SourceSupport,
                        "premise-available" => ObligationKind::PremiseAvailable,
                        _ => ObligationKind::Custom(kind.clone()),
                    }
                };
                let ojson = serde_json::json!({ "kind": kind, "target": cid.as_str(), "severity": "mandatory" });
                let oid = content_id(Domain::Obligation, &ojson).unwrap();
                let obl = Obligation {
                    id: oid.clone(),
                    kind: okind,
                    target: cid.clone(),
                    inputs: vec![cid.clone()],
                    severity: Severity::Mandatory,
                    state: ObligationState::Pending,
                    discharged_by: None,
                    message: format!("required obligation {kind} on {cid}"),
                };
                self.module.obligations.insert(oid.clone(), obl);
                if let Some(c) = self.module.claims.get_mut(&cid) {
                    c.obligations.push(oid);
                }
                Ok(())
            }
            Stmt::Discharge { obligation, by, .. } => {
                // Discharge every pending obligation of the targeted claim, each
                // through `Module::discharge`, which enforces `by ∈ E ∪ R` and
                // records a `Discharge` event. A claim id is no longer accepted
                // as the discharging authority (spec §6.6).
                let cid = self.claim(obligation)?;
                let by_id = self.resolve_discharge_by(by)?;
                let pending: Vec<Id> = self
                    .module
                    .claims
                    .get(&cid)
                    .map(|c| {
                        c.obligations
                            .iter()
                            .filter(|oid| {
                                self.module
                                    .obligations
                                    .get(*oid)
                                    .map(|o| o.state == ObligationState::Pending)
                                    .unwrap_or(false)
                            })
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                for oid in pending {
                    self.module
                        .discharge(oid, by_id.clone(), ObligationState::Satisfied)?;
                }
                Ok(())
            }
            Stmt::Verify { target, .. } => {
                let cid = self.claim(target)?;
                // Verification failure is a legitimate state, not a fatal error:
                // the claim simply stays unverified (quarantined) and the rest of
                // the module continues to execute. This preserves valid reasoning
                // while never silently verifying an unsupported claim.
                if let Err(e) = self.module.verify(cid) {
                    self.verify_failures
                        .push(format!("verify {} failed: {}", target, e));
                }
                Ok(())
            }
            Stmt::Challenge { target, .. } => {
                let cid = self.claim(target)?;
                self.module.challenge(cid)?;
                Ok(())
            }
            Stmt::Contradict { a, b, kind, .. } => {
                let ca = self.claim(a)?;
                let cb = self.claim(b)?;
                let va = self.module.claims[&ca].value.clone();
                let vb = self.module.claims[&cb].value.clone();
                let detail = Value::Record(vec![("a".into(), va), ("b".into(), vb)]);
                let witness = Witness {
                    summary: format!("contradiction {} between {} and {}", kind, a, b),
                    detail,
                };
                self.module
                    .contradict(&[ca, cb], contradiction_kind(kind), witness, None)?;
                Ok(())
            }
            Stmt::Branch {
                label,
                parent,
                assumptions,
                ..
            } => {
                let pid = self.context(parent)?;
                let mut aids = vec![];
                for al in assumptions {
                    let cid = self.claim(al)?;
                    if let Some(c) = self.module.claims.get(&cid) {
                        aids.extend(c.assumptions.iter().cloned());
                    }
                }
                let id = self.module.branch(pid, label, &aids)?;
                self.context_labels.insert(label.clone(), id);
                Ok(())
            }
            Stmt::Merge { label, a, b, .. } => {
                let aid = self.context(a)?;
                let bid = self.context(b)?;
                let (id, _conflicts) = self.module.merge(aid, bid, label)?;
                self.context_labels.insert(label.clone(), id);
                Ok(())
            }
            Stmt::Invalidate { target, reason, .. } => {
                let cid = self.claim(target)?;
                self.module.invalidate(cid, reason)?;
                Ok(())
            }
            Stmt::Attest { target, .. } => {
                let cid = self.claim(target)?;
                self.module.attest(cid)?;
                Ok(())
            }
            Stmt::Call {
                label,
                op,
                inputs,
                ty,
                capability,
                ctx,
                ..
            } => self.exec_call(label, op, inputs, ty, capability, ctx),
        }
    }

    fn resolve_discharge_by(&self, by: &str) -> Result<Id, RuntimeError> {
        // `by` must reference evidence or a receipt: the discharging authority
        // is a piece of evidence or an authentic receipt, never another claim
        // (spec §6.6: discharged_by ∈ E ∪ R).
        if let Some(e) = self.evidence_labels.get(by) {
            return Ok(e.clone());
        }
        if let Some(r) = self.receipt_labels.get(by) {
            return Ok(r.clone());
        }
        Err(RuntimeError::UnknownLabel(by.to_string()))
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_assert(
        &mut self,
        label: &str,
        value: &axiom_parser::ast::Expr,
        ty: &TypeExpr,
        evidence: &[String],
        uncertainty: &Option<axiom_parser::ast::UncertaintyExpr>,
        ctx: &Option<String>,
        observe: bool,
    ) -> Result<(), RuntimeError> {
        let t = convert_type(ty)?;
        let v = convert_expr(value)?;
        let u = match uncertainty {
            Some(u) => convert_uncertainty(u)?,
            None => Uncertainty::Unknown,
        };
        let ev_ids: Vec<Id> = evidence
            .iter()
            .map(|e| self.evidence(e))
            .collect::<Result<_, _>>()?;
        let cid = match ctx {
            Some(c) => Some(self.context(c)?),
            None => None,
        };
        let id = if observe {
            self.module.observe(label, t, v, u, ev_ids, cid)?
        } else {
            self.module.assert(label, t, v, u, ev_ids, cid, None)?
        };
        self.claim_labels.insert(label.to_string(), id);
        Ok(())
    }

    fn exec_call(
        &mut self,
        label: &str,
        op: &str,
        inputs: &[String],
        ty: &TypeExpr,
        capability: &str,
        ctx: &Option<String>,
    ) -> Result<(), RuntimeError> {
        let input_ids: Vec<Id> = inputs
            .iter()
            .map(|i| self.claim(i))
            .collect::<Result<_, _>>()?;
        let input_values: Vec<Value> = input_ids
            .iter()
            .map(|i| self.module.claims[i].value.clone())
            .collect();
        let input_types: Vec<Type> = input_ids
            .iter()
            .map(|i| self.module.claims[i].ty.clone())
            .collect();
        let out_ty = convert_type(ty)?;
        let inputs_hash = content_id(Domain::Receipt, &serde_json::json!(input_values)).unwrap();
        let ctx_id = match ctx {
            Some(c) => Some(self.context(c)?),
            None => None,
        };

        let rid = if self.replay {
            let r = self
                .preloaded
                .iter()
                .find(|r| {
                    r.operation == op
                        && r.inputs_hash == inputs_hash
                        && r.verify_integrity(&self.trust_roots)
                })
                .ok_or_else(|| RuntimeError::ReplayMissingReceipt(op.to_string()))?;
            r.id.clone()
        } else {
            if !self.caps.contains(capability) {
                return Err(RuntimeError::CapabilityDenied(capability.to_string()));
            }
            let res = self.external.run(op, &input_values, &self.caps)?;
            // The runtime signs the captured receipt under the trusted secret
            // for the provider. Without a configured trust root the receipt
            // cannot be made authentic, so live execution fails closed.
            let trust = self
                .trust_roots
                .iter()
                .find(|t| t.provider == res.provider)
                .ok_or_else(|| RuntimeError::UnknownTrustRoot(res.provider.clone()))?;
            self.module.add_receipt(
                op,
                "1",
                &res.provider,
                res.logical_time,
                inputs_hash.clone(),
                &res.value,
                out_ty.clone(),
                trust,
            )
        };

        // Ensure the external operation definition is registered.
        let def = OperationDef {
            id: OperationDef::identity(op, "1"),
            name: op.to_string(),
            version: "1".into(),
            class: OpClass::VerifiedExternal,
            inputs: input_types,
            output: out_ty,
            determinism: DeterminismClass::Receipted,
            uncertainty_rule: UncertaintyRule::Exact,
            generates: vec![ObligationKind::ToolReceiptValidation],
            capabilities: vec![Capability::Tool(
                capability.trim_start_matches("tool:").to_string(),
            )],
        };
        self.module.register_op(def);

        let out = self.module.derive(
            op,
            "1",
            &input_ids,
            ctx_id,
            label,
            Some(rid),
            &BuiltinExecutor,
            &self.trust_roots,
        )?;
        self.claim_labels.insert(label.to_string(), out);
        Ok(())
    }

    // -- contradiction detection -----------------------------------------

    /// Scan a context for pairwise contradictions and record first-class
    /// contradiction nodes. Returns the number of contradictions detected.
    ///
    /// Scans the context's *full* claim set (its own `local_claims` plus the
    /// `inherited_claims` it received from ancestors) so inherited claims
    /// participate in contradiction detection.
    pub fn detect_contradictions(&mut self, ctx_label: &str) -> Result<usize, RuntimeError> {
        let ctx = self.context(ctx_label)?;
        let mut claim_ids: Vec<Id> = self.module.contexts[&ctx].local_claims.clone();
        for i in &self.module.contexts[&ctx].inherited_claims {
            if !claim_ids.contains(i) {
                claim_ids.push(i.clone());
            }
        }
        let mut detected = 0;
        for i in 0..claim_ids.len() {
            for j in (i + 1)..claim_ids.len() {
                let a = claim_ids[i].clone();
                let b = claim_ids[j].clone();
                if let Some((kind, summary)) = self.classify_contradiction(&a, &b) {
                    let va = self.module.claims[&a].value.clone();
                    let vb = self.module.claims[&b].value.clone();
                    let detail = Value::Record(vec![("a".into(), va), ("b".into(), vb)]);
                    let witness = Witness { summary, detail };
                    self.module
                        .contradict(&[a, b], kind, witness, Some(ctx.clone()))?;
                    detected += 1;
                }
            }
        }
        Ok(detected)
    }

    /// Classify a pairwise contradiction. Implements the automatically
    /// detectable kinds: proposition negation, incompatible equality (same
    /// label, different value), disjoint intervals, incompatible units (same
    /// magnitude, different unit), violated postcondition (a failed obligation
    /// targets either claim), and assumption conflict (two assumptions with the
    /// same scope but different values). The remaining kinds
    /// (mutually-exclusive membership, evidence conflict) are raised explicitly
    /// via `contradict` statements.
    fn classify_contradiction(
        &self,
        a: &Id,
        b: &Id,
    ) -> Option<(axiom_core::ContradictionKind, String)> {
        let ca = &self.module.claims[a];
        let cb = &self.module.claims[b];
        use axiom_core::ContradictionKind;
        use axiom_types::Value;

        // Violated postcondition: either claim carries a Failed obligation.
        for c in [ca, cb] {
            for oid in &c.obligations {
                if let Some(o) = self.module.obligations.get(oid) {
                    if o.state == ObligationState::Failed {
                        return Some((
                            ContradictionKind::ViolatedPostcondition,
                            format!("obligation {} failed on {}", o.id, c.label),
                        ));
                    }
                }
            }
        }

        // Assumption conflict: two assumptions sharing a scope but with
        // different values.
        let scopes_a: Vec<(String, Value)> = ca
            .assumptions
            .iter()
            .filter_map(|aid| self.module.assumptions.get(aid))
            .map(|asum| {
                let v = self
                    .module
                    .claims
                    .get(&asum.claim)
                    .map(|c| c.value.clone())
                    .unwrap_or(Value::Bool(false));
                (asum.scope.clone(), v)
            })
            .collect();
        let scopes_b: Vec<(String, Value)> = cb
            .assumptions
            .iter()
            .filter_map(|aid| self.module.assumptions.get(aid))
            .map(|asum| {
                let v = self
                    .module
                    .claims
                    .get(&asum.claim)
                    .map(|c| c.value.clone())
                    .unwrap_or(Value::Bool(false));
                (asum.scope.clone(), v)
            })
            .collect();
        for (sa, va) in &scopes_a {
            for (sb, vb) in &scopes_b {
                if sa == sb && va != vb {
                    return Some((
                        ContradictionKind::AssumptionConflict,
                        format!("assumptions with scope {sa} disagree: {va:?} vs {vb:?}"),
                    ));
                }
            }
        }

        // Incompatible equality: same label, different value.
        if ca.label == cb.label && ca.value != cb.value {
            return Some((
                ContradictionKind::IncompatibleEquality,
                format!("claim {} asserted with two values", ca.label),
            ));
        }

        match (&ca.value, &cb.value) {
            (Value::Bool(x), Value::Bool(y)) if x != y => Some((
                ContradictionKind::PropositionNegation,
                format!("bool conflict {} vs {}", x, y),
            )),
            (Value::Interval { lo: lo1, hi: hi1 }, Value::Interval { lo: lo2, hi: hi2 }) => {
                if lo1.cmp_num(hi2) == std::cmp::Ordering::Greater
                    || lo2.cmp_num(hi1) == std::cmp::Ordering::Greater
                {
                    Some((
                        ContradictionKind::DisjointInterval,
                        "disjoint numeric intervals".into(),
                    ))
                } else {
                    None
                }
            }
            (Value::Quantity(q1), Value::Quantity(q2)) => {
                if q1.value == q2.value && q1.unit != q2.unit {
                    Some((
                        ContradictionKind::IncompatibleUnit,
                        "same magnitude, incompatible units".into(),
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    // -- identity / digest ----------------------------------------------

    pub fn digest(&self) -> Id {
        self.module.digest()
    }

    pub fn event_log_digest(&self) -> Id {
        self.module.event_log_digest()
    }

    pub fn verified_claims(&self) -> Vec<Id> {
        self.module
            .claims
            .iter()
            .filter(|(_, c)| c.status == ClaimStatus::Verified)
            .map(|(id, _)| id.clone())
            .collect()
    }
}
