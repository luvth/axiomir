//! Dependency-aware incremental invalidation for Axiom IR.
//!
//! When any premise, assumption, evidence, receipt, or operation version
//! changes, the engine identifies the *exact* set of claims that depend on it
//! (the invalidation frontier), invalidates them, and recomputes only the
//! derived conclusions whose inputs are available. Unrelated verified work is
//! preserved untouched.

use axiom_core::registry::OpExecutor;
use axiom_core::{ClaimStatus, Event, Id, Module, TrustRoot};
use axiom_runtime::Runtime;
use std::collections::{HashMap, HashSet, VecDeque};

/// One recorded status transition, the machine-readable explanation of a change.
#[derive(Debug, Clone)]
pub struct StatusChange {
    pub claim: Id,
    pub label: String,
    pub from: ClaimStatus,
    pub to: ClaimStatus,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct InvalidationReport {
    /// The node(s) whose change triggered invalidation.
    pub roots: Vec<Id>,
    /// Claims marked Invalidated and not recomputed.
    pub invalidated: Vec<Id>,
    /// Claims successfully recomputed (and re-verified where possible).
    pub recomputed: Vec<Id>,
    /// Verified claims left untouched by the change.
    pub preserved: Vec<Id>,
    /// Every status transition, in order.
    pub changes: Vec<StatusChange>,
}

/// The dependency graph of a module: forward edges (who a claim depends on)
/// and reverse edges (who depends on a claim).
pub struct DependencyGraph {
    pub forward: HashMap<Id, Vec<Id>>,
    pub reverse: HashMap<Id, Vec<Id>>,
}

impl DependencyGraph {
    pub fn build(m: &Module) -> DependencyGraph {
        let mut forward = HashMap::new();
        let mut reverse: HashMap<Id, Vec<Id>> = HashMap::new();
        for (cid, c) in &m.claims {
            let deps = c.depends_on(m);
            forward.insert(cid.clone(), deps.clone());
            for d in deps {
                reverse.entry(d).or_default().push(cid.clone());
            }
        }
        DependencyGraph { forward, reverse }
    }

    /// Direct claims that depend on `node`.
    pub fn dependents(&self, node: &Id) -> Vec<Id> {
        self.reverse.get(node).cloned().unwrap_or_default()
    }

