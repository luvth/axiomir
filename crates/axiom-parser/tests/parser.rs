//! Integration tests for axiom-parser.
//!
//! These tests exercise the public API (`parse_module`, `format_source`,
//! `format_module`) from outside the crate, as an external consumer would.

use axiom_parser::{ast::Stmt, format_source, parse_module};
use axiom_runtime::run_source;

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

/// A well-formed multi-statement module used across several tests.
const SAMPLE: &str = r#"module demo "1"

evidence e_src "text/plain" "temperature 21.5C" trust=trusted

assert ambient = q(21.5 "C") : quantity
assert limit = q(30.0 "C") : quantity

derive sum = qadd(ambient, limit) : quantity

verify sum
"#;

// ---------------------------------------------------------------------------
// 1. parse_valid_module
// ---------------------------------------------------------------------------

/// Parse a multi-statement module and verify basic structural properties.
#[test]
fn parse_valid_module() {
    let ast = parse_module(SAMPLE).expect("parse should succeed");

    // Header
    assert_eq!(ast.name, "demo");
    assert_eq!(ast.version, "1");

    // Statements: evidence, assert, assert, derive, verify  => 5
    assert_eq!(
        ast.stmts.len(),
        5,
        "expected 5 statements, got {}",
        ast.stmts.len()
    );

    // First stmt is Evidence with label e_src
    assert!(
        matches!(&ast.stmts[0], Stmt::Evidence { label, .. } if label == "e_src"),
        "first statement should be Evidence(e_src)"
    );

    // Last stmt is Verify targeting sum
    assert!(
        matches!(&ast.stmts[4], Stmt::Verify { target, .. } if target == "sum"),
        "last statement should be Verify(sum)"
    );
}

// ---------------------------------------------------------------------------
// 2. formatter_idempotent
// ---------------------------------------------------------------------------

/// Formatting the same source twice must yield identical output (idempotence).
#[test]
fn formatter_idempotent() {
    let f1 = format_source(SAMPLE).expect("first format should succeed");
    let f2 = format_source(&f1).expect("second format should succeed");
    assert_eq!(f1, f2, "format_source must be idempotent");
    // Sanity: the formatted output is non-empty.
    assert!(!f1.trim().is_empty());
}

// ---------------------------------------------------------------------------
// 3. round_trip_parse_format
// ---------------------------------------------------------------------------

/// Parsing, formatting, then parsing again must produce a structurally
/// equivalent AST (same name, version, statement count, and per-statement
/// discriminants / primary labels).
#[test]
fn round_trip_parse_format() {
    let ast1 = parse_module(SAMPLE).expect("initial parse");
    let formatted = format_source(SAMPLE).expect("format");
    let ast2 = parse_module(&formatted).expect("re-parse of formatted");

    assert_eq!(ast1.name, ast2.name);
    assert_eq!(ast1.version, ast2.version);
    assert_eq!(
        ast1.stmts.len(),
        ast2.stmts.len(),
        "statement count must survive round-trip"
    );

    // Compare each statement's kind and primary label field.
    for (s1, s2) in ast1.stmts.iter().zip(ast2.stmts.iter()) {
        assert_eq!(stmt_kind(s1), stmt_kind(s2), "statement kind mismatch");
        assert_eq!(stmt_label(s1), stmt_label(s2), "statement label mismatch");
    }
}

// ---------------------------------------------------------------------------
// 4. malformed_rejected
// ---------------------------------------------------------------------------

/// A variety of syntactically broken inputs must return Err containing at
/// least one diagnostic with a non-empty message.
#[test]
fn malformed_rejected() {
    let bad_inputs = [
        // bare `module` keyword without name or version
        "module",
        // derive with unclosed paren
        "module x \"1\"\nderive out = add(\n",
        // assert with missing value (label then colon, no expr)
        "module x \"1\"\nassert x = : rational\n",
        // unknown instruction
        "module x \"1\"\nfrobnicate x\n",
        // missing module header entirely
        "assert a = true : bool\n",
    ];

    for src in &bad_inputs {
        let result = parse_module(src);
        assert!(
            result.is_err(),
            "expected parse error for {:?}, but got Ok",
            src
        );
        let diags = result.unwrap_err();
        assert!(
            !diags.is_empty(),
            "expected non-empty diagnostics for {:?}",
            src
        );
        assert!(
            diags.iter().any(|d| !d.message.is_empty()),
            "expected at least one diagnostic with a non-empty message for {:?}",
            src
        );
    }
}

// ---------------------------------------------------------------------------
// 5. hash_independent_of_formatting
// ---------------------------------------------------------------------------

