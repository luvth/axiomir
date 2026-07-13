//! Command implementations for the Axiom CLI.

use crate::Outcome;
use axiom_core::registry::{builtin_operations, BuiltinExecutor};
use axiom_core::{ClaimStatus, Id, Module, Receipt, Value};
use axiom_incremental::invalidate_and_recompute;
use axiom_parser::{parse_module, Diagnostic};
use axiom_runtime::Runtime;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::fs;

const EXIT_FAIL: i32 = 1;

// ===========================================================================
// Display helpers
// ===========================================================================

pub fn show_value(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Num(n) => n.display(),
        Value::Str(s) => format!("\"{}\"", s),
        Value::Sym(s) => format!("'{}", s),
        Value::Quantity(q) => q.display(),
        Value::Interval { lo, hi } => format!("[{}, {}]", lo.display(), hi.display()),
        Value::Relation(_) => "relation".into(),
        Value::Record(fields) => {
            let inner = fields
                .iter()
                .map(|(n, v)| format!("{}: {}", n, show_value(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {} }}", inner)
        }
        Value::Extension {
            ns, name, version, ..
        } => format!("ext:{ns}:{name}@{version}"),
    }
}

pub fn status_str(s: ClaimStatus) -> &'static str {
    match s {
        ClaimStatus::Asserted => "asserted",
        ClaimStatus::Assumed => "assumed",
        ClaimStatus::Observed => "observed",
        ClaimStatus::Pending => "pending",
        ClaimStatus::Disputed => "disputed",
        ClaimStatus::Challenged => "challenged",
        ClaimStatus::Verified => "verified",
        ClaimStatus::Invalidated => "invalidated",
        ClaimStatus::ExternallyAttested => "externally-attested",
    }
}

fn offset_to_line_col(src: &str, offset: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col = 0usize;
    for (i, ch) in src.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn render_diagnostics(src: &str, diags: &[Diagnostic]) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::new();
    for d in diags {
        let level = match d.level {
            axiom_parser::Level::Error => "error",
            axiom_parser::Level::Warning => "warning",
        };
        match d.span {
            Some(span) => {
                let (line, col) = offset_to_line_col(src, span.start);
                out.push_str(&format!(
                    "{}:{}: {}: {}\n",
                    line + 1,
                    col + 1,
                    level,
                    d.message
                ));
                if let Some(src_line) = lines.get(line) {
                    out.push_str(src_line);
                    out.push('\n');
                    out.push_str(&format!("{}^--- here\n", " ".repeat(col)));
                }
            }
            None => out.push_str(&format!("{}: {}\n", level, d.message)),
        }
    }
    out
}

// ===========================================================================
// Loading and execution
// ===========================================================================

fn read_source(path: &str) -> Result<String, Outcome> {
    fs::read_to_string(path).map_err(|e| Outcome {
        exit: EXIT_FAIL,
        human: format!("cannot read {}: {}", path, e),
        json: Json::Null,
    })
}

/// Parse a module file. On parse failure returns a structured Outcome.
fn parse_file(path: &str) -> Result<(String, axiom_parser::ModuleAst), Outcome> {
    let src = read_source(path)?;
    match parse_module(&src) {
        Ok(ast) => Ok((src, ast)),
        Err(diags) => {
            let rendered = render_diagnostics(&src, &diags);
            Err(Outcome {
                exit: EXIT_FAIL,
                human: format!("parse error in {}:\n{}", path, rendered),
                json: serde_json::json!({
                    "ok": false,
                    "stage": "parse",
                    "path": path,
                    "diagnostics": diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>(),
                }),
            })
        }
    }
}

struct ExecOpts {
    caps: Vec<String>,
    builtin_tool: bool,
    replay: Option<Vec<Receipt>>,
}

