//! External operation execution. External operations are capability-gated and
//! always produce an immutable receipt. The default runtime grants no
//! capabilities, so no external side effect can occur unless explicitly
//! allowed. For replay, the runtime bypasses live execution entirely and uses
//! the captured receipt.

use crate::error::RuntimeError;
use axiom_types::num::Num;
use axiom_types::Value;
use std::collections::BTreeSet;

pub struct ExternalResult {
    pub value: Value,
    pub provider: String,
    pub logical_time: u64,
}

pub trait ExternalExecutor: Send + Sync {
    fn run(
        &self,
        op: &str,
        inputs: &[Value],
        caps: &BTreeSet<String>,
    ) -> Result<ExternalResult, RuntimeError>;
}

/// A deterministic, fixture-backed calculator used by the demonstrations. It is
/// "external" only in the sense of capability gating and receipt capture; its
/// computation is exact and reproducible, which is what makes offline replay
/// produce an identical module digest.
pub struct ToolCalculator;

impl ExternalExecutor for ToolCalculator {
    fn run(
        &self,
        op: &str,
        inputs: &[Value],
        caps: &BTreeSet<String>,
    ) -> Result<ExternalResult, RuntimeError> {
        if !caps.contains("tool:calculator") {
            return Err(RuntimeError::CapabilityDenied("tool:calculator".into()));
        }
        if op != "tool.calculator" {
            return Err(RuntimeError::External(
                op.to_string(),
                "unknown external op".into(),
            ));
        }
        let mut sum = Num::Int(0);
        for v in inputs {
            match v {
                Value::Num(n) => {
                    sum = sum
                        .checked_add(*n)
                        .map_err(|e| RuntimeError::External(op.to_string(), e.to_string()))?;
                }
                _ => {
                    return Err(RuntimeError::External(
                        op.to_string(),
                        "calculator expects numeric inputs".into(),
                    ))
                }
            }
        }
        Ok(ExternalResult {
            value: Value::Num(sum),
            provider: "fixture:calculator".into(),
            logical_time: 0,
        })
    }
}

/// A no-op executor that refuses all live external calls. Used in replay mode
/// and as the default when no executor is configured.
pub struct ReplayOnlyExecutor;

impl ExternalExecutor for ReplayOnlyExecutor {
    fn run(
        &self,
        op: &str,
        _inputs: &[Value],
        _caps: &BTreeSet<String>,
    ) -> Result<ExternalResult, RuntimeError> {
        Err(RuntimeError::ReplayMissingReceipt(op.to_string()))
    }
}
