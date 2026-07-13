//! Builtin operation registry and the [`OpExecutor`] trait.
//!
//! Pure operations are executed inside the trusted core via [`BuiltinExecutor`].
//! External operations are never executed here; the runtime supplies a receipt
//! and [`OpExecutor::resolve_receipt`] recovers the recorded output.

use crate::{
    DeterminismClass, ObligationKind, OpClass, OperationDef, Receipt, Type, UncertaintyRule, Value,
};
use axiom_types::num::Num;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ExecError {
    #[error("unknown operation {0}")]
    UnknownOp(String),
    #[error("value/type error: {0}")]
    Value(String),
    #[error("division by zero")]
    DivZero,
    #[error("unit mismatch: {0}")]
    Unit(String),
}

/// Computes the output value of an operation, or resolves a receipt.
pub trait OpExecutor {
    fn exec(&self, op: &OperationDef, inputs: &[Value]) -> Result<Value, ExecError>;
    fn resolve_receipt(&self, r: &Receipt) -> Result<Value, ExecError> {
        Ok(r.output.clone())
    }
}

/// The trusted executor for all builtin pure operations.
pub struct BuiltinExecutor;

fn num_of(v: &Value) -> Result<Num, ExecError> {
    match v {
        Value::Num(n) => Ok(*n),
        _ => Err(ExecError::Value("expected numeric value".into())),
    }
}

fn arith(a: &Value, b: &Value, kind: &str) -> Result<Value, ExecError> {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => {
            let r = match kind {
                "add" => x
                    .checked_add(*y)
                    .map_err(|e| ExecError::Value(e.to_string())),
                "sub" => x
                    .checked_sub(*y)
                    .map_err(|e| ExecError::Value(e.to_string())),
                "mul" => x
                    .checked_mul(*y)
                    .map_err(|e| ExecError::Value(e.to_string())),
                "div" => x.checked_div(*y).map_err(|_| ExecError::DivZero),
                other => return Err(ExecError::Value(format!("unknown arithmetic op {other}"))),
            };
            Ok(Value::Num(r.map_err(|e| ExecError::Value(e.to_string()))?))
        }
        (Value::Quantity(x), Value::Quantity(y)) => {
            let r = match kind {
                "add" => x.add(y).map_err(|e| ExecError::Unit(e.to_string())),
                "sub" => x.sub(y).map_err(|e| ExecError::Unit(e.to_string())),
                "mul" => x.mul(y).map_err(|e| ExecError::Unit(e.to_string())),
                "div" => x.div(y).map_err(|e| ExecError::Unit(e.to_string())),
                other => return Err(ExecError::Unit(format!("unknown quantity op {other}"))),
            };
            Ok(Value::Quantity(r?))
        }
        _ => Err(ExecError::Value(format!(
            "incompatible operands for {kind}"
        ))),
    }
}

fn compare(a: &Value, b: &Value, kind: &str) -> Result<Value, ExecError> {
    let ord = match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.cmp_num(y),
        (Value::Quantity(x), Value::Quantity(y)) => x.value.cmp_num(&y.value),
        _ => {
            return Err(ExecError::Value(
                "incompatible operands for comparison".into(),
            ))
        }
    };
    let res = match kind {
        "eq" => ord == std::cmp::Ordering::Equal,
        "neq" => ord != std::cmp::Ordering::Equal,
        "lt" => ord == std::cmp::Ordering::Less,
        "le" => ord != std::cmp::Ordering::Greater,
        "gt" => ord == std::cmp::Ordering::Greater,
        "ge" => ord != std::cmp::Ordering::Less,
        other => return Err(ExecError::Value(format!("unknown comparison op {other}"))),
    };
    Ok(Value::Bool(res))
}

impl OpExecutor for BuiltinExecutor {
    fn exec(&self, op: &OperationDef, inputs: &[Value]) -> Result<Value, ExecError> {
        match op.name.as_str() {
            "core.add" | "core.sub" | "core.mul" | "core.div" => {
                arith(&inputs[0], &inputs[1], &op.name["core.".len()..])
            }
            "core.qadd" | "core.qsub" | "core.qmul" | "core.qdiv" => {
                arith(&inputs[0], &inputs[1], &op.name["core.q".len()..])
            }
            "core.eq" | "core.neq" | "core.lt" | "core.le" | "core.gt" | "core.ge" => {
                compare(&inputs[0], &inputs[1], &op.name["core.".len()..])
            }
            "core.interval" => {
                let lo = num_of(&inputs[0])?;
                let hi = num_of(&inputs[1])?;
                if lo.cmp_num(&hi) == std::cmp::Ordering::Greater {
                    return Err(ExecError::Value("interval lo > hi".into()));
                }
                Ok(Value::Interval { lo, hi })
            }
            "core.not" => match &inputs[0] {
                Value::Bool(b) => Ok(Value::Bool(!*b)),
                _ => Err(ExecError::Value("not requires bool".into())),
            },
            _ => Err(ExecError::UnknownOp(op.name.clone())),
        }
    }
}

