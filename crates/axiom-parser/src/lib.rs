//! `axiom-parser`: lexer, parser, formatter, and AST for the Axiom textual
//! language. Syntactic only — semantic analysis (label resolution, type
//! checking, execution) lives in `axiom-runtime`.

pub mod ast;
pub mod diag;
pub mod formatter;
pub mod lexer;
pub mod parser;

pub use ast::{Expr, ModuleAst, Stmt, TypeExpr, UncertaintyExpr};
pub use diag::{Diagnostic, Level, ParseResult};
pub use formatter::format_module;
pub use lexer::{render_diagnostic, Lexer, Tok, Token};
pub use parser::Parser;

/// Parse source text into an AST. Returns all diagnostics on failure.
pub fn parse_module(src: &str) -> ParseResult<ModuleAst> {
    let tokens = Lexer::new(src).lex()?;
    Parser::new(tokens).parse()
}

/// Parse source, then render canonical form (used by `axiom fmt`).
pub fn format_source(src: &str) -> ParseResult<String> {
    let ast = parse_module(src)?;
    Ok(format_module(&ast))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
module demo "1"

evidence e_src "text/plain" "temperature 21.5C" trust=trusted

assert ambient = q(21.5 "C") : quantity
assert limit = q(30.0 "C") : quantity

derive sum = qadd(ambient, limit) : quantity

verify sum
"#;

    #[test]
    fn parse_ok() {
        let ast = parse_module(SAMPLE).expect("parse");
        assert_eq!(ast.name, "demo");
        assert_eq!(ast.stmts.len(), 5);
    }

    #[test]
    fn roundtrip_and_idempotent() {
        let ast = parse_module(SAMPLE).expect("parse");
        let f1 = format_module(&ast);
        let ast2 = parse_module(&f1).expect("reparse");
        let f2 = format_module(&ast2);
        // Idempotence: formatting twice yields identical canonical text.
        assert_eq!(f1, f2);
        // Round-trip: re-parsing canonical text reproduces the same text.
        assert_eq!(f1, format_module(&parse_module(&f1).expect("reparse2")));
        let _ = ast2;
    }

    #[test]
    fn malformed_rejected() {
        let src = "module x \"1\"\nassert = : bool\n";
        assert!(parse_module(src).is_err());
    }

    #[test]
    fn deeply_nested_expression_fails_closed() {
        // Build `eq(eq(eq(...)))` with nesting far beyond the parser's depth
        // bound. Must return an Err (clean diagnostic), never overflow the stack.
        let mut inner = "0".to_string();
        for i in 1..=5000 {
            inner = format!("eq({i}, {inner})");
        }
        let src = format!("module deep \"1\"\nassert x = {} : bool\n", inner);
        assert!(
            parse_module(&src).is_err(),
            "deeply nested expr must be rejected"
        );
    }

    #[test]
    fn reasonable_nesting_accepted() {
        // Ten levels of nesting is well within the bound and must parse.
        let mut inner = "0".to_string();
        for i in 1..=10 {
            inner = format!("eq({i}, {inner})");
        }
        let src = format!("module ok \"1\"\nassert x = {} : bool\n", inner);
        match parse_module(&src) {
            Ok(_) => {}
            Err(d) => panic!(
                "parse failed: {:?}",
                d.iter().map(|x| &x.message).collect::<Vec<_>>()
            ),
        }
    }
}
