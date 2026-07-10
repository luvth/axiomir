//! Source diagnostics with spans.

use crate::ast::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: Level,
    pub message: String,
    pub span: Option<Span>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Option<Span>) -> Diagnostic {
        Diagnostic {
            level: Level::Error,
            message: message.into(),
            span,
        }
    }
    pub fn warning(message: impl Into<String>, span: Option<Span>) -> Diagnostic {
        Diagnostic {
            level: Level::Warning,
            message: message.into(),
            span,
        }
    }
}

pub type ParseResult<T> = Result<T, Vec<Diagnostic>>;
