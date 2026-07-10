use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RuntimeError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("unknown label `{0}` referenced before declaration")]
    UnknownLabel(String),
    #[error("unknown evidence label `{0}`")]
    UnknownEvidenceLabel(String),
    #[error("unknown receipt label `{0}`")]
    UnknownReceiptLabel(String),
    #[error("unknown context label `{0}`")]
    UnknownContextLabel(String),
    #[error("unknown operation `{0}`")]
    UnknownOperation(String),
    #[error("core error: {0}")]
    Core(String),
    #[error("value conversion error: {0}")]
    Value(String),
    #[error("type conversion error: {0}")]
    Type(String),
    #[error("uncertainty conversion error: {0}")]
    Uncertainty(String),
    #[error("capability `{0}` is not granted to this runtime")]
    CapabilityDenied(String),
    #[error("external operation `{0}` failed: {1}")]
    External(String, String),
    #[error("replay requires receipt `{0}` but none was supplied")]
    ReplayMissingReceipt(String),
    #[error("module failed verification invariant: {0}")]
    Invariant(String),
    #[error("module too large: {0} statements exceed the execution limit")]
    ModuleTooLarge(String),
}

impl From<axiom_core::CoreError> for RuntimeError {
    fn from(e: axiom_core::CoreError) -> Self {
        RuntimeError::Core(e.to_string())
    }
}