fn execute_file(path: &str, opts: &ExecOpts) -> Result<(String, Runtime), Outcome> {
    let (src, ast) = parse_file(path)?;
    let mut rt = Runtime::new(&ast.name);
    for c in &opts.caps {
        rt.grant(c);
    }
    if opts.builtin_tool {
        rt.with_builtin_tool();
    }
    if let Some(receipts) = &opts.replay {
        rt.replay_mode(receipts.clone());
    }
    match rt.execute(&ast) {
        Ok(()) => Ok((src, rt)),
        Err(e) => Err(Outcome {
            exit: EXIT_FAIL,
            human: format!("execution error in {}: {}", path, e),
            json: serde_json::json!({ "ok": false, "stage": "execute", "path": path, "error": e.to_string() }),
        }),
    }
}

// ===========================================================================
// Module summary
// ===========================================================================

fn module_summary(rt: &Runtime) -> (String, Json) {
    let m = &rt.module;
    let mut by_status: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut claims = vec![];
    for (_, c) in &m.claims {
        by_status
            .entry(status_str(c.status).to_string())
            .or_default()
            .push(c.label.clone());
        claims.push(serde_json::json!({
            "label": c.label,
            "status": status_str(c.status),
            "type": c.ty.name(),
            "value": show_value(&c.value),
            "uncertainty": c.uncertainty.kind(),
            "context": c.context_id.as_str(),
        }));
    }
    let verified = rt
        .verified_claims()
        .iter()
        .map(|id| {
            m.claims
                .get(id)
                .map(|c| c.label.clone())
                .unwrap_or_else(|| id.as_str())
        })
        .collect::<Vec<_>>();
    let human = format!(
        "module '{}'\ndigest: {}\nevent-log: {}\nclaims: {}\nverified: {}\ncontradictions: {}\nobligations: {}",
        m.name,
        rt.digest().as_str(),
        rt.event_log_digest().as_str(),
        m.claims.len(),
        verified.join(", "),
        m.contradictions.len(),
        m.obligations.len(),
    );
    let json = serde_json::json!({
        "ok": true,
        "name": m.name,
        "digest": rt.digest().as_str(),
        "event_log_digest": rt.event_log_digest().as_str(),
        "claim_count": m.claims.len(),
        "verified": verified,
        "contradiction_count": m.contradictions.len(),
        "obligation_count": m.obligations.len(),
        "by_status": by_status,
        "claims": claims,
    });
    (human, json)
}

// ===========================================================================
// Commands
// ===========================================================================

pub fn check(path: &str) -> Outcome {
    match execute_file(
        path,
        &ExecOpts {
            caps: vec![],
            builtin_tool: false,
            replay: None,
        },
    ) {
        Ok((_src, rt)) => {
            let (h, j) = module_summary(&rt);
            Outcome::ok(format!("check OK\n{}", h), j)
        }
        Err(o) => o,
    }
}

pub fn fmt(path: &str, check_only: bool) -> Outcome {
    let src = match read_source(path) {
        Ok(s) => s,
        Err(o) => return o,
    };
    match axiom_parser::format_source(&src) {
        Ok(formatted) => {
            if check_only {
                if formatted.trim_end() == src.trim_end() {
                    Outcome::ok(
                        format!("{}: formatting stable", path),
                        serde_json::json!({ "ok": true, "formatted": true }),
                    )
                } else {
                    Outcome {
                        exit: EXIT_FAIL,
                        human: format!("{}: would reformat:\n{}", path, formatted),
                        json: serde_json::json!({ "ok": false, "formatted": false, "canonical": formatted }),
                    }
                }
            } else {
                Outcome::ok(
                    formatted.clone(),
                    serde_json::json!({ "ok": true, "formatted": formatted }),
                )
            }
        }
        Err(diags) => Outcome {
            exit: EXIT_FAIL,
            human: format!(
                "format error in {}:\n{}",
                path,
                render_diagnostics(&src, &diags)
            ),
            json: serde_json::json!({ "ok": false, "stage": "format", "diagnostics": diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>() }),
        },
    }
}

