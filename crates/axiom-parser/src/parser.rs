//! Recursive-descent parser for the Axiom textual language.
//!
//! Statements are line-oriented: each instruction occupies one line and is
//! terminated by a newline. No model-specific heuristics are used; the grammar
//! is fully syntactic.

use crate::ast::*;
use crate::diag::{Diagnostic, ParseResult};
use crate::lexer::{Tok, Token};

/// Maximum nesting depth for expressions/records. A hostile module can nest
/// `q(q(q(...)))` or `{{...}}` arbitrarily; beyond this bound the parser fails
/// closed with a diagnostic instead of risking a call-stack overflow.
const MAX_EXPR_DEPTH: usize = 256;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Parser {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Tok {
        &self.tokens[self.pos.min(self.tokens.len() - 1)].kind
    }

    fn peek_token(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos.min(self.tokens.len() - 1)].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at_end(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }

    fn is_newline(&self) -> bool {
        matches!(self.peek(), Tok::Newline)
    }

    fn skip_newlines(&mut self) {
        while self.is_newline() {
            self.advance();
        }
    }

    fn expect(&mut self, kind: &Tok) -> ParseResult<Token> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(kind) {
            Ok(self.advance())
        } else {
            Err(vec![Diagnostic::error(
                format!("expected {:?}, found {:?}", kind, self.peek()),
                Some(self.peek_token().span),
            )])
        }
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        match self.peek().clone() {
            Tok::Ident(s) => {
                self.advance();
                Ok(s)
            }
            other => Err(vec![Diagnostic::error(
                format!("expected identifier, found {:?}", other),
                Some(self.peek_token().span),
            )]),
        }
    }

    fn expect_num(&mut self) -> ParseResult<String> {
        match self.peek().clone() {
            Tok::Num(s) => {
                self.advance();
                Ok(s)
            }
            other => Err(vec![Diagnostic::error(
                format!("expected number, found {:?}", other),
                Some(self.peek_token().span),
            )]),
        }
    }

    fn expect_str(&mut self) -> ParseResult<String> {
        match self.peek().clone() {
            Tok::Str(s) => {
                self.advance();
                Ok(s)
            }
            other => Err(vec![Diagnostic::error(
                format!("expected string, found {:?}", other),
                Some(self.peek_token().span),
            )]),
        }
    }

    pub fn parse(mut self) -> ParseResult<ModuleAst> {
        self.skip_newlines();
        // Header.
        if !matches!(self.peek(), Tok::Ident(s) if s == "module") {
            return Err(vec![Diagnostic::error(
                "module must begin with `module <name> \"version\"`",
                None,
            )]);
        }
        self.advance();
        let name = self.expect_ident()?;
        let version = self.expect_str()?;
        self.skip_newlines();

        let mut stmts = vec![];
        while !self.at_end() {
            self.skip_newlines();
            if self.at_end() {
                break;
            }
            let stmt = self.parse_stmt()?;
            stmts.push(stmt);
            // Consume the trailing newline(s).
            self.skip_newlines();
        }
        Ok(ModuleAst {
            name,
            version,
            stmts,
        })
    }

    fn parse_stmt(&mut self) -> ParseResult<Stmt> {
        let start = self.peek_token().span.start;
        let kw = self.expect_ident()?;
        let stmt = match kw.as_str() {
            "evidence" => self.parse_evidence()?,
            "assert" => self.parse_assert_like(StmtKind::Assert)?,
            "observe" => self.parse_assert_like(StmtKind::Observe)?,
            "assume" => self.parse_assume()?,
            "derive" => self.parse_derive()?,
            "require" => self.parse_require()?,
            "discharge" => self.parse_discharge()?,
            "verify" => {
                let target = self.expect_ident()?;
                Stmt::Verify {
                    target,
                    span: self.span(start),
                }
            }
            "challenge" => {
                let target = self.expect_ident()?;
                Stmt::Challenge {
                    target,
                    span: self.span(start),
                }
            }
            "contradict" => self.parse_contradict(start)?,
            "branch" => self.parse_branch(start)?,
            "merge" => self.parse_merge(start)?,
            "invalidate" => self.parse_invalidate(start)?,
            "attest" => {
                let target = self.expect_ident()?;
                Stmt::Attest {
                    target,
                    span: self.span(start),
                }
            }
            "call" => self.parse_call(start)?,
            other => {
                return Err(vec![Diagnostic::error(
                    format!("unknown instruction `{}`", other),
                    Some(self.peek_token().span),
                )])
            }
        };
        Ok(stmt)
    }

    fn span(&self, start: usize) -> Span {
        let end = self.tokens[self.pos.min(self.tokens.len() - 1)].span.end;
        Span {
            start,
            end: end.max(start),
        }
    }

    fn parse_evidence(&mut self) -> ParseResult<Stmt> {
        let start = self.peek_token().span.start;
        let label = self.expect_ident()?;
        let media = self.expect_str()?;
        let content = if matches!(self.peek(), Tok::Str(_)) {
            Some(self.expect_str()?)
        } else {
            None
        };
        let mut trust = "unverified".to_string();
        let mut provider = None;
        let mut signature = None;
        // Optional trailing clauses (order-independent):
        //   trust = <tier>
        //   provider = "<ident>"
        //   signature = "<hex>"
        while matches!(
            self.peek(),
            Tok::Ident(s) if s == "trust" || s == "provider" || s == "signature"
        ) {
            let clause = self.expect_ident()?;
            self.expect(&Tok::Equals)?;
            match clause.as_str() {
                "trust" => trust = self.expect_ident()?,
                "provider" => provider = Some(self.expect_str()?),
                "signature" => signature = Some(self.expect_str()?),
                // Unreachable: the while condition already restricts the set.
                _ => unreachable!(),
            }
        }
        Ok(Stmt::Evidence {
            label,
            media,
            content,
            trust,
            provider,
            signature,
            span: self.span(start),
        })
    }

    fn parse_assert_like(&mut self, kind: StmtKind) -> ParseResult<Stmt> {
        let start = self.peek_token().span.start;
        let label = self.expect_ident()?;
        self.expect(&Tok::Equals)?;
        let value = self.parse_expr(0)?;
        self.expect(&Tok::Colon)?;
        let ty = self.parse_type()?;
        let mut evidence = vec![];
        let mut uncertainty = None;
        let mut ctx = None;
        // Optional trailing clauses (order-independent).
        while matches!(self.peek(), Tok::Ident(s) if s == "evidence" || s == "uncertainty" || s == "ctx")
        {
            let clause = self.expect_ident()?;
            match clause.as_str() {
                "evidence" => {
                    self.expect(&Tok::LBracket)?;
                    loop {
                        if matches!(self.peek(), Tok::RBracket) {
                            break;
                        }
                        evidence.push(self.expect_ident()?);
                        if matches!(self.peek(), Tok::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.expect(&Tok::RBracket)?;
                }
                "uncertainty" => {
                    uncertainty = Some(self.parse_uncertainty()?);
                }
                "ctx" => {
                    ctx = Some(self.expect_ident()?);
                }
                _ => {
                    return Err(vec![Diagnostic::error(
                        format!("unexpected clause `{:?}`", clause),
                        Some(self.peek_token().span),
                    )])
                }
            }
        }
        let span = self.span(start);
        Ok(match kind {
            StmtKind::Assert => Stmt::Assert {
                label,
                value,
                ty,
                evidence,
                uncertainty,
                ctx,
                span,
            },
            StmtKind::Observe => Stmt::Observe {
                label,
                value,
                ty,
                evidence,
                uncertainty,
                span,
            },
        })
    }

    fn parse_assume(&mut self) -> ParseResult<Stmt> {
        let start = self.peek_token().span.start;
        let label = self.expect_ident()?;
        self.expect(&Tok::Equals)?;
        let value = self.parse_expr(0)?;
        self.expect(&Tok::Colon)?;
        let ty = self.parse_type()?;
        let mut scope = "default".to_string();
        let mut ctx = None;
        while matches!(self.peek(), Tok::Ident(s) if s == "scope" || s == "ctx") {
            let clause = self.expect_ident()?;
            match clause.as_str() {
                "scope" => scope = self.expect_str()?,
                "ctx" => ctx = Some(self.expect_ident()?),
                _ => {
                    return Err(vec![Diagnostic::error(
                        format!("unexpected clause `{clause}`"),
                        Some(self.peek_token().span),
                    )])
                }
            }
        }
        Ok(Stmt::Assume {
            label,
            value,
            ty,
            scope,
            ctx,
            span: self.span(start),
        })
    }

    fn parse_derive(&mut self) -> ParseResult<Stmt> {
        let start = self.peek_token().span.start;
        let label = self.expect_ident()?;
        self.expect(&Tok::Equals)?;
        let op = self.expect_ident()?;
        self.expect(&Tok::LParen)?;
        let inputs = self.parse_ident_list()?;
        self.expect(&Tok::RParen)?;
        self.expect(&Tok::Colon)?;
        let ty = self.parse_type()?;
        let receipt = if matches!(self.peek(), Tok::Ident(s) if s == "receipt") {
            self.advance();
            Some(self.expect_ident()?)
        } else {
            None
        };
        let ctx = if matches!(self.peek(), Tok::Ident(s) if s == "ctx") {
            self.advance();
            Some(self.expect_ident()?)
        } else {
            None
        };
        Ok(Stmt::Derive {
            label,
            op,
            inputs,
            ty,
            receipt,
            ctx,
            span: self.span(start),
        })
    }

    fn parse_require(&mut self) -> ParseResult<Stmt> {
        let start = self.peek_token().span.start;
        let kind = self.expect_ident()?;
        if !matches!(self.peek(), Tok::Ident(s) if s == "on") {
            return Err(vec![Diagnostic::error(
                "expected `on` after require kind",
                Some(self.peek_token().span),
            )]);
        }
        self.advance();
        let target = self.expect_ident()?;
        Ok(Stmt::Require {
            kind,
            target,
            span: self.span(start),
        })
    }

    fn parse_discharge(&mut self) -> ParseResult<Stmt> {
        let start = self.peek_token().span.start;
        let obligation = self.expect_ident()?;
        if !matches!(self.peek(), Tok::Ident(s) if s == "by") {
            return Err(vec![Diagnostic::error(
                "expected `by` after discharge target",
                Some(self.peek_token().span),
            )]);
        }
        self.advance();
        let by = self.expect_ident()?;
        let state = if matches!(self.peek(), Tok::Ident(s) if s == "as") {
            self.advance();
            Some(self.expect_ident()?)
        } else {
            None
        };
        Ok(Stmt::Discharge {
            obligation,
            by,
            state,
            span: self.span(start),
        })
    }

    fn parse_contradict(&mut self, start: usize) -> ParseResult<Stmt> {
        let a = self.expect_ident()?;
        let b = self.expect_ident()?;
        if !matches!(self.peek(), Tok::Ident(s) if s == "as") {
            return Err(vec![Diagnostic::error(
                "expected `as <kind>`",
                Some(self.peek_token().span),
            )]);
        }
        self.advance();
        let kind = self.expect_ident()?;
        Ok(Stmt::Contradict {
            a,
            b,
            kind,
            span: self.span(start),
        })
    }

    fn parse_branch(&mut self, start: usize) -> ParseResult<Stmt> {
        let label = self.expect_ident()?;
        if !matches!(self.peek(), Tok::Ident(s) if s == "from") {
            return Err(vec![Diagnostic::error(
                "expected `from <parent>`",
                Some(self.peek_token().span),
            )]);
        }
        self.advance();
        let parent = self.expect_ident()?;
        let mut assumptions = vec![];
        if matches!(self.peek(), Tok::Ident(s) if s == "with") {
            self.advance();
            loop {
                assumptions.push(self.expect_ident()?);
                if matches!(self.peek(), Tok::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        Ok(Stmt::Branch {
            label,
            parent,
            assumptions,
            span: self.span(start),
        })
    }

    fn parse_merge(&mut self, start: usize) -> ParseResult<Stmt> {
        let label = self.expect_ident()?;
        self.expect(&Tok::Equals)?;
        let a = self.expect_ident()?;
        if !matches!(self.peek(), Tok::Plus) {
            return Err(vec![Diagnostic::error(
                "expected `+` in merge",
                Some(self.peek_token().span),
            )]);
        }
        self.advance();
        let b = self.expect_ident()?;
        Ok(Stmt::Merge {
            label,
            a,
            b,
            span: self.span(start),
        })
    }

    fn parse_invalidate(&mut self, start: usize) -> ParseResult<Stmt> {
        let target = self.expect_ident()?;
        if !matches!(self.peek(), Tok::Ident(s) if s == "because") {
            return Err(vec![Diagnostic::error(
                "expected `because \"...\"`",
                Some(self.peek_token().span),
            )]);
        }
        self.advance();
        let reason = self.expect_str()?;
        Ok(Stmt::Invalidate {
            target,
            reason,
            span: self.span(start),
        })
    }

    fn parse_call(&mut self, start: usize) -> ParseResult<Stmt> {
        let label = self.expect_ident()?;
        self.expect(&Tok::Equals)?;
        let op = self.expect_ident()?;
        self.expect(&Tok::LParen)?;
        let inputs = self.parse_ident_list()?;
        self.expect(&Tok::RParen)?;
        self.expect(&Tok::Colon)?;
        let ty = self.parse_type()?;
        if !matches!(self.peek(), Tok::Ident(s) if s == "cap") {
            return Err(vec![Diagnostic::error(
                "expected `cap \"<capability>\"`",
                Some(self.peek_token().span),
            )]);
        }
        self.advance();
        let capability = self.expect_str()?;
        let ctx = if matches!(self.peek(), Tok::Ident(s) if s == "ctx") {
            self.advance();
            Some(self.expect_ident()?)
        } else {
            None
        };
        Ok(Stmt::Call {
            label,
            op,
            inputs,
            ty,
            capability,
            ctx,
            span: self.span(start),
        })
    }

    // -- expressions ----------------------------------------------------

    fn parse_arg_list(&mut self, depth: usize) -> ParseResult<Vec<Expr>> {
        let mut args = vec![];
        if matches!(self.peek(), Tok::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr(depth)?);
            match self.peek() {
                Tok::Comma => {
                    self.advance();
                }
                Tok::RParen => break,
                Tok::Num(_)
                | Tok::Str(_)
                | Tok::Sym(_)
                | Tok::LBrace
                | Tok::LParen
                | Tok::Ident(_) => continue,
                _ => break,
            }
        }
        Ok(args)
    }

    fn parse_ident_list(&mut self) -> ParseResult<Vec<String>> {
        let mut v = vec![];
        if matches!(self.peek(), Tok::RParen) {
            return Ok(v);
        }
        loop {
            v.push(self.expect_ident()?);
            match self.peek() {
                Tok::Comma => {
                    self.advance();
                }
                Tok::RParen => break,
                Tok::Ident(_) => continue,
                _ => break,
            }
        }
        Ok(v)
    }

    fn parse_expr(&mut self, depth: usize) -> ParseResult<Expr> {
        if depth > MAX_EXPR_DEPTH {
            return Err(vec![Diagnostic::error(
                "expression nesting too deep (possible resource exhaustion)",
                Some(self.peek_token().span),
            )]);
        }
        match self.peek().clone() {
            Tok::Ident(s) if s == "true" => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Tok::Ident(s) if s == "false" => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Tok::Num(_) => {
                let n = self.expect_num()?;
                Ok(Expr::Num(n))
            }
            Tok::Str(_) => {
                let s = self.expect_str()?;
                Ok(Expr::Str(s))
            }
            Tok::Sym(_) => {
                let s = match self.peek().clone() {
                    Tok::Sym(x) => x,
                    other => {
                        return Err(vec![Diagnostic::error(
                            format!("expected symbol, found {:?}", other),
                            Some(self.peek_token().span),
                        )])
                    }
                };
                self.advance();
                Ok(Expr::Sym(s))
            }
            Tok::Ident(s) if self.peek_token_is_call() => {
                let name = s;
                self.advance();
                self.expect(&Tok::LParen)?;
                let args = self.parse_arg_list(depth + 1)?;
                self.expect(&Tok::RParen)?;
                match name.as_str() {
                    "q" => {
                        if args.len() != 2 {
                            return Err(vec![Diagnostic::error(
                                "q() expects (value, unit-string)",
                                Some(self.peek_token().span),
                            )]);
                        }
                        let unit = match &args[1] {
                            Expr::Str(u) => u.clone(),
                            _ => {
                                return Err(vec![Diagnostic::error(
                                    "q() unit must be a string",
                                    Some(self.peek_token().span),
                                )])
                            }
                        };
                        Ok(Expr::Quantity {
                            value: Box::new(args[0].clone()),
                            unit,
                        })
                    }
                    "interval" => {
                        if args.len() != 2 {
                            return Err(vec![Diagnostic::error(
                                "interval() expects (lo, hi)",
                                Some(self.peek_token().span),
                            )]);
                        }
                        Ok(Expr::Interval {
                            lo: Box::new(args[0].clone()),
                            hi: Box::new(args[1].clone()),
                        })
                    }
                    "rat" => {
                        if args.len() != 2 {
                            return Err(vec![Diagnostic::error(
                                "rat() expects (num, den)",
                                Some(self.peek_token().span),
                            )]);
                        }
                        let (num, den) = match (&args[0], &args[1]) {
                            (Expr::Num(a), Expr::Num(b)) => (a.clone(), b.clone()),
                            _ => {
                                return Err(vec![Diagnostic::error(
                                    "rat() arguments must be integer literals",
                                    Some(self.peek_token().span),
                                )])
                            }
                        };
                        if den == "0" {
                            return Err(vec![Diagnostic::error(
                                "rat() denominator must be non-zero",
                                Some(self.peek_token().span),
                            )]);
                        }
                        Ok(Expr::RationalLit { num, den })
                    }
                    "eq" | "neq" | "lt" | "le" | "gt" | "ge" | "inset" | "notinset" => {
                        Ok(Expr::Relation { op: name, args })
                    }
                    other => Err(vec![Diagnostic::error(
                        format!("unknown value constructor `{}`", other),
                        Some(self.peek_token().span),
                    )]),
                }
            }
            Tok::Ident(s) => {
                self.advance();
                Ok(Expr::Ref(s))
            }
            Tok::LBrace => self.parse_record(depth),
            other => Err(vec![Diagnostic::error(
                format!("unexpected token in expression: {:?}", other),
                Some(self.peek_token().span),
            )]),
        }
    }

    fn peek_token_is_call(&self) -> bool {
        // True when the current Ident is immediately followed by '('.
        matches!(self.tokens[self.pos.min(self.tokens.len() - 1) + 1..].iter().find(|t| !matches!(t.kind, Tok::Newline)), Some(t) if matches!(t.kind, Tok::LParen))
    }

    fn parse_record(&mut self, depth: usize) -> Result<Expr, Vec<Diagnostic>> {
        self.expect(&Tok::LBrace)?;
        let mut fields = vec![];
        if !matches!(self.peek(), Tok::RBrace) {
            loop {
                let name = self.expect_ident()?;
                self.expect(&Tok::Colon)?;
                let value = self.parse_expr(depth)?;
                fields.push((name, value));
                if matches!(self.peek(), Tok::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(&Tok::RBrace)?;
        Ok(Expr::Record(fields))
    }

    // -- types -----------------------------------------------------------

    fn parse_type(&mut self) -> ParseResult<TypeExpr> {
        let name = self.expect_ident()?;
        if name == "record" {
            self.expect(&Tok::LBrace)?;
            let mut fields = vec![];
            if !matches!(self.peek(), Tok::RBrace) {
                loop {
                    let fname = self.expect_ident()?;
                    self.expect(&Tok::Colon)?;
                    let fty = self.parse_type()?;
                    fields.push((fname, fty));
                    if matches!(self.peek(), Tok::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            self.expect(&Tok::RBrace)?;
            return Ok(TypeExpr::Record(fields));
        }
        if name.contains(':') || name.contains('@') {
            let (ns_name, version) = name.rsplit_once('@').unwrap_or((&name, "1"));
            let (ns, ename) = ns_name.split_once(':').unwrap_or(("ext", ns_name));
            return Ok(TypeExpr::Extension {
                ns: ns.to_string(),
                name: ename.to_string(),
                version: version.to_string(),
            });
        }
        Ok(TypeExpr::Name(name))
    }

    // -- uncertainty -----------------------------------------------------

    fn parse_uncertainty(&mut self) -> ParseResult<UncertaintyExpr> {
        let name = self.expect_ident()?;
        match name.as_str() {
            "exact" => Ok(UncertaintyExpr::Exact),
            "unknown" => Ok(UncertaintyExpr::Unknown),
            "conflicting" => Ok(UncertaintyExpr::Conflicting),
            "probability" | "numeric" | "weight" => {
                self.expect(&Tok::LParen)?;
                let a = self.expect_num()?;
                self.expect(&Tok::Comma)?;
                let b = self.expect_num()?;
                self.expect(&Tok::RParen)?;
                match name.as_str() {
                    "probability" => Ok(UncertaintyExpr::Probability { lo: a, hi: b }),
                    "numeric" => Ok(UncertaintyExpr::Numeric { lo: a, hi: b }),
                    _ => Ok(UncertaintyExpr::Weight { w: a }),
                }
            }
            "external" => {
                let src = self.expect_str()?;
                Ok(UncertaintyExpr::ExternallyAsserted { source: src })
            }
            other => Err(vec![Diagnostic::error(
                format!("unknown uncertainty `{}`", other),
                Some(self.peek_token().span),
            )]),
        }
    }
}

enum StmtKind {
    Assert,
    Observe,
}