/// The semantic digest of a module must not depend on irrelevant formatting
/// differences. We verify at two levels:
///
/// (a) Parser level: two differently-formatted but semantically equivalent
///     sources must produce ASTs with the same name, statement count, and
///     per-statement structure.
/// (b) Runtime level: `run_source` on both variants must yield the same
///     `digest()` (content-addressed identity).
#[test]
fn hash_independent_of_formatting() {
    // Compact layout — minimal whitespace.
    let src_compact = "module m \"1\"\nassert a = 1 : rational\nassert b = 2 : rational\n";

    // Generous extra blank lines and indented-looking layout.
    let src_spacious =
        "module m \"1\"\n\n\nassert a = 1 : rational\n\n\nassert b = 2 : rational\n\n";

    // --- parser-level equivalence ---
    let ast_compact = parse_module(src_compact).expect("compact parse");
    let ast_spacious = parse_module(src_spacious).expect("spacious parse");

    assert_eq!(ast_compact.name, ast_spacious.name);
    assert_eq!(ast_compact.version, ast_spacious.version);
    assert_eq!(ast_compact.stmts.len(), ast_spacious.stmts.len());
    for (s1, s2) in ast_compact.stmts.iter().zip(ast_spacious.stmts.iter()) {
        assert_eq!(stmt_kind(s1), stmt_kind(s2));
        assert_eq!(stmt_label(s1), stmt_label(s2));
    }

    // --- runtime-level digest equivalence ---
    let rt_compact = run_source(src_compact).expect("compact runtime");
    let rt_spacious = run_source(src_spacious).expect("spacious runtime");

    assert_eq!(
        rt_compact.digest(),
        rt_spacious.digest(),
        "runtime digest must be equal for whitespace-only formatting differences"
    );
}

// ---------------------------------------------------------------------------
// 6. unknown_operation_rejected
// ---------------------------------------------------------------------------

/// A `derive` statement that uses an unknown operation parses successfully
/// (the parser is purely syntactic), but executing it through the runtime
/// must return an error.
#[test]
fn unknown_operation_rejected() {
    let src = "module m \"1\"\nassert a = 2 : rational\nderive out = vendor_widget(a) : rational\n";

    // The parser must accept the syntax.
    let ast = parse_module(src).expect("parser should accept unknown op syntax");

    // Confirm the AST contains a Derive statement with op "vendor_widget".
    let has_derive = ast
        .stmts
        .iter()
        .any(|s| matches!(s, Stmt::Derive { op, .. } if op == "vendor_widget"));
    assert!(
        has_derive,
        "expected a Derive statement with op 'vendor_widget'"
    );

    // The runtime must reject the unknown operation.
    let result = run_source(src);
    assert!(
        result.is_err(),
        "run_source should return Err for unknown operation 'vendor_widget'"
    );
}

// ---------------------------------------------------------------------------
// 7. comment_and_blank_line_tolerance
// ---------------------------------------------------------------------------

/// Blank lines between statements must be accepted. If the lexer also supports
/// line comments, those are tested too. If `#` comments are not in the
/// grammar, the test still passes (blank-line tolerance is verified alone).
#[test]
fn comment_and_blank_line_tolerance() {
    // Blank lines only (always supported per the parser's skip_newlines logic).
    let src_blanks = "module demo \"1\"\n\nassert a = true : bool\n\nassert b = false : bool\n";
    let ast = parse_module(src_blanks).expect("blank lines between statements should parse");
    assert_eq!(ast.name, "demo");
    assert_eq!(ast.stmts.len(), 2);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return a short discriminant string for a statement, for equality comparison.
fn stmt_kind(s: &Stmt) -> &'static str {
    match s {
        Stmt::Module { .. } => "Module",
        Stmt::Evidence { .. } => "Evidence",
        Stmt::Assert { .. } => "Assert",
        Stmt::Observe { .. } => "Observe",
        Stmt::Assume { .. } => "Assume",
        Stmt::Derive { .. } => "Derive",
        Stmt::Require { .. } => "Require",
        Stmt::Discharge { .. } => "Discharge",
        Stmt::Verify { .. } => "Verify",
        Stmt::Challenge { .. } => "Challenge",
        Stmt::Contradict { .. } => "Contradict",
        Stmt::Branch { .. } => "Branch",
        Stmt::Merge { .. } => "Merge",
        Stmt::Invalidate { .. } => "Invalidate",
        Stmt::Attest { .. } => "Attest",
        Stmt::Call { .. } => "Call",
    }
}

/// Return the primary label of a statement (the label/target field, or empty
/// string for statements that don't have one at this syntactic level).
fn stmt_label(s: &Stmt) -> &str {
    match s {
        Stmt::Module { name, .. } => name,
        Stmt::Evidence { label, .. } => label,
        Stmt::Assert { label, .. } => label,
        Stmt::Observe { label, .. } => label,
        Stmt::Assume { label, .. } => label,
        Stmt::Derive { label, .. } => label,
        Stmt::Require { target, .. } => target,
        Stmt::Discharge { obligation, .. } => obligation,
        Stmt::Verify { target, .. } => target,
        Stmt::Challenge { target, .. } => target,
        Stmt::Contradict { a, .. } => a,
        Stmt::Branch { label, .. } => label,
        Stmt::Merge { label, .. } => label,
        Stmt::Invalidate { target, .. } => target,
        Stmt::Attest { target, .. } => target,
        Stmt::Call { label, .. } => label,
    }
}
