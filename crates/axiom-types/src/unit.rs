//! Unit algebra for quantity values. Units are products of base-unit powers.
//! Canonical form drops zero exponents, giving deterministic dimension identity.

use crate::error::TypeError;
use crate::num::Num;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Unit {
    /// Base unit symbol -> integer exponent. Canonical: no zero exponents.
    pub base: BTreeMap<String, i64>,
}

impl Unit {
    pub fn dimensionless() -> Unit {
        Unit::default()
    }

    pub fn base(name: &str) -> Unit {
        let mut m = BTreeMap::new();
        m.insert(name.to_string(), 1);
        Unit { base: m }
    }

    pub fn canonical(&self) -> Unit {
        let mut base = self.base.clone();
        base.retain(|_, e| *e != 0);
        Unit { base }
    }

    pub fn is_dimensionless(&self) -> bool {
        self.base.values().all(|e| *e == 0)
    }

    /// Combine two units by adding exponents (multiplication of quantities).
    pub fn multiply(&self, other: &Unit) -> Unit {
        let mut base = self.base.clone();
        for (k, e) in &other.base {
            *base.entry(k.clone()).or_insert(0) += *e;
        }
        Unit { base }.canonical()
    }

    /// Combine two units by subtracting exponents (division of quantities).
    pub fn divide(&self, other: &Unit) -> Unit {
        let mut base = self.base.clone();
        for (k, e) in &other.base {
            *base.entry(k.clone()).or_insert(0) -= *e;
        }
        Unit { base }.canonical()
    }

    /// Dimension-equality: two units may be added only when dimensions match.
    pub fn same_dimension(&self, other: &Unit) -> bool {
        self.canonical().base == other.canonical().base
    }

    pub fn display(&self) -> String {
        let u = self.canonical();
        if u.base.is_empty() {
            return "1".to_string();
        }
        let mut pos = vec![];
        let mut neg = vec![];
        for (k, e) in &u.base {
            if *e > 0 {
                pos.push((k.clone(), *e));
            } else {
                neg.push((k.clone(), -e));
            }
        }
        let mut s = String::new();
        for (i, (k, e)) in pos.iter().enumerate() {
            if i > 0 {
                s.push('*');
            }
            s.push_str(k);
            if *e > 1 {
                s.push_str(&format!("^{e}"));
            }
        }
        if !neg.is_empty() {
            s.push('/');
            for (i, (k, e)) in neg.iter().enumerate() {
                if i > 0 {
                    s.push('*');
                }
                s.push_str(k);
                if *e > 1 {
                    s.push_str(&format!("^{e}"));
                }
            }
        }
        s
    }
}

/// A numeric value carrying a unit.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Quantity {
    pub value: Num,
    pub unit: Unit,
}

impl Quantity {
    pub fn add(&self, other: &Quantity) -> Result<Quantity, TypeError> {
        if !self.unit.same_dimension(&other.unit) {
            return Err(TypeError::UnitMismatch(
                self.unit.display(),
                other.unit.display(),
            ));
        }
        Ok(Quantity {
            value: self
                .value
                .checked_add(other.value)
                .map_err(|e| TypeError::Mismatch {
                    expected: "quantity".into(),
                    found: e.to_string(),
                })?,
            unit: self.unit.clone(),
        })
    }

    pub fn sub(&self, other: &Quantity) -> Result<Quantity, TypeError> {
        if !self.unit.same_dimension(&other.unit) {
            return Err(TypeError::UnitMismatch(
                self.unit.display(),
                other.unit.display(),
            ));
        }
        Ok(Quantity {
            value: self
                .value
                .checked_sub(other.value)
                .map_err(|e| TypeError::Mismatch {
                    expected: "quantity".into(),
                    found: e.to_string(),
                })?,
            unit: self.unit.clone(),
        })
    }

    pub fn mul(&self, other: &Quantity) -> Result<Quantity, TypeError> {
        Ok(Quantity {
            value: self
                .value
                .checked_mul(other.value)
                .map_err(|e| TypeError::Mismatch {
                    expected: "quantity".into(),
                    found: e.to_string(),
                })?,
            unit: self.unit.multiply(&other.unit),
        })
    }

    pub fn div(&self, other: &Quantity) -> Result<Quantity, TypeError> {
        Ok(Quantity {
            value: self
                .value
                .checked_div(other.value)
                .map_err(|e| TypeError::Mismatch {
                    expected: "quantity".into(),
                    found: e.to_string(),
                })?,
            unit: self.unit.divide(&other.unit),
        })
    }

    pub fn display(&self) -> String {
        format!("{} {}", self.value.display(), self.unit.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_match_and_mismatch() {
        let m = Quantity {
            value: Num::Int(3),
            unit: Unit::base("m"),
        };
        let km = Quantity {
            value: Num::Int(1),
            unit: Unit::base("m"),
        };
        assert!(m.add(&km).is_ok());
        let s = Quantity {
            value: Num::Int(2),
            unit: Unit::base("s"),
        };
        assert!(m.add(&s).is_err());
    }

    #[test]
    fn multiply_units() {
        let m = Unit::base("m");
        let s = Unit::base("s");
        assert_eq!(m.multiply(&s).display(), "m*s");
        assert_eq!(m.divide(&s).display(), "m/s");
    }
}
