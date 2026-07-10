//! Canonical formatter. The output is deterministic for a given AST, so
//! `format(format(ast)) == format(ast)` (idempotent), and
//! `parse(format(ast))` reproduces a structurally equal AST (round-trip).

use crate::ast::*;

fn esc(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn fmt_expr(e: &Expr) -> String {
    match e {
        Expr::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        Expr::Num(n) => n.clone(),
        Expr::Str(s) => format!("\"{}\"", esc(s)),
        Expr::Sym(s) => format!("'{}", s),
        Expr::Quantity { value, unit } => format!("q({} \"{}\")", fmt_expr(value), esc(unit)),
        Expr::Interval { lo, hi } => format!("interval({} {})", fmt_expr(lo), fmt_expr(hi)),
        Expr::Record(fields) => {
            let inner = fields
                .iter()
                .map(|(n, v)| format!("{}: {}", n, fmt_expr(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {} }}", inner)
        }
        Expr::Relation { op, args } => {
            let inner = args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", op, inner)
        }
        Expr::Ref(r) => r.clone(),
    }
}

fn fmt_type(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Name(n) => n.clone(),
        TypeExpr::Record(fields) => {
            let inner = fields
                .iter()
                .map(|(n, ty)| format!("{}: {}", n, fmt_type(ty)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("record {{ {} }}", inner)
        }
        TypeExpr::Extension { ns, name, version } => format!("{}:{}@{}", ns, name, version),
    }
}

fn fmt_uncertainty(u: &UncertaintyExpr) -> String {
    match u {
        UncertaintyExpr::Exact => "exact".into(),
        UncertaintyExpr::Unknown => "unknown".into(),
        UncertaintyExpr::Conflicting => "conflicting".into(),
        UncertaintyExpr::Probability { lo, hi } => format!("probability({}, {})", lo, hi),
        UncertaintyExpr::Numeric { lo, hi } => format!("numeric({}, {})", lo, hi),
        UncertaintyExpr::Weight { w } => format!("weight({})", w),
        UncertaintyExpr::ExternallyAsserted { source } => format!("external(\"{}\")", esc(source)),
    }
}

fn fmt_stmt(s: &Stmt) -> String {
    match s {
        Stmt::Module { name, version, .. } => format!("module {} \"{}\"", name, esc(version)),
        Stmt::Evidence {
            label,
            media,
            content,
            trust,
            ..
        } => {
            let c = content
                .as_ref()
                .map(|x| format!(" \"{}\"", esc(x)))
                .unwrap_or_default();
            format!(
                "evidence {} \"{}\"{}{} trust={}",
                label,
                esc(media),
                c,
                "",
                trust
            )
        }
        Stmt::Assert {
            label,
            value,
            ty,
            evidence,
            uncertainty,
            ctx,
            ..
        } => fmt_assert_like("assert", label, value, ty, evidence, uncertainty, ctx, None),
        Stmt::Observe {
            label,
            value,
            ty,
            evidence,
            uncertainty,
            ..
        } => fmt_assert_like(
            "observe",
            label,
            value,
            ty,
            evidence,
            uncertainty,
            &None,
            None,
        ),
        Stmt::Assume {
            label,
            value,
            ty,
            scope,
            ctx,
            ..
        } => {
            let mut s = format!(
                "assume {} = {} : {} scope \"{}\"",
                label,
                fmt_expr(value),
                fmt_type(ty),
                esc(scope)
            );
            if let Some(c) = ctx {
                s.push_str(&format!(" ctx {}", c));
            }
            s
        }
        Stmt::Derive {
            label,
            op,
            inputs,
            ty,
            receipt,
            ctx,
            ..
        } => {
            let args = inputs.join(", ");
            let mut s = format!("derive {} = {}({}) : {}", label, op, args, fmt_type(ty));
            if let Some(r) = receipt {
                s.push_str(&format!(" receipt {}", r));
            }
            if let Some(c) = ctx {
                s.push_str(&format!(" ctx {}", c));
            }
            s
        }
        Stmt::Require { kind, target, .. } => format!("require {} on {}", kind, target),
        Stmt::Discharge {
            obligation,
            by,
            state,
            ..
        } => {
            let mut s = format!("discharge {} by {}", obligation, by);
            if let Some(st) = state {
                s.push_str(&format!(" as {}", st));
            }
            s
        }
        Stmt::Verify { target, .. } => format!("verify {}", target),
        Stmt::Challenge { target, .. } => format!("challenge {}", target),
        Stmt::Contradict { a, b, kind, .. } => format!("contradict {} {} as {}", a, b, kind),
        Stmt::Branch {
            label,
            parent,
            assumptions,
            ..
        } => {
            let mut s = format!("branch {} from {}", label, parent);
            if !assumptions.is_empty() {
                s.push_str(&format!(" with {}", assumptions.join(", ")));
            }
            s
        }
        Stmt::Merge { label, a, b, .. } => format!("merge {} = {} + {}", label, a, b),
        Stmt::Invalidate { target, reason, .. } => {
            format!("invalidate {} because \"{}\"", target, esc(reason))
        }
        Stmt::Attest { target, .. } => format!("attest {}", target),
        Stmt::Call {
            label,
            op,
            inputs,
            ty,
            capability,
            ctx,
            ..
        } => {
            let args = inputs.join(", ");
            let mut s = format!(
                "call {} = {}({}) : {} cap \"{}\"",
                label,
                op,
                args,
                fmt_type(ty),
                esc(capability)
            );
            if let Some(c) = ctx {
                s.push_str(&format!(" ctx {}", c));
            }
            s
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fmt_assert_like(
    kw: &str,
    label: &str,
    value: &Expr,
    ty: &TypeExpr,
    evidence: &[String],
    uncertainty: &Option<UncertaintyExpr>,
    ctx: &Option<String>,
    _unused: Option<()>,
) -> String {
    let mut s = format!("{} {} = {} : {}", kw, label, fmt_expr(value), fmt_type(ty));
    if !evidence.is_empty() {
        s.push_str(&format!(" evidence [{}]", evidence.join(", ")));
    }
    if let Some(u) = uncertainty {
        s.push_str(&format!(" uncertainty {}", fmt_uncertainty(u)));
    }
    if let Some(c) = ctx {
        s.push_str(&format!(" ctx {}", c));
    }
    s
}

/// Render an AST to canonical textual form.
pub fn format_module(ast: &ModuleAst) -> String {
    let mut out = String::new();
    out.push_str(&fmt_stmt(&Stmt::Module {
        name: ast.name.clone(),
        version: ast.version.clone(),
        span: crate::ast::Span { start: 0, end: 0 },
    }));
    out.push('\n');
    for s in &ast.stmts {
        out.push_str(&fmt_stmt(s));
        out.push('\n');
    }
    out
}
