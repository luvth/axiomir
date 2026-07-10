//! `axiom-sdk`: an ergonomic Rust facade over the Axiom runtime.
//!
//! The SDK re-exports the core vocabulary and adds a small fluent [`Builder`] so
//! that Rust programs can construct and execute Axiom Modules without writing the
//! textual language. Advanced control remains available through the re-exports.

pub use axiom_core::{
    ClaimStatus, ContradictionKind, Id, Module, ObligationState, Type, Uncertainty, Value,
};
pub use axiom_encoding::{content_id, Domain};
pub use axiom_parser::{format_source, parse_module};
pub use axiom_runtime::{run_source, Runtime};
pub use axiom_types::{num::Num, unit::Quantity, unit::Unit};

use axiom_core::Uncertainty as Unc;

/// A fluent builder for Axiom Modules.
///
/// Example:
/// ```no_run
/// use axiom_sdk::{Builder, Uncertainty};
/// let mut b = Builder::new("demo");
/// let a = b.assert_int("a", 2, Uncertainty::Exact);
/// let c = b.assert_int("c", 3, Uncertainty::Exact);
/// let out = b.derive_rational("out", "core.add", &[a, c]);
/// b.verify("out").unwrap();
/// let rt = b.build();
/// assert_eq!(rt.verified_claims().len(), 1);
/// ```
pub struct Builder {
    rt: Runtime,
}

impl Builder {
    pub fn new(name: &str) -> Builder {
        Builder {
            rt: Runtime::new(name),
        }
    }

    pub fn assert_int(&mut self, label: &str, v: i128, u: Unc) -> Id {
        self.rt
            .module
            .assert(
                label,
                Type::Rational,
                Value::Num(Num::Int(v)),
                u,
                vec![],
                None,
                None,
            )
            .unwrap()
    }

    pub fn assert_quantity(&mut self, label: &str, value: Num, unit: &str, u: Unc) -> Id {
        self.rt
            .module
            .assert(
                label,
                Type::Quantity,
                Value::Quantity(Quantity {
                    value,
                    unit: Unit::base(unit),
                }),
                u,
                vec![],
                None,
                None,
            )
            .unwrap()
    }

    pub fn assert_bool(&mut self, label: &str, b: bool, u: Unc) -> Id {
        self.rt
            .module
            .assert(label, Type::Bool, Value::Bool(b), u, vec![], None, None)
            .unwrap()
    }

    pub fn assume_bool(&mut self, label: &str, b: bool, scope: &str) -> Id {
        let (_a, c) = self
            .rt
            .module
            .assume(label, Type::Bool, Value::Bool(b), scope, None)
            .unwrap();
        c
    }

    pub fn derive_rational(&mut self, label: &str, op: &str, inputs: &[Id]) -> Id {
        self.rt
            .module
            .derive(
                op,
                "1",
                inputs,
                None,
                label,
                None,
                &axiom_core::registry::BuiltinExecutor,
            )
            .unwrap()
    }

    pub fn derive_quantity(&mut self, label: &str, op: &str, inputs: &[Id]) -> Id {
        self.rt
            .module
            .derive(
                op,
                "1",
                inputs,
                None,
                label,
                None,
                &axiom_core::registry::BuiltinExecutor,
            )
            .unwrap()
    }

    pub fn derive_bool(&mut self, label: &str, op: &str, inputs: &[Id]) -> Id {
        self.rt
            .module
            .derive(
                op,
                "1",
                inputs,
                None,
                label,
                None,
                &axiom_core::registry::BuiltinExecutor,
            )
            .unwrap()
    }

    pub fn verify(&mut self, label: &str) -> Result<(), String> {
        let id = self.rt.claim(label).map_err(|e| e.to_string())?;
        self.rt.module.verify(id).map_err(|e| e.to_string())
    }

    pub fn explain(&self, label: &str) -> Result<axiom_runtime::Explanation, String> {
        self.rt.explain(label).map_err(|e| e.to_string())
    }

    pub fn digest(&self) -> Id {
        self.rt.digest()
    }

    pub fn verified_claims(&self) -> Vec<Id> {
        self.rt.verified_claims()
    }

    pub fn build(self) -> Runtime {
        self.rt
    }
}