pub fn run(
    path: &str,
    caps: &[String],
    builtin_tool: bool,
    emit_receipts: Option<&str>,
) -> Outcome {
    match execute_file(
        path,
        &ExecOpts {
            caps: caps.to_vec(),
            builtin_tool,
            replay: None,
        },
    ) {
        Ok((_src, rt)) => {
            let (h, j) = module_summary(&rt);
            if let Some(out) = emit_receipts {
                let log = ReceiptLog {
                    source: rt.module.name.clone(),
                    receipts: rt.module.receipts.values().cloned().collect(),
                };
                match fs::write(out, serde_json::to_string_pretty(&log).unwrap()) {
                    Ok(()) => {}
                    Err(e) => {
                        return Outcome::fail(format!("cannot write receipts: {}", e), Json::Null)
                    }
                }
            }
            Outcome::ok(h, j)
        }
        Err(o) => o,
    }
}

pub fn verify(path: &str, caps: &[String], builtin_tool: bool) -> Outcome {
    match execute_file(
        path,
        &ExecOpts {
            caps: caps.to_vec(),
            builtin_tool,
            replay: None,
        },
    ) {
        Ok((_src, rt)) => {
            let m = &rt.module;
            let mut lines = vec![format!("module '{}' verification", m.name)];
            let mut verified = vec![];
            let mut blocked = vec![];
            for (_, c) in &m.claims {
                if c.status == ClaimStatus::Verified {
                    verified.push(c.label.clone());
                } else if c.derivation.is_some() {
                    blocked.push(format!("{} ({})", c.label, status_str(c.status)));
                }
            }
            lines.push(format!("verified: {}", verified.join(", ")));
            lines.push(format!("derived-but-not-verified: {}", blocked.join(", ")));
            let json = serde_json::json!({
                "ok": true,
                "verified": verified,
                "not_verified": blocked,
                "digest": rt.digest().as_str(),
            });
            Outcome::ok(lines.join("\n"), json)
        }
        Err(o) => o,
    }
}

