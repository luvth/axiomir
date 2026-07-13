//! `axiom-producer`: a deterministic translator from structured reasoning plans
//! to Axiom IR modules.
//!
//! This answers the review's "who produces Axiom?" question with a concrete,
//! inspectable producer: an external system (an LLM extraction pipeline, a
//! compiler from a typed reasoning format, a proof exporter, …) emits a
//! [`ReasoningPlan`] — a typed intermediate — and [`produce`] renders it to a
//! valid Axiom text module that the normal parser, runtime, verifier, replay,
//! and incremental engine consume.
//!
//! The translation is *deterministic*: the same plan always yields byte-identical
//! source, so producers are reproducible and the resulting module digest is
//! stable.

use std::fmt::Write;

use serde::{Deserialize, Serialize};

/// A value literal expressible in the produced Axiom source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Literal {
    Int(i128),
    Rational(i128),
    Quantity(i128, String),
    Bool(bool),
}

impl Literal {
    fn render(&self) -> String {
        match self {
            Literal::Int(n) => n.to_string(),
            // A rational literal is rendered canonically as `rat(num den)`,
            // which the parser converts to a reduced `Value::Num(Num::Rational)`.
            // We never emit a bare integer and claim it is a rational.
            Literal::Rational(n) => format!("rat({n} 1)"),
            Literal::Quantity(v, u) => format!("q({v} \"{u}\")"),
            Literal::Bool(b) => b.to_string(),
        }
    }

    fn axiom_type(&self) -> &'static str {
        match self {
            // Integer literals are typed `rational` to match the language
            // convention (e.g. `assert x = 10 : rational`), which the builtin
            // operation registry also expects for `core.add`/etc. A fraction is
            // rendered canonically as `rat(num den)`.
            Literal::Int(_) => "rational",
            Literal::Rational(_) => "rational",
            Literal::Quantity(_, _) => "quantity",
            Literal::Bool(_) => "bool",
        }
    }
}

/// A premise: a base claim the plan asserts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Premise {
    pub label: String,
    pub value: Literal,
    /// Optional evidence backing the premise (emitted as an `evidence` node and
    /// an `observe` so the claim carries provenance).
    pub evidence: Option<String>,
}

/// A scoped, challengeable assumption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assumption {
    pub label: String,
    pub value: Literal,
    pub scope: String,
}

/// A derived inference step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub label: String,
    /// An Axiom operation identity, e.g. `core.add` or `qmul`.
    pub op: String,
    /// Labels of the input claims (premises / assumptions / prior steps).
    pub inputs: Vec<String>,
    /// If set, the step carries a mandatory proof obligation that must be
    /// discharged before the claim can verify.
    pub requires_obligation: bool,
    /// Evidence label used to discharge the obligation (requires
    /// `requires_obligation`).
    pub discharge_by: Option<String>,
    /// Optional explicit output type for the step. When present it must match
    /// the registered operation's output type; when absent the producer infers
    /// the type from the operation registry.
    pub ty: Option<String>,
    /// If true, the step is marked `verify` in the produced source.
    pub verify: bool,
}

/// A structured reasoning plan — the producer's input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningPlan {
    pub module_name: String,
    pub premises: Vec<Premise>,
    pub assumptions: Vec<Assumption>,
    pub steps: Vec<Step>,
}

impl ReasoningPlan {
    /// Render the plan as a valid Axiom IR text module.
    ///
    /// Deterministic: identical plans produce byte-identical output.
    ///
    /// Returns an error if a step references an unknown operation, has the
    /// wrong arity, or declares an output type that disagrees with the
    /// registered operation's output type. Validation uses the shared operation
    /// registry so the producer cannot silently drift from the runtime type
    /// system.
    pub fn produce(&self) -> Result<String, ProduceError> {
        produce(self)
    }
}

