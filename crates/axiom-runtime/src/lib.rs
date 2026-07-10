//! `axiom-runtime`: deterministic execution, external receipts, replay,
//! contradiction detection, and structural explanation for Axiom IR.

pub mod convert;
pub mod error;
pub mod explain;
pub mod external;
pub mod runtime;

pub use error::RuntimeError;
pub use explain::{DerivationInfo, Explanation, ObligationInfo};
pub use external::{ExternalExecutor, ReplayOnlyExecutor, ToolCalculator};
pub use runtime::Runtime;

/// Convenience: parse source and execute it in a default (no-capability)
/// runtime, returning the runtime for inspection.
pub fn run_source(src: &str) -> Result<Runtime, RuntimeError> {
    let ast =
        axiom_parser::parse_module(src).map_err(|d| RuntimeError::Parse(format!("{:?}", d)))?;
    let mut rt = Runtime::new("module");
    rt.execute(&ast)?;
    Ok(rt)
}