/// The standard set of builtin operations registered into every module.
pub fn builtin_operations() -> Vec<OperationDef> {
    let mut v = vec![];
    let id = |name: &str, version: &str| OperationDef::identity(name, version);
    v.push(OperationDef {
        id: id("core.add", "1"),
        name: "core.add".into(),
        version: "1".into(),
        class: OpClass::PureInternal,
        inputs: vec![Type::Rational, Type::Rational],
        output: Type::Rational,
        determinism: DeterminismClass::Deterministic,
        uncertainty_rule: UncertaintyRule::ConjoinInputs,
        generates: vec![ObligationKind::TypeCompat],
        capabilities: vec![],
    });
    v.push(OperationDef {
        id: id("core.sub", "1"),
        name: "core.sub".into(),
        version: "1".into(),
        class: OpClass::PureInternal,
        inputs: vec![Type::Rational, Type::Rational],
        output: Type::Rational,
        determinism: DeterminismClass::Deterministic,
        uncertainty_rule: UncertaintyRule::ConjoinInputs,
        generates: vec![ObligationKind::TypeCompat],
        capabilities: vec![],
    });
    v.push(OperationDef {
        id: id("core.mul", "1"),
        name: "core.mul".into(),
        version: "1".into(),
        class: OpClass::PureInternal,
        inputs: vec![Type::Rational, Type::Rational],
        output: Type::Rational,
        determinism: DeterminismClass::Deterministic,
        uncertainty_rule: UncertaintyRule::ConjoinInputs,
        generates: vec![ObligationKind::TypeCompat],
        capabilities: vec![],
    });
    v.push(OperationDef {
        id: id("core.div", "1"),
        name: "core.div".into(),
        version: "1".into(),
        class: OpClass::PureInternal,
        inputs: vec![Type::Rational, Type::Rational],
        output: Type::Rational,
        determinism: DeterminismClass::Deterministic,
        uncertainty_rule: UncertaintyRule::ConjoinInputs,
        // Division carries a mandatory, non-auto-satisfied numeric-bounds proof
        // obligation (division-by-zero / range). It is the canonical example of
        // a *risky* pure op: a bare `verify` MUST be quarantined until the
        // obligation is explicitly discharged (spec §6.6). This is what makes
        // obligation-gated verification fundamental rather than decorative.
        generates: vec![ObligationKind::TypeCompat, ObligationKind::NumericBounds],
        capabilities: vec![],
    });
    // Quantity arithmetic.
    for (name, kind, gens) in [
        (
            "core.qadd",
            "qadd",
            vec![
                ObligationKind::TypeCompat,
                ObligationKind::DimensionalConsistency,
            ],
        ),
        (
            "core.qsub",
            "qsub",
            vec![
                ObligationKind::TypeCompat,
                ObligationKind::DimensionalConsistency,
            ],
        ),
        (
            "core.qmul",
            "qmul",
            vec![
                ObligationKind::TypeCompat,
                ObligationKind::DimensionalConsistency,
            ],
        ),
        (
            "core.qdiv",
            "qdiv",
            vec![
                ObligationKind::TypeCompat,
                ObligationKind::DimensionalConsistency,
            ],
        ),
    ] {
        let _ = kind;
        v.push(OperationDef {
            id: id(name, "1"),
            name: name.into(),
            version: "1".into(),
            class: OpClass::PureInternal,
            inputs: vec![Type::Quantity, Type::Quantity],
            output: Type::Quantity,
            determinism: DeterminismClass::Deterministic,
            uncertainty_rule: UncertaintyRule::ConjoinInputs,
            generates: gens,
            capabilities: vec![],
        });
    }
    // Comparisons (operate on quantities; dimensionless quantities use unit "1").
    for name in [
        "core.eq", "core.neq", "core.lt", "core.le", "core.gt", "core.ge",
    ] {
        v.push(OperationDef {
            id: id(name, "1"),
            name: name.into(),
            version: "1".into(),
            class: OpClass::PureInternal,
            inputs: vec![Type::Quantity, Type::Quantity],
            output: Type::Bool,
            determinism: DeterminismClass::Deterministic,
            uncertainty_rule: UncertaintyRule::Exact,
            generates: vec![ObligationKind::TypeCompat],
            capabilities: vec![],
        });
    }
    // Interval construction.
    v.push(OperationDef {
        id: id("core.interval", "1"),
        name: "core.interval".into(),
        version: "1".into(),
        class: OpClass::PureInternal,
        inputs: vec![Type::Rational, Type::Rational],
        output: Type::NumericInterval,
        determinism: DeterminismClass::Deterministic,
        uncertainty_rule: UncertaintyRule::Exact,
        generates: vec![ObligationKind::TypeCompat],
        capabilities: vec![],
    });
    // Logical not.
    v.push(OperationDef {
        id: id("core.not", "1"),
        name: "core.not".into(),
        version: "1".into(),
        class: OpClass::PureInternal,
        inputs: vec![Type::Bool],
        output: Type::Bool,
        determinism: DeterminismClass::Deterministic,
        uncertainty_rule: UncertaintyRule::Exact,
        generates: vec![ObligationKind::TypeCompat],
        capabilities: vec![],
    });
    v
}

/// Convenience registry type.
pub type OpRegistry = std::collections::BTreeMap<String, OperationDef>;