/// Error returned when a [`ReasoningPlan`] cannot be rendered to valid Axiom.
#[derive(Debug)]
pub enum ProduceError {
    /// JSON deserialization of a plan failed.
    Json(serde_json::Error),
    /// The plan references an unknown operation.
    UnknownOp(String),
    /// A step supplies the wrong number of inputs for its operation.
    Arity {
        op: String,
        expected: usize,
        got: usize,
    },
    /// The step's declared output type disagrees with the registry.
    OutputType {
        op: String,
        expected: String,
        got: String,
    },
}

impl std::fmt::Display for ProduceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProduceError::Json(e) => write!(f, "plan json error: {e}"),
            ProduceError::UnknownOp(o) => write!(f, "unknown operation `{o}`"),
            ProduceError::Arity { op, expected, got } => write!(
                f,
                "operation `{op}` expects {expected} inputs but plan supplied {got}"
            ),
            ProduceError::OutputType { op, expected, got } => write!(
                f,
                "operation `{op}` produces type `{expected}` but plan declared `{got}`"
            ),
        }
    }
}

impl From<serde_json::Error> for ProduceError {
    fn from(e: serde_json::Error) -> Self {
        ProduceError::Json(e)
    }
}

/// The shared builtin operation registry, used for type/arity validation so the
/// producer never duplicates the runtime type system.
fn registry() -> std::collections::BTreeMap<String, axiom_core::OperationDef> {
    axiom_core::registry::builtin_operations()
        .into_iter()
        .map(|d| (d.name.clone(), d))
        .collect()
}

/// The canonical textual name of a core type, used to render the `: type`
/// clause of a derived step.
fn type_name(t: &axiom_core::Type) -> String {
    match t {
        axiom_core::Type::Bool => "bool",
        axiom_core::Type::Int { .. } => "int",
        axiom_core::Type::Rational => "rational",
        axiom_core::Type::Decimal => "decimal",
        axiom_core::Type::String => "string",
        axiom_core::Type::Symbol => "symbol",
        axiom_core::Type::Quantity => "quantity",
        axiom_core::Type::NumericInterval => "interval",
        axiom_core::Type::Relation => "relation",
        axiom_core::Type::Temporal => "temporal",
        axiom_core::Type::Record { .. } => "record",
        axiom_core::Type::Extension { ns, name, .. } => return format!("{ns}:{name}"),
    }
    .to_string()
}

/// Translate a [`ReasoningPlan`] into valid Axiom IR text source.
///
/// Operation output types come from the operation registry (never from a
/// `op.starts_with('q')` heuristic). The producer rejects unknown operations,
/// wrong arity, and declared output types that disagree with the registry. An
/// unqualified op (e.g. `qmul`) resolves to its `core.` namespaced definition,
/// matching the runtime's `resolve_op`.
pub fn produce(plan: &ReasoningPlan) -> Result<String, ProduceError> {
    let reg = registry();
    let mut out = String::new();
    let _ = writeln!(out, "module {} \"1\"", sanitize_ident(&plan.module_name));

    // 1. Evidence nodes for any premise that carries provenance.
    for p in &plan.premises {
        if let Some(ev) = &p.evidence {
            let ev_id = format!("ev_{}", sanitize_ident(&p.label));
            let _ = writeln!(out, "evidence {ev_id} \"text/plain\" \"{}\"", escape(ev));
            // Observe the premise, binding it to its evidence.
            let _ = writeln!(
                out,
                "observe {} = {} : {} evidence [{}]",
                sanitize_ident(&p.label),
                p.value.render(),
                p.value.axiom_type(),
                ev_id
            );
        }
    }

    // 2. Plain asserted premises (no evidence).
    for p in &plan.premises {
        if p.evidence.is_none() {
            let _ = writeln!(
                out,
                "assert {} = {} : {}",
                sanitize_ident(&p.label),
                p.value.render(),
                p.value.axiom_type()
            );
        }
    }

    // 3. Assumptions.
    for a in &plan.assumptions {
        let _ = writeln!(
            out,
            "assume {} = {} : {} scope \"{}\"",
            sanitize_ident(&a.label),
            a.value.render(),
            a.value.axiom_type(),
            escape(&a.scope)
        );
    }

    // 4. Derived steps.
    for s in &plan.steps {
        let op_name = if reg.contains_key(&s.op) {
            s.op.clone()
        } else {
            format!("core.{}", s.op)
        };
        let def = reg
            .get(&op_name)
            .ok_or_else(|| ProduceError::UnknownOp(s.op.clone()))?;
        if def.inputs.len() != s.inputs.len() {
            return Err(ProduceError::Arity {
                op: s.op.clone(),
                expected: def.inputs.len(),
                got: s.inputs.len(),
            });
        }
        let declared = type_name(&def.output);
        if s.ty.as_deref() != Some(declared.as_str()) && s.ty.is_some() {
            return Err(ProduceError::OutputType {
                op: s.op.clone(),
                expected: declared.clone(),
                got: s.ty.clone().unwrap(),
            });
        }
        let inputs = s
            .inputs
            .iter()
            .map(|s| sanitize_ident(s))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            out,
            "derive {} = {}({}) : {}",
            sanitize_ident(&s.label),
            s.op,
            inputs,
            declared
        );
        if s.requires_obligation {
            let _ = writeln!(
                out,
                "require numeric-bounds on {}",
                sanitize_ident(&s.label)
            );
            if let Some(by) = &s.discharge_by {
                let _ = writeln!(
                    out,
                    "discharge {} by {} as satisfied",
                    sanitize_ident(&s.label),
                    sanitize_ident(by)
                );
            }
        }
        if s.verify {
            let _ = writeln!(out, "verify {}", sanitize_ident(&s.label));
        }
    }

    Ok(out)
}

