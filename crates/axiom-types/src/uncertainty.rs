//! Typed uncertainty algebra.
//!
//! Axiom deliberately does **not** use a universal `confidence: f64`.
//! Instead, uncertainty is one of a small, explicitly scoped set of models.
//! Implicit conversion between incompatible models is forbidden: attempting
//! to combine two incompatible models returns [`UncertaintyError::Incompatible`].

use crate::error::UncertaintyError;
use crate::num::Num;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Uncertainty {
    /// Known with certainty under the current receipts and obligations.
    Exact,
    /// No uncertainty information is available.
    Unknown,
    /// Subjective probability bounds in `[0,1]` (independence assumed on combine).
    ProbabilityInterval { lo: Num, hi: Num },
    /// Numeric bounds `[lo, hi]` on a quantity.
    NumericInterval { lo: Num, hi: Num },
    /// Normalized support weight in `[0,1]` from evidence.
    EvidenceWeight { weight: Num },
    /// Known to be in conflict with a preserved witness.
    Conflicting,
    /// Asserted by an external authority; carries no internal algebra.
    ExternallyAsserted { source: String },
    /// Extension-defined uncertainty model, addressed by namespace/name/version.
    Extension {
        ns: String,
        name: String,
        version: String,
        data: Vec<u8>,
    },
}

impl Uncertainty {
    pub fn kind(&self) -> &'static str {
        match self {
            Uncertainty::Exact => "exact",
            Uncertainty::Unknown => "unknown",
            Uncertainty::ProbabilityInterval { .. } => "probability",
            Uncertainty::NumericInterval { .. } => "numeric-interval",
            Uncertainty::EvidenceWeight { .. } => "evidence-weight",
            Uncertainty::Conflicting => "conflicting",
            Uncertainty::ExternallyAsserted { .. } => "externally-asserted",
            Uncertainty::Extension { .. } => "extension",
        }
    }

    fn bounds_ok(lo: &Num, hi: &Num) -> Result<(), UncertaintyError> {
        if lo.cmp_num(hi) == std::cmp::Ordering::Greater {
            return Err(UncertaintyError::OutOfRange("lo > hi".into()));
        }
        for b in [lo, hi] {
            if b.cmp_num(&Num::Int(0)) == std::cmp::Ordering::Less
                || b.cmp_num(&Num::Int(1)) == std::cmp::Ordering::Greater
            {
                return Err(UncertaintyError::OutOfRange("bound outside [0,1]".into()));
            }
        }
        Ok(())
    }

    /// Combine two uncertainties under a *conjunction* (both must hold).
    ///
    /// Rules are specified, not universal. Incompatible models are rejected
    /// rather than silently approximated.
    pub fn combine(&self, other: &Uncertainty) -> Result<Uncertainty, UncertaintyError> {
        use Uncertainty::*;
        match (self, other) {
            // Exact is the identity for conjunction.
            (Exact, u) | (u, Exact) => Ok(u.clone()),
            // Unknown dominates: nothing can be inferred.
            (Unknown, _) | (_, Unknown) => Ok(Unknown),
            // Conflicting dominates.
            (Conflicting, _) | (_, Conflicting) => Ok(Conflicting),
            // External assertion dominates (we do not recompute it).
            (ExternallyAsserted { .. }, u) | (u, ExternallyAsserted { .. }) => Ok(u.clone()),

            (ProbabilityInterval { lo: a, hi: b }, ProbabilityInterval { lo: c, hi: d }) => {
                Self::bounds_ok(a, b)?;
                Self::bounds_ok(c, d)?;
                Ok(ProbabilityInterval {
                    lo: a
                        .checked_mul(*c)
                        .map_err(|_| UncertaintyError::OutOfRange("mul".into()))?,
                    hi: b
                        .checked_mul(*d)
                        .map_err(|_| UncertaintyError::OutOfRange("mul".into()))?,
                })
            }
            (EvidenceWeight { weight: a }, EvidenceWeight { weight: b }) => {
                let w = a
                    .checked_mul(*b)
                    .map_err(|_| UncertaintyError::WeightRange)?;
                if w.cmp_num(&Num::Int(1)) == std::cmp::Ordering::Greater {
                    return Err(UncertaintyError::WeightRange);
                }
                Ok(EvidenceWeight { weight: w })
            }
            (NumericInterval { lo: a, hi: b }, NumericInterval { lo: c, hi: d }) => {
                let new_lo = if a.cmp_num(c) == std::cmp::Ordering::Greater {
                    *a
                } else {
                    *c
                };
                let new_hi = if b.cmp_num(d) == std::cmp::Ordering::Less {
                    *b
                } else {
                    *d
                };
                if new_lo.cmp_num(&new_hi) == std::cmp::Ordering::Greater {
                    return Ok(Conflicting);
                }
                Ok(NumericInterval {
                    lo: new_lo,
                    hi: new_hi,
                })
            }
            (
                Extension {
                    ns, name, version, ..
                },
                _,
            )
            | (
                _,
                Extension {
                    ns, name, version, ..
                },
            ) => Err(UncertaintyError::Incompatible(
                self.kind().into(),
                format!("extension:{ns}:{name}@{version}"),
            )),
            // Any remaining cross-model pair is unsupported and rejected.
            (a, b) => Err(UncertaintyError::Incompatible(
                a.kind().into(),
                b.kind().into(),
            )),
        }
    }

    /// Whether two uncertainty models may be combined at all.
    pub fn compatible_with(&self, other: &Uncertainty) -> bool {
        self.combine(other).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probability_conjunction() {
        let a = Uncertainty::ProbabilityInterval {
            lo: Num::rational(1, 2).unwrap(),
            hi: Num::Int(1),
        };
        let b = Uncertainty::ProbabilityInterval {
            lo: Num::rational(1, 2).unwrap(),
            hi: Num::Int(1),
        };
        let c = a.combine(&b).unwrap();
        assert_eq!(
            c,
            Uncertainty::ProbabilityInterval {
                lo: Num::rational(1, 4).unwrap(),
                hi: Num::Int(1)
            }
        );
    }

    #[test]
    fn incompatible_is_rejected() {
        let a = Uncertainty::ProbabilityInterval {
            lo: Num::rational(1, 2).unwrap(),
            hi: Num::Int(1),
        };
        let b = Uncertainty::NumericInterval {
            lo: Num::Int(0),
            hi: Num::Int(10),
        };
        assert!(a.combine(&b).is_err());
    }

    #[test]
    fn disjoint_intervals_conflict() {
        let a = Uncertainty::NumericInterval {
            lo: Num::Int(0),
            hi: Num::Int(1),
        };
        let b = Uncertainty::NumericInterval {
            lo: Num::Int(2),
            hi: Num::Int(3),
        };
        assert_eq!(a.combine(&b).unwrap(), Uncertainty::Conflicting);
    }
}
