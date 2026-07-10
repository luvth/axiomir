//! Conversion from the parsed AST to core semantic values.

use crate::error::RuntimeError;
use axiom_core::{ContradictionKind, Type, Uncertainty};
use axiom_parser::ast::{Expr, TypeExpr, UncertaintyExpr};
use axiom_types::{
    num::Num,
    unit::{Quantity, Unit},
    Relation, Value,
};

pub fn convert_type(t: &TypeExpr) -> Result<Type, RuntimeError> {
    match t {
        TypeExpr::Name(n) => match n.as_str() {
            "bool" => Ok(Type::Bool),
            "int" => Ok(Type::Int { signed: true }),
            "uint" => Ok(Type::Int { signed: false }),
            "rational" => Ok(Type::Rational),
            "decimal" => Ok(Type::Decimal),
            "string" => Ok(Type::String),
            "symbol" => Ok(Type::Symbol),
            "quantity" => Ok(Type::Quantity),
            "interval" => Ok(Type::NumericInterval),
            "relation" => Ok(Type::Relation),
            "temporal" => Ok(Type::Temporal),
            other => Err(RuntimeError::Type(format!("unknown type name `{other}`"))),
        },
        TypeExpr::Record(fields) => {
            let mut out = vec![];
            for (n, fty) in fields {
                out.push((n.clone(), convert_type(fty)?));
            }
            Ok(Type::Record { fields: out })
        }
        TypeExpr::Extension { ns, name, version } => Ok(Type::Extension {
            ns: ns.clone(),
            name: name.clone(),
            version: version.clone(),
        }),
    }
}

pub fn convert_uncertainty(u: &UncertaintyExpr) -> Result<Uncertainty, RuntimeError> {
    match u {
        UncertaintyExpr::Exact => Ok(Uncertainty::Exact),
        UncertaintyExpr::Unknown => Ok(Uncertainty::Unknown),
        UncertaintyExpr::Conflicting => Ok(Uncertainty::Conflicting),
        UncertaintyExpr::Probability { lo, hi } => Ok(Uncertainty::ProbabilityInterval {
            lo: Num::parse(lo).map_err(|e| RuntimeError::Uncertainty(e.to_string()))?,
            hi: Num::parse(hi).map_err(|e| RuntimeError::Uncertainty(e.to_string()))?,
        }),
        UncertaintyExpr::Numeric { lo, hi } => Ok(Uncertainty::NumericInterval {
            lo: Num::parse(lo).map_err(|e| RuntimeError::Uncertainty(e.to_string()))?,
            hi: Num::parse(hi).map_err(|e| RuntimeError::Uncertainty(e.to_string()))?,
        }),
        UncertaintyExpr::Weight { w } => Ok(Uncertainty::EvidenceWeight {
            weight: Num::parse(w).map_err(|e| RuntimeError::Uncertainty(e.to_string()))?,
        }),
        UncertaintyExpr::ExternallyAsserted { source } => Ok(Uncertainty::ExternallyAsserted {
            source: source.clone(),
        }),
    }
}

pub fn convert_expr(e: &Expr) -> Result<Value, RuntimeError> {
    match e {
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Num(n) => Ok(Value::Num(
            Num::parse(n).map_err(|e| RuntimeError::Value(e.to_string()))?,
        )),
        Expr::Str(s) => Ok(Value::Str(s.clone())),
        Expr::Sym(s) => Ok(Value::Sym(s.clone())),
        Expr::Quantity { value, unit } => {
            let v = convert_expr(value)?;
            let num = match v {
                Value::Num(n) => n,
                _ => return Err(RuntimeError::Value("quantity value must be numeric".into())),
            };
            Ok(Value::Quantity(Quantity {
                value: num,
                unit: Unit::base(unit),
            }))
        }
        Expr::Interval { lo, hi } => Ok(Value::Interval {
            lo: num_of(convert_expr(lo)?)?,
            hi: num_of(convert_expr(hi)?)?,
        }),
        Expr::Record(fields) => {
            let mut out = vec![];
            for (n, fv) in fields {
                out.push((n.clone(), convert_expr(fv)?));
            }
            Ok(Value::Record(out))
        }
        Expr::Relation { op, args } => {
            let mut vals = vec![];
            for a in args {
                vals.push(Box::new(convert_expr(a)?));
            }
            let rel = match op.as_str() {
                "eq" => Relation::Eq(vals.remove(0), vals.remove(0)),
                "neq" => Relation::Neq(vals.remove(0), vals.remove(0)),
                "lt" => Relation::Lt(vals.remove(0), vals.remove(0)),
                "le" => Relation::Le(vals.remove(0), vals.remove(0)),
                "gt" => Relation::Gt(vals.remove(0), vals.remove(0)),
                "ge" => Relation::Ge(vals.remove(0), vals.remove(0)),
                "inset" => {
                    let elem = vals.remove(0);
                    let set: Vec<Value> = vals.into_iter().map(|b| *b).collect();
                    Relation::InSet(elem, set)
                }
                "notinset" => {
                    let elem = vals.remove(0);
                    let set: Vec<Value> = vals.into_iter().map(|b| *b).collect();
                    Relation::NotInSet(elem, set)
                }
                other => return Err(RuntimeError::Value(format!("unknown relation `{other}`"))),
            };
            Ok(Value::Relation(rel))
        }
        Expr::Ref(r) => Err(RuntimeError::Value(format!(
            "reference `{r}` is not allowed as a literal value; use it as a derivation input"
        ))),
    }
}

fn num_of(v: Value) -> Result<Num, RuntimeError> {
    match v {
        Value::Num(n) => Ok(n),
        _ => Err(RuntimeError::Value("expected numeric endpoint".into())),
    }
}

pub fn contradiction_kind(s: &str) -> ContradictionKind {
    match s {
        "proposition-negation" => ContradictionKind::PropositionNegation,
        "incompatible-equality" => ContradictionKind::IncompatibleEquality,
        "disjoint-interval" => ContradictionKind::DisjointInterval,
        "incompatible-unit" => ContradictionKind::IncompatibleUnit,
        "mutually-exclusive-membership" => ContradictionKind::MutuallyExclusiveMembership,
        "violated-postcondition" => ContradictionKind::ViolatedPostcondition,
        "evidence-conflict" => ContradictionKind::EvidenceConflict,
        "assumption-conflict" => ContradictionKind::AssumptionConflict,
        other => ContradictionKind::Extension(other.to_string()),
    }
}