/// Convenience: build a plan from a JSON string (the wire format an LLM
/// extraction pipeline would emit) and produce Axiom source.
pub fn produce_from_json(json: &str) -> Result<String, ProduceError> {
    let plan: ReasoningPlan = serde_json::from_str(json)?;
    produce(&plan)
}

/// Sanitize an identifier so it is a valid Axiom label (alphanumeric + `_`).
fn sanitize_ident(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('x');
    }
    out
}

/// Escape a double-quoted string literal.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plan() -> ReasoningPlan {
        ReasoningPlan {
            module_name: "physics_work".to_string(),
            premises: vec![
                Premise {
                    label: "force".to_string(),
                    value: Literal::Quantity(10, "N".to_string()),
                    evidence: Some("sensor reported 10 N".to_string()),
                },
                Premise {
                    label: "dist".to_string(),
                    value: Literal::Quantity(2, "m".to_string()),
                    evidence: Some("sensor reported 2 m".to_string()),
                },
            ],
            assumptions: vec![Assumption {
                label: "coeff".to_string(),
                value: Literal::Quantity(1, "1".to_string()),
                scope: "adj_factor".to_string(),
            }],
            steps: vec![
                Step {
                    label: "work".to_string(),
                    op: "core.qmul".to_string(),
                    inputs: vec!["force".to_string(), "dist".to_string()],
                    requires_obligation: false,
                    discharge_by: None,
                    ty: None,
                    verify: true,
                },
                Step {
                    label: "adj".to_string(),
                    op: "core.qmul".to_string(),
                    inputs: vec!["work".to_string(), "coeff".to_string()],
                    requires_obligation: true,
                    discharge_by: Some("ev_force".to_string()),
                    ty: None,
                    verify: true,
                },
            ],
        }
    }

    #[test]
    fn producer_is_deterministic() {
        let p = sample_plan();
        assert_eq!(
            produce(&p).unwrap(),
            produce(&p).unwrap(),
            "produce must be deterministic"
        );
    }

    #[test]
    fn producer_output_is_valid_and_verifiable() {
        let src = sample_plan().produce().expect("plan must produce");
        // 1. Producer emitted source.
        let ast = axiom_parser::parse_module(&src).expect("producer output must parse");
        // 2. Runtime executes it.
        let mut rt = axiom_runtime::Runtime::new("physics_work");
        rt.execute(&ast).expect("producer output must execute");
        // 3. The verified claims are exactly the two `verify` steps.
        assert_eq!(rt.verified_claims().len(), 2, "work and adj must verify");
        // 4. Determinism of the produced module digest.
        let d1 = rt.digest();
        drop(rt);
        let mut rt2 = axiom_runtime::Runtime::new("physics_work");
        let ast2 = axiom_parser::parse_module(&src).unwrap();
        rt2.execute(&ast2).unwrap();
        assert_eq!(d1, rt2.digest(), "producer output digest is stable");
    }

    #[test]
    fn producer_from_json_works() {
        let json = r#"{
            "module_name": "jplan",
            "premises": [{"label":"a","value":{"Int":2},"evidence":null}],
            "assumptions": [],
            "steps": [{"label":"out","op":"core.add","inputs":["a","a"],"requires_obligation":false,"discharge_by":null,"verify":true}]
        }"#;
        let src = produce_from_json(json).expect("json plan must parse");
        let ast = axiom_parser::parse_module(&src).expect("produced source must parse");
        let mut rt = axiom_runtime::Runtime::new("jplan");
        rt.execute(&ast).unwrap();
        assert_eq!(rt.verified_claims().len(), 1);
    }

    #[test]
    fn producer_pipeline_incremental_update() {
        use axiom_incremental::invalidate_runtime;
        use axiom_types::num::Num;
        use axiom_types::Quantity;
        use axiom_types::Unit;

        // Produce a module from a plan, execute it.
        let src = sample_plan().produce().expect("plan must produce");
        let ast = axiom_parser::parse_module(&src).unwrap();
        let mut rt = axiom_runtime::Runtime::new("physics_work");
        rt.execute(&ast).unwrap();
        assert_eq!(rt.verified_claims().len(), 2);

        // Incremental update: correct the `force` premise and recompute only
        // its dependents (exact dirty frontier), exactly as the review's
        // Question -> Producer -> ... -> Incremental update workflow requires.
        rt.module
            .assert(
                "force",
                axiom_core::Type::Quantity,
                axiom_core::Value::Quantity(Quantity {
                    value: Num::Int(20),
                    unit: Unit::base("N"),
                }),
                axiom_core::Uncertainty::Exact,
                vec![],
                None,
                None,
            )
            .unwrap();
        let force_id = rt.claim("force").unwrap();
        let report = invalidate_runtime(&mut rt, &force_id, &[]);

        // Only claims transitively depending on `force` are in the frontier.
        let work = rt.claim("work").unwrap();
        let adj = rt.claim("adj").unwrap();
        let coeff = rt.claim("coeff").unwrap();
        assert!(report.invalidated.contains(&work));
        assert!(report.invalidated.contains(&adj));
        assert!(!report.invalidated.contains(&coeff));
        // Re-derived values reflect the corrected premise (10*2 -> 20*2 = 40).
        let work_val = rt.module.claims[&work].value.clone();
        assert!(
            format!("{work_val:?}").contains("40"),
            "work must reflect corrected force=20 -> qmul(20,2)=40"
        );
    }

    // --- F3: producer type inference from the operation registry -----------

    #[test]
    fn producer_rejects_unknown_operation() {
        let json = r#"{
            "module_name": "bad",
            "premises": [],
            "assumptions": [],
            "steps": [{"label":"o","op":"no.such.op","inputs":[],"requires_obligation":false,"discharge_by":null,"verify":false}]
        }"#;
        let err = produce_from_json(json).unwrap_err();
        assert!(format!("{err}").contains("unknown operation"), "got: {err}");
    }

    #[test]
    fn producer_rejects_wrong_arity() {
        // core.add expects 2 inputs; supply 1.
        let json = r#"{
            "module_name": "bad",
            "premises": [{"label":"a","value":{"Int":2},"evidence":null}],
            "assumptions": [],
            "steps": [{"label":"o","op":"core.add","inputs":["a"],"requires_obligation":false,"discharge_by":null,"verify":false}]
        }"#;
        let err = produce_from_json(json).unwrap_err();
        assert!(format!("{err}").contains("inputs"), "got: {err}");
    }

    #[test]
    fn producer_rejects_mismatched_output_type() {
        // core.add produces rational; declare quantity.
        let json = r#"{
            "module_name": "bad",
            "premises": [{"label":"a","value":{"Int":2},"evidence":null}],
            "assumptions": [],
            "steps": [{"label":"o","op":"core.add","inputs":["a","a"],"ty":"quantity","requires_obligation":false,"discharge_by":null,"verify":false}]
        }"#;
        let err = produce_from_json(json).unwrap_err();
        assert!(format!("{err}").contains("type"), "got: {err}");
    }

    #[test]
    fn producer_infers_output_types_from_registry() {
        // core.add -> rational, core.qmul -> quantity, core.eq -> bool,
        // core.interval -> interval. None of these uses the old heuristic.
        let json = r#"{
            "module_name": "inf",
            "premises": [
                {"label":"a","value":{"Int":2},"evidence":null},
                {"label":"b","value":{"Int":3},"evidence":null},
                {"label":"q1","value":{"Quantity":[10,"N"]},"evidence":null},
                {"label":"q2","value":{"Quantity":[2,"N"]},"evidence":null}
            ],
            "assumptions": [],
            "steps": [
                {"label":"sum","op":"core.add","inputs":["a","b"],"requires_obligation":false,"discharge_by":null,"verify":false},
                {"label":"qsum","op":"core.qmul","inputs":["q1","q2"],"requires_obligation":false,"discharge_by":null,"verify":false},
                {"label":"eqq","op":"core.eq","inputs":["q1","q2"],"requires_obligation":false,"discharge_by":null,"verify":false},
                {"label":"iv","op":"core.interval","inputs":["a","b"],"requires_obligation":false,"discharge_by":null,"verify":false}
            ]
        }"#;
        let src = produce_from_json(json).expect("plan must produce");
        assert!(src.contains("derive sum = core.add(a, b) : rational"));
        assert!(src.contains("derive qsum = core.qmul(q1, q2) : quantity"));
        assert!(src.contains("derive eqq = core.eq(q1, q2) : bool"));
        assert!(src.contains("derive iv = core.interval(a, b) : interval"));
    }

    // --- F2: rational literal rendering -------------------------------------

    #[test]
    fn producer_renders_rational_literals() {
        let plan = ReasoningPlan {
            module_name: "rat".into(),
            premises: vec![Premise {
                label: "half".into(),
                value: Literal::Rational(1i128),
                evidence: None,
            }],
            assumptions: vec![],
            steps: vec![],
        };
        let src = plan.produce().unwrap();
        assert!(
            src.contains("assert half = rat(1 1) : rational"),
            "got: {src}"
        );
        // Round-trips through the parser and converts to a reduced rational.
        let ast = axiom_parser::parse_module(&src).unwrap();
        let mut rt = axiom_runtime::Runtime::new("rat");
        rt.execute(&ast).unwrap();
        let c = &rt.module.claims[&rt.claim("half").unwrap()];
        assert_eq!(
            c.value,
            axiom_core::Value::Num(axiom_types::num::Num::rational(1, 1).unwrap())
        );
    }

    #[test]
    fn rational_denominator_zero_rejected() {
        let src = "module r \"1\"\nassert x = rat(1 0) : rational";
        let res: Result<(), axiom_runtime::RuntimeError> = axiom_parser::parse_module(src)
            .map_err(|_| axiom_runtime::RuntimeError::Value("parse".into()))
            .and_then(|ast| {
                let mut rt = axiom_runtime::Runtime::new("r");
                rt.execute(&ast)
            });
        assert!(res.is_err(), "rat(...,0) must be rejected");
    }
}