pub fn explain(path: &str, claim_label: &str) -> Outcome {
    match execute_file(
        path,
        &ExecOpts {
            caps: vec![],
            builtin_tool: false,
            replay: None,
        },
    ) {
        Ok((_src, rt)) => match rt.explain(claim_label) {
            Ok(e) => {
                let mut lines = vec![];
                lines.push(format!("claim '{}' ({})", e.label, e.claim.as_str()));
                lines.push(format!("  status:   {}", status_str(e.status)));
                lines.push(format!("  type:     {}", e.ty.name()));
                lines.push(format!("  value:    {}", show_value(&e.value)));
                lines.push(format!("  context:  {}", e.context.as_str()));
                lines.push(format!("  uncertainty: {}", e.uncertainty.kind()));
                if let Some(d) = &e.derivation {
                    lines.push(format!("  derived by: {}@{}", d.op, d.op_version));
                    lines.push(format!(
                        "    inputs: {}",
                        d.inputs
                            .iter()
                            .map(|i| i.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    lines.push(format!(
                        "    receipt: {}",
                        d.receipt
                            .as_ref()
                            .map(|r| r.as_str())
                            .unwrap_or_else(|| "none".into())
                    ));
                } else {
                    lines.push("  derived by: none (asserted/observed/assumed)".to_string());
                }
                lines.push(format!(
                    "  obligations: {}",
                    e.obligations
                        .iter()
                        .map(|o| format!("{}[{}]={}", o.kind, o.severity, o.state))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                lines.push(format!(
                    "  assumptions: {}",
                    e.assumptions
                        .iter()
                        .map(|a| a.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                lines.push(format!(
                    "  evidence: {}",
                    e.evidence
                        .iter()
                        .map(|a| a.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                lines.push(format!(
                    "  contradicts: {}",
                    e.contradicts
                        .iter()
                        .map(|a| a.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                lines.push(format!(
                    "  invalidation_conditions: {}",
                    e.invalidation_conditions.join("; ")
                ));
                let json = serde_json::json!({
                    "label": e.label, "status": status_str(e.status), "type": e.ty.name(),
                    "value": show_value(&e.value), "context": e.context.as_str(),
                    "derivation": e.derivation.as_ref().map(|d| serde_json::json!({"op": d.op, "version": d.op_version, "inputs": d.inputs.iter().map(|i| i.as_str()).collect::<Vec<_>>()})),
                    "obligations": e.obligations.iter().map(|o| serde_json::json!({"kind": o.kind, "severity": o.severity, "state": o.state})).collect::<Vec<_>>(),
                    "assumptions": e.assumptions.iter().map(|a| a.as_str()).collect::<Vec<_>>(),
                    "evidence": e.evidence.iter().map(|a| a.as_str()).collect::<Vec<_>>(),
                    "contradicts": e.contradicts.iter().map(|a| a.as_str()).collect::<Vec<_>>(),
                });
                Outcome::ok(lines.join("\n"), json)
            }
            Err(e) => Outcome::fail(format!("explain error: {}", e), Json::Null),
        },
        Err(o) => o,
    }
}

pub fn trace(path: &str, claim_label: &str) -> Outcome {
    match execute_file(
        path,
        &ExecOpts {
            caps: vec![],
            builtin_tool: false,
            replay: None,
        },
    ) {
        Ok((_src, rt)) => {
            let id = match rt.claim(claim_label) {
                Ok(id) => id,
                Err(e) => return Outcome::fail(format!("trace error: {}", e), Json::Null),
            };
            let mut lines = vec![];
            trace_node(&rt.module, &id, 0, &mut lines);
            let json = serde_json::json!({ "root": claim_label, "trace": lines.join("\n") });
            Outcome::ok(lines.join("\n"), json)
        }
        Err(o) => o,
    }
}

fn trace_node(m: &Module, id: &Id, depth: usize, out: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    if let Some(c) = m.claims.get(id) {
        out.push(format!(
            "{}{} = {} : {} [{}]",
            indent,
            c.label,
            show_value(&c.value),
            c.ty.name(),
            status_str(c.status)
        ));
        if let Some(did) = &c.derivation {
            if let Some(d) = m.derivations.get(did) {
                let opname = m
                    .operations
                    .get(&d.op)
                    .map(|o| o.name.clone())
                    .unwrap_or_else(|| d.op.as_str());
                out.push(format!("{}  <- {}@{}", indent, opname, d.op_version));
                for inp in &d.inputs {
                    trace_node(m, inp, depth + 2, out);
                }
            }
        }
    }
}

pub fn contradictions(path: &str) -> Outcome {
    match execute_file(
        path,
        &ExecOpts {
            caps: vec![],
            builtin_tool: false,
            replay: None,
        },
    ) {
        Ok((_src, rt)) => {
            let cons = rt.contradictions();
            let mut lines = vec![format!("contradictions: {}", cons.len())];
            let mut arr = vec![];
            for c in &cons {
                lines.push(format!(
                    "- {} kind={} claims={} context={}",
                    c.id.as_str(),
                    kind_name(&c.kind),
                    c.claims
                        .iter()
                        .map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    c.context.as_str()
                ));
                lines.push(format!("    witness: {}", c.witness.summary));
                arr.push(serde_json::json!({
                    "id": c.id.as_str(),
                    "kind": kind_name(&c.kind),
                    "claims": c.claims.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
                    "context": c.context.as_str(),
                    "witness": c.witness.summary,
                }));
            }
            Outcome::ok(
                lines.join("\n"),
                serde_json::json!({ "count": cons.len(), "contradictions": arr }),
            )
        }
        Err(o) => o,
    }
}

fn kind_name(k: &axiom_core::ContradictionKind) -> String {
    use axiom_core::ContradictionKind::*;
    match k {
        PropositionNegation => "proposition-negation".to_string(),
        IncompatibleEquality => "incompatible-equality".to_string(),
        DisjointInterval => "disjoint-interval".to_string(),
        IncompatibleUnit => "incompatible-unit".to_string(),
        MutuallyExclusiveMembership => "mutually-exclusive-membership".to_string(),
        ViolatedPostcondition => "violated-postcondition".to_string(),
        EvidenceConflict => "evidence-conflict".to_string(),
        AssumptionConflict => "assumption-conflict".to_string(),
        Extension(s) => s.clone(),
    }
}

pub fn invalidate(path: &str, node_label: &str) -> Outcome {
    match execute_file(
        path,
        &ExecOpts {
            caps: vec![],
            builtin_tool: false,
            replay: None,
        },
    ) {
        Ok((_src, mut rt)) => {
            let id = match rt.claim(node_label) {
                Ok(id) => id,
                Err(e) => return Outcome::fail(format!("invalidate error: {}", e), Json::Null),
            };
            let report = invalidate_and_recompute(&mut rt.module, &id, &BuiltinExecutor, &[]);
            let mut lines = vec![];
            lines.push(format!(
                "invalidation frontier rooted at '{}' ({})",
                node_label,
                id.as_str()
            ));
            lines.push(format!(
                "  invalidated: {}",
                report
                    .invalidated
                    .iter()
                    .map(|i| i.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            lines.push(format!(
                "  recomputed:  {}",
                report
                    .recomputed
                    .iter()
                    .map(|i| i.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            lines.push(format!(
                "  preserved:   {} verified claims untouched",
                report.preserved.len()
            ));
            lines.push("  status changes:".to_string());
            for ch in &report.changes {
                lines.push(format!(
                    "    {}: {} -> {} ({})",
                    ch.label,
                    status_str(ch.from),
                    status_str(ch.to),
                    ch.reason
                ));
            }
            let json = serde_json::json!({
                "root": node_label,
                "invalidated": report.invalidated.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
                "recomputed": report.recomputed.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
                "preserved_count": report.preserved.len(),
                "changes": report.changes.iter().map(|c| serde_json::json!({"label": c.label, "from": status_str(c.from), "to": status_str(c.to), "reason": c.reason})).collect::<Vec<_>>(),
            });
            Outcome::ok(lines.join("\n"), json)
        }
        Err(o) => o,
    }
}

#[derive(Serialize, Deserialize)]
struct ReceiptLog {
    source: String,
    receipts: Vec<Receipt>,
}

pub fn replay(log_path: &str) -> Outcome {
    let text = match fs::read_to_string(log_path) {
        Ok(t) => t,
        Err(e) => return Outcome::fail(format!("cannot read {}: {}", log_path, e), Json::Null),
    };
    let log: ReceiptLog = match serde_json::from_str(&text) {
        Ok(l) => l,
        Err(e) => return Outcome::fail(format!("invalid receipt log: {}", e), Json::Null),
    };
    // Live run (capability granted + builtin tool) to get the reference digest.
    let mut live = Runtime::new("replay");
    live.grant("tool:calculator");
    live.with_builtin_tool();
    let ast_live = match parse_module(&log.source) {
        Ok(a) => a,
        Err(d) => {
            return Outcome::fail(
                format!(
                    "parse error in receipt log source:\n{:?}",
                    d.iter().map(|x| &x.message).collect::<Vec<_>>()
                ),
                Json::Null,
            )
        }
    };
    match live.execute(&ast_live) {
        Ok(()) => {}
        Err(e) => return Outcome::fail(format!("live execution failed: {}", e), Json::Null),
    }
    let digest_live = live.digest();
    // Replay run: no capability, receipts only.
    let mut rep = Runtime::new("replay");
    rep.replay_mode(log.receipts.clone());
    let ast_rep = parse_module(&log.source).unwrap();
    match rep.execute(&ast_rep) {
        Ok(()) => {}
        Err(e) => return Outcome::fail(format!("replay execution failed: {}", e), Json::Null),
    }
    let digest_replay = rep.digest();
    let equal = digest_live == digest_replay;
    let lines = format!(
        "replay\n  live digest:    {}\n  replay digest:  {}\n  identical:     {}\n  receipts used: {}",
        digest_live.as_str(),
        digest_replay.as_str(),
        equal,
        log.receipts.len(),
    );
    let json = serde_json::json!({
        "live_digest": digest_live.as_str(),
        "replay_digest": digest_replay.as_str(),
        "identical": equal,
        "receipts": log.receipts.len(),
    });
    if equal {
        Outcome::ok(lines, json)
    } else {
        Outcome::fail(lines, json)
    }
}

pub fn diff(path_a: &str, path_b: &str) -> Outcome {
    let ra = match execute_file(
        path_a,
        &ExecOpts {
            caps: vec![],
            builtin_tool: false,
            replay: None,
        },
    ) {
        Ok((_s, rt)) => rt,
        Err(o) => return o,
    };
    let rb = match execute_file(
        path_b,
        &ExecOpts {
            caps: vec![],
            builtin_tool: false,
            replay: None,
        },
    ) {
        Ok((_s, rt)) => rt,
        Err(o) => return o,
    };
    let mut lines = vec![format!("diff {} <-> {}", path_a, path_b)];
    let a_status: BTreeMap<String, (String, String)> = ra
        .module
        .claims
        .iter()
        .map(|(_, c)| {
            (
                c.label.clone(),
                (status_str(c.status).to_string(), show_value(&c.value)),
            )
        })
        .collect();
    let b_status: BTreeMap<String, (String, String)> = rb
        .module
        .claims
        .iter()
        .map(|(_, c)| {
            (
                c.label.clone(),
                (status_str(c.status).to_string(), show_value(&c.value)),
            )
        })
        .collect();
    let mut only_a = vec![];
    let mut only_b = vec![];
    let mut changed = vec![];
    for (k, va) in &a_status {
        match b_status.get(k) {
            None => only_a.push(k.clone()),
            Some(vb) => {
                if va != vb {
                    changed.push(format!("{}: {} {} -> {} {}", k, va.0, va.1, vb.0, vb.1));
                }
            }
        }
    }
    for k in b_status.keys() {
        if !a_status.contains_key(k) {
            only_b.push(k.clone());
        }
    }
    lines.push(format!("  only in A: {}", only_a.join(", ")));
    lines.push(format!("  only in B: {}", only_b.join(", ")));
    lines.push(format!("  changed:   {}", changed.join("; ")));
    lines.push(format!("  digest A: {}", ra.digest().as_str()));
    lines.push(format!("  digest B: {}", rb.digest().as_str()));
    let json = serde_json::json!({
        "only_in_a": only_a, "only_in_b": only_b, "changed": changed,
        "digest_a": ra.digest().as_str(), "digest_b": rb.digest().as_str(),
    });
    Outcome::ok(lines.join("\n"), json)
}

pub fn graph(path: &str) -> Outcome {
    match execute_file(
        path,
        &ExecOpts {
            caps: vec![],
            builtin_tool: false,
            replay: None,
        },
    ) {
        Ok((_src, rt)) => {
            let m = &rt.module;
            let label_of: BTreeMap<Id, String> = m
                .claims
                .iter()
                .map(|(id, c)| (id.clone(), c.label.clone()))
                .collect();
            let mut dot = vec!["digraph axiom {".to_string()];
            for (id, c) in &m.claims {
                let n = label_of.get(id).cloned().unwrap_or_else(|| id.as_str());
                dot.push(format!(
                    "  \"{}\" [label=\"{}\\n{}\" shape=box];",
                    id.as_str(),
                    n,
                    status_str(c.status)
                ));
                for dep in c.depends_on(m) {
                    let dn = label_of.get(&dep).cloned().unwrap_or_else(|| dep.as_str());
                    dot.push(format!(
                        "  \"{}\" -> \"{}\" [label=\"{}\"];",
                        dep.as_str(),
                        id.as_str(),
                        dn
                    ));
                }
            }
            dot.push("}".to_string());
            let dot_str = dot.join("\n");
            let json = serde_json::json!({
                "nodes": m.claims.len(),
                "edges": m.claims.values().map(|c| c.depends_on(m).len()).sum::<usize>(),
                "dot": dot_str,
            });
            Outcome::ok(dot_str, json)
        }
        Err(o) => o,
    }
}

pub fn inspect(path: &str, node_label: &str) -> Outcome {
    match execute_file(
        path,
        &ExecOpts {
            caps: vec![],
            builtin_tool: false,
            replay: None,
        },
    ) {
        Ok((_src, rt)) => {
            let id = match rt.claim(node_label) {
                Ok(id) => id,
                Err(e) => return Outcome::fail(format!("inspect error: {}", e), Json::Null),
            };
            if let Some(c) = rt.module.claims.get(&id) {
                let lines = format!(
                    "inspect '{}'\n  id: {}\n  semantic_id: {}\n  status: {}\n  type: {}\n  value: {}\n  uncertainty: {}\n  context: {}\n  assumptions: {}\n  evidence: {}\n  derivation: {}\n  obligations: {}\n  invalidation_conditions: {}",
                    c.label, c.id.as_str(), c.semantic_id.as_str(), status_str(c.status), c.ty.name(), show_value(&c.value),
                    c.uncertainty.kind(), c.context_id.as_str(),
                    c.assumptions.iter().map(|a| a.as_str()).collect::<Vec<_>>().join(", "),
                    c.evidence.iter().map(|a| a.as_str()).collect::<Vec<_>>().join(", "),
                    c.derivation.as_ref().map(|d| d.as_str()).unwrap_or_else(|| "none".into()),
                    c.obligations.iter().map(|o| o.as_str()).collect::<Vec<_>>().join(", "),
                    c.invalidation_conditions.join("; "),
                );
                let json = serde_json::json!({
                    "label": c.label, "id": c.id.as_str(), "semantic_id": c.semantic_id.as_str(),
                    "status": status_str(c.status), "type": c.ty.name(), "value": show_value(&c.value),
                    "uncertainty": c.uncertainty.kind(), "context": c.context_id.as_str(),
                    "obligations": c.obligations.iter().map(|o| o.as_str()).collect::<Vec<_>>(),
                });
                Outcome::ok(lines, json)
            } else {
                Outcome::fail(format!("claim {} not found", node_label), Json::Null)
            }
        }
        Err(o) => o,
    }
}

pub fn doctor() -> Outcome {
    let mut lines = vec![];
    lines.push("axiom doctor — self-test".to_string());
    lines.push(format!("  format version: {}", env!("CARGO_PKG_VERSION")));
    let ops = builtin_operations();
    lines.push(format!(
        "  builtin operations: {}",
        ops.iter()
            .map(|o| o.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    // Self-test: parse + execute a trivial module.
    let sample = "module selftest \"1\"\nassert a = 2 : rational\nassert b = 3 : rational\nderive out = add(a, b) : rational\nverify out\n";
    match axiom_parser::parse_module(sample) {
        Ok(ast) => {
            let mut rt = Runtime::new("selftest");
            match rt.execute(&ast) {
                Ok(()) => {
                    let verified = rt.verified_claims().len();
                    lines.push(format!(
                        "  selftest: parsed + executed OK, verified claims = {}",
                        verified
                    ));
                    lines.push("  status: HEALTHY".to_string());
                    Outcome::ok(
                        lines.join("\n"),
                        serde_json::json!({ "healthy": true, "builtin_ops": ops.len(), "verified": verified }),
                    )
                }
                Err(e) => Outcome::fail(
                    format!("{}\n  selftest FAILED: {}", lines.join("\n"), e),
                    serde_json::json!({ "healthy": false, "error": e.to_string() }),
                ),
            }
        }
        Err(d) => Outcome::fail(
            format!(
                "{}\n  selftest parse FAILED: {:?}",
                lines.join("\n"),
                d.iter().map(|x| &x.message).collect::<Vec<_>>()
            ),
            Json::Null,
        ),
    }
}
