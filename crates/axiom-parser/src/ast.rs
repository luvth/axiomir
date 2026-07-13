//! Abstract syntax tree for the Axiom textual language.
//!
//! The parser is *syntactic*: it does not resolve label references or check
//! types. Reference resolution and semantic analysis happen in the runtime.
//! This keeps the grammar free of model-specific heuristics.

/// Byte-offset span within the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TypeExpr {
    Name(String),
    Record(Vec<(String, TypeExpr)>),
    Extension {
        ns: String,
        name: String,
        version: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UncertaintyExpr {
    Exact,
    Unknown,
    Probability { lo: String, hi: String },
    Numeric { lo: String, hi: String },
    Weight { w: String },
    Conflicting,
    ExternallyAsserted { source: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    Bool(bool),
    /// Raw numeric literal text (int / decimal / rational). Interpreted later.
    Num(String),
    Str(String),
    Sym(String),
    Quantity {
        value: Box<Expr>,
        unit: String,
    },
    /// Exact rational literal `rat(num, den)` with `den != 0`. Rendered and
    /// parsed canonically; converts to a reduced `Value::Num(Num::Rational)`.
    RationalLit {
        num: String,
        den: String,
    },
    Interval {
        lo: Box<Expr>,
        hi: Box<Expr>,
    },
    Record(Vec<(String, Expr)>),
    Relation {
        op: String,
        args: Vec<Expr>,
    },
    /// A reference to a previously declared label.
    Ref(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Stmt {
    Module {
        name: String,
        version: String,
        span: Span,
    },
    Evidence {
        label: String,
        media: String,
        content: Option<String>,
        /// Trust tier (`trusted` | `unverified` | `untrusted`).
        trust: String,
        /// Authority that vouches for this evidence node. Distinct from
        /// `media` (serialization) and `locator` (which artifact). Required
        /// when `trust = trusted`; used as the HMAC namespace for `signature`.
        provider: Option<String>,
        /// Hex-encoded HMAC-SHA256 tag over the canonical evidence binding,
        /// under the trusted secret configured for `provider`. Present and
        /// verified when `trust = trusted`; absent otherwise.
        signature: Option<String>,
        span: Span,
    },
    Assert {
        label: String,
        value: Expr,
        ty: TypeExpr,
        evidence: Vec<String>,
        uncertainty: Option<UncertaintyExpr>,
        ctx: Option<String>,
        span: Span,
    },
    Observe {
        label: String,
        value: Expr,
        ty: TypeExpr,
        evidence: Vec<String>,
        uncertainty: Option<UncertaintyExpr>,
        span: Span,
    },
    Assume {
        label: String,
        value: Expr,
        ty: TypeExpr,
        scope: String,
        ctx: Option<String>,
        span: Span,
    },
    Derive {
        label: String,
        op: String,
        inputs: Vec<String>,
        ty: TypeExpr,
        receipt: Option<String>,
        ctx: Option<String>,
        span: Span,
    },
    Require {
        kind: String,
        target: String,
        span: Span,
    },
    Discharge {
        obligation: String,
        by: String,
        state: Option<String>,
        span: Span,
    },
    Verify {
        target: String,
        span: Span,
    },
    Challenge {
        target: String,
        span: Span,
    },
    Contradict {
        a: String,
        b: String,
        kind: String,
        span: Span,
    },
    Branch {
        label: String,
        parent: String,
        assumptions: Vec<String>,
        span: Span,
    },
    Merge {
        label: String,
        a: String,
        b: String,
        span: Span,
    },
    Invalidate {
        target: String,
        reason: String,
        span: Span,
    },
    Attest {
        target: String,
        span: Span,
    },
    Call {
        label: String,
        op: String,
        inputs: Vec<String>,
        ty: TypeExpr,
        capability: String,
        ctx: Option<String>,
        span: Span,
    },
}

/// A parsed module: header plus ordered statements.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModuleAst {
    pub name: String,
    pub version: String,
    pub stmts: Vec<Stmt>,
}
