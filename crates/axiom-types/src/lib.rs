//! `axiom-types`: the typed value model for Axiom IR.
//!
//! This crate contains no execution logic and no hashing. It defines the
//! irreducible vocabulary of values, types, units, and uncertainty used by
//! every other Axiom crate. The normative numeric path uses exact arithmetic
//! ([`Num`]) so that canonical encoding and deterministic replay never depend
//! on floating-point rounding.

pub mod error;
pub mod num;
pub mod uncertainty;
pub mod unit;
pub mod value;

pub use error::{NumError, TypeError, UncertaintyError};
pub use num::Num;
pub use uncertainty::Uncertainty;
pub use unit::{Quantity, Unit};
pub use value::{Relation, Type, Value};
