use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum NumError {
    #[error("division by zero")]
    DivisionByZero,
    #[error("numeric overflow")]
    Overflow,
    #[error("cannot represent exact result; information would be lost")]
    LossOfPrecision,
    #[error("invalid numeric literal: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TypeError {
    #[error("value of type {found} is not compatible with declared type {expected}")]
    Mismatch { expected: String, found: String },
    #[error("unsigned type cannot hold negative value: {0}")]
    NegativeForUnsigned(String),
    #[error("quantity units incompatible: {0} vs {1}")]
    UnitMismatch(String, String),
    #[error("record field {0} missing or wrong type")]
    RecordField(String),
    #[error("unknown extension type {ns}:{name}@{version}")]
    UnknownExtension {
        ns: String,
        name: String,
        version: String,
    },
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum UncertaintyError {
    #[error(
        "implicit conversion between incompatible uncertainty models is forbidden: {0} vs {1}"
    )]
    Incompatible(String, String),
    #[error("uncertainty bound out of range: {0}")]
    OutOfRange(String),
    #[error("evidence weight must lie in [0,1]")]
    WeightRange,
}