    /// Transitive dependents (the full invalidation frontier).
    pub fn transitive_dependents(&self, node: &Id) -> HashSet<Id> {
        let mut out = HashSet::new();
        let mut q = VecDeque::new();
        for d in self.dependents(node) {
            q.push_back(d);
        }
        while let Some(n) = q.pop_front() {
            if out.contains(&n) {
                continue;
            }
            out.insert(n.clone());
            for d in self.dependents(&n) {
                if !out.contains(&d) {
                    q.push_back(d);
                }
            }
        }
        out
    }
}

/// Compute the reverse-dependency map (node -> claims that depend on it).
pub fn reverse_dependents(m: &Module) -> HashMap<Id, Vec<Id>> {
    DependencyGraph::build(m).reverse
}

/// The core operation: invalidate everything that depends on `changed`, then
/// recompute eligible derivations. `changed` may be a claim, evidence,
/// assumption, or receipt id; claims referencing it are seeded into the
/// frontier. `trust_roots` are required so that any receipted derivation
/// re-resolved during recomputation is re-authenticated.
pub fn invalidate_and_recompute(
    m: &mut Module,
    changed: &Id,
    executor: &dyn OpExecutor,
    trust_roots: &[TrustRoot],
) -> InvalidationReport {
    let rev = reverse_dependents(m);

    // BFS the affected claim set from `changed`. If `changed` is an assumption,
    // its introduced claim is also invalidated (retracting the assumption).
    let mut affected: HashSet<Id> = HashSet::new();
    if let Some(a) = m.assumptions.get(changed) {
        affected.insert(a.claim.clone());
    }
    if m.claims.contains_key(changed) {
        affected.insert(changed.clone());
    }
    let mut queue: VecDeque<Id> = VecDeque::new();
    if let Some(ds) = rev.get(changed) {
        for d in ds {
            queue.push_back(d.clone());
        }
    }
    while let Some(n) = queue.pop_front() {
        if affected.contains(&n) {
            continue;
        }
        affected.insert(n.clone());
        if let Some(ds) = rev.get(&n) {
            for d in ds {
                if !affected.contains(d) {
                    queue.push_back(d.clone());
                }
            }
        }
    }

    // A *premise* (asserted/observed/assumed claim: no derivation) that changed is
    // the corrected input; it must stay valid. Only its derived dependents are
    // invalidated. Removing it from `affected` also lets those dependents recompute
    // (their input is available).
    if m.claims
        .get(changed)
        .and_then(|c| c.derivation.as_ref())
        .is_none()
    {
        affected.remove(changed);
    }

    // Invalidate affected claims.
    let mut changes = vec![];
    let mut invalidated = vec![];
    for cid in &affected {
        if let Some(c) = m.claims.get_mut(cid) {
            let from = c.status;
            if from != ClaimStatus::Invalidated {
                c.status = ClaimStatus::Invalidated;
                c.invalidation_conditions
                    .push(format!("dependency {} changed", changed.as_str()));
                changes.push(StatusChange {
                    claim: cid.clone(),
                    label: c.label.clone(),
                    from,
                    to: ClaimStatus::Invalidated,
                    reason: format!("transitive dependency on {} changed", changed.as_str()),
                });
                invalidated.push(cid.clone());
                m.events.push(Event::Invalidate {
                    claim: cid.clone(),
                    reason: format!("dependency changed: {}", changed.as_str()),
                });
            }
        }
    }

    // Recompute derived claims whose inputs are all available, in
    // topological order (a worklist until fixpoint).
    let mut recomputed: HashSet<Id> = HashSet::new();
    let mut worklist: Vec<Id> = affected
        .iter()
        .filter(|c| {
            m.claims
                .get(*c)
                .and_then(|cl| cl.derivation.as_ref())
                .is_some()
        })
        .cloned()
        .collect();
    loop {
        let mut progressed = false;
        let mut next = vec![];
        for cid in worklist {
            let der_id = match m.claims.get(&cid).and_then(|c| c.derivation.clone()) {
                Some(d) => d,
                None => continue,
            };
            let d = match m.derivations.get(&der_id) {
                Some(d) => d.clone(),
                None => continue,
            };
            let inputs_ok = d
                .inputs
                .iter()
                .all(|i| !affected.contains(i) || recomputed.contains(i));
            let inputs_available = d.inputs.iter().all(|i| {
                m.claims
                    .get(i)
                    .map(|c| c.status != ClaimStatus::Invalidated)
                    .unwrap_or(false)
            });
            if inputs_ok && inputs_available {
                if let Some(def) = m.operations.get(&d.op).cloned() {
                    let ctx = m.claims.get(&cid).map(|c| c.context_id.clone());
                    let _ = m.derive(
                        &def.name,
                        &def.version,
                        &d.inputs,
                        ctx,
                        &d.output_label,
                        d.receipt.clone(),
                        executor,
                        trust_roots,
                    );
                    let _ = m.verify(cid.clone());
                    if let Some(c) = m.claims.get(&cid) {
                        changes.push(StatusChange {
                            claim: cid.clone(),
                            label: c.label.clone(),
                            from: ClaimStatus::Invalidated,
                            to: c.status,
                            reason: "recomputed from available inputs".into(),
                        });
                    }
                    recomputed.insert(cid.clone());
                    progressed = true;
                }
            } else {
                next.push(cid);
            }
        }
        worklist = next;
        if !progressed {
            break;
        }
    }

    let recomputed_vec: Vec<Id> = recomputed.into_iter().collect();
    let preserved: Vec<Id> = m
        .claims
        .iter()
        .filter(|(id, c)| c.status == ClaimStatus::Verified && !affected.contains(*id))
        .map(|(id, _)| id.clone())
        .collect();

    InvalidationReport {
        roots: vec![changed.clone()],
        invalidated,
        recomputed: recomputed_vec,
        preserved,
        changes,
    }
}

/// Re-execute every derivation from scratch (used for benchmark comparison
/// against incremental recomputation). Preserves each derivation's original
/// context so node identities are stable.
#[allow(clippy::type_complexity)]
pub fn full_recompute(
    m: &mut Module,
    executor: &dyn OpExecutor,
    trust_roots: &[TrustRoot],
) -> usize {
    let derivations: Vec<(String, String, Vec<Id>, Id, String, Option<Id>, Id)> = m
        .derivations
        .values()
        .map(|d| {
            let def = m.operations.get(&d.op).cloned();
            let ctx = m
                .claims
                .get(&d.output)
                .map(|c| c.context_id.clone())
                .unwrap_or_else(|| m.root_context.clone());
            (
                def.as_ref().map(|x| x.name.clone()).unwrap_or_default(),
                def.as_ref().map(|x| x.version.clone()).unwrap_or_default(),
                d.inputs.clone(),
                d.output.clone(),
                d.output_label.clone(),
                d.receipt.clone(),
                ctx,
            )
        })
        .collect();
    let mut n = 0;
    for (name, ver, inputs, _out, label, receipt, ctx) in derivations {
        let _ = m.derive(
            &name,
            &ver,
            &inputs,
            Some(ctx),
            &label,
            receipt,
            executor,
            trust_roots,
        );
        n += 1;
    }
    n
}

/// Convenience: run invalidation on a runtime's module in place.
pub fn invalidate_runtime(
    rt: &mut Runtime,
    changed: &Id,
    trust_roots: &[TrustRoot],
) -> InvalidationReport {
    invalidate_and_recompute(
        &mut rt.module,
        changed,
        &axiom_core::registry::BuiltinExecutor,
        trust_roots,
    )
}
