//! Typed value model and the initial Axiom type system.

use crate::error::TypeError;
use crate::num::Num;
use crate::unit::Quantity;
use serde::{Deserialize, Serialize};

/// Initial practical type system for Axiom claims.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Type {
    Bool,
    /// Integer; `signed=false` rejects negative values at type-check time.
    Int {
        signed: bool,
    },
    Decimal,
    Rational,
    String,
    Symbol,
    /// Unit-bearing numeric quantity.
    Quantity,
    /// Numeric interval `[lo, hi]`.
    NumericInterval,
    /// Relation (equality, inequality, membership, temporal).
    Relation,
    /// Structured record with named, typed fields.
    Record {
        fields: Vec<(String, Type)>,
    },
    /// Temporal assertion: a proposition located at a logical time.
    Temporal,
    /// Externally defined extension type, addressed by namespace/name/version.
    Extension {
        ns: String,
        name: String,
        version: String,
    },
}

impl Type {
    pub fn name(&self) -> String {
        match self {
            Type::Bool => "bool".into(),
            Type::Int { signed: true } => "int".into(),
            Type::Int { signed: false } => "uint".into(),
            Type::Decimal => "decimal".into(),
            Type::Rational => "rational".into(),
            Type::String => "string".into(),
            Type::Symbol => "symbol".into(),
            Type::Quantity => "quantity".into(),
            Type::NumericInterval => "interval".into(),
            Type::Relation => "relation".into(),
            Type::Record { .. } => "record".into(),
            Type::Temporal => "temporal".into(),
            Type::Extension { ns, name, .. } => format!("{ns}:{name}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Relation {
    Eq(Box<Value>, Box<Value>),
    Neq(Box<Value>, Box<Value>),
    Lt(Box<Value>, Box<Value>),
    Le(Box<Value>, Box<Value>),
    Gt(Box<Value>, Box<Value>),
    Ge(Box<Value>, Box<Value>),
    InSet(Box<Value>, Vec<Value>),
    NotInSet(Box<Value>, Vec<Value>),
    /// A proposition asserted at a logical time `when` (Num in abstract time units).
    TemporalAt {
        when: Num,
        claim: Box<Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    Num(Num),
    Str(String),
    Sym(String),
    Quantity(Quantity),
    Interval {
        lo: Num,
        hi: Num,
    },
    Relation(Relation),
    Record(Vec<(String, Value)>),
    /// Opaque, canonical-bytes-carrying extension value.
    Extension {
        ns: String,
        name: String,
        version: String,
        data: Vec<u8>,
    },
}

impl Value {
    /// Structural type of a value, ignoring declared types.
    pub fn structural_type(&self) -> Type {
        match self {
            Value::Bool(_) => Type::Bool,
            Value::Num(_) => Type::Rational, // bare numeric literal
            Value::Str(_) => Type::String,
            Value::Sym(_) => Type::Symbol,
            Value::Quantity(_) => Type::Quantity,
            Value::Interval { .. } => Type::NumericInterval,
            Value::Relation(_) => Type::Relation,
            Value::Record(fields) => Type::Record {
                fields: fields
                    .iter()
                    .map(|(n, v)| (n.clone(), v.structural_type()))
                    .collect(),
            },
            Value::Extension {
                ns, name, version, ..
            } => Type::Extension {
                ns: ns.clone(),
                name: name.clone(),
                version: version.clone(),
            },
        }
    }

    /// Verify this value is compatible with the declared type under the
    /// initial type system. Returns `Ok(())` or a precise [`TypeError`].
    pub fn check(&self, ty: &Type) -> Result<(), TypeError> {
        match (self, ty) {
            (Value::Bool(_), Type::Bool) => Ok(()),
            (Value::Num(n), Type::Int { signed }) => {
                if !*signed && matches!(n, Num::Int(v) if *v < 0) {
                    return Err(TypeError::NegativeForUnsigned(n.display()));
                }
                Ok(())
            }
            (Value::Num(_), Type::Decimal) => Ok(()),
            (Value::Num(_), Type::Rational) => Ok(()),
            (Value::Str(_), Type::String) => Ok(()),
            (Value::Sym(_), Type::Symbol) => Ok(()),
            (Value::Quantity(_), Type::Quantity) => Ok(()),
            (Value::Interval { .. }, Type::NumericInterval) => Ok(()),
            (Value::Relation(_), Type::Relation) => Ok(()),
            (Value::Record(fields), Type::Record { fields: ftypes }) => {
                if fields.len() != ftypes.len() {
                    return Err(TypeError::RecordField("arity".into()));
                }
                for ((fn_, v), (efn, et)) in fields.iter().zip(ftypes.iter()) {
                    if fn_ != efn {
                        return Err(TypeError::RecordField(fn_.clone()));
                    }
                    v.check(et)?;
                }
                Ok(())
            }
            (
                Value::Extension {
                    ns, name, version, ..
                },
                Type::Extension {
                    ns: ens,
                    name: en,
                    version: ev,
                },
            ) => {
                if ns != ens || name != en || version != ev {
                    return Err(TypeError::UnknownExtension {
                        ns: ns.clone(),
                        name: name.clone(),
                        version: version.clone(),
                    });
                }
                Ok(())
            }
            (v, t) => Err(TypeError::Mismatch {
                expected: t.name(),
                found: v.structural_type().name(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;

    #[test]
    fn type_check_ok_and_fail() {
        assert!(Value::Bool(true).check(&Type::Bool).is_ok());
        let q = Value::Quantity(Quantity {
            value: Num::Int(1),
            unit: Unit::base("m"),
        });
        assert!(q.check(&Type::Quantity).is_ok());
        assert!(Value::Num(Num::Int(-1))
            .check(&Type::Int { signed: false })
            .is_err());
        assert!(Value::Str("x".into()).check(&Type::Bool).is_err());
    }
}
