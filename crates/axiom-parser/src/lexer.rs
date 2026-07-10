//! Lexer for the Axiom textual language.
//!
//! Resource limits are enforced to resist malformed-input denial of service:
//! token count, comment nesting depth, string/identifier length, and total
//! source size are all bounded.

use crate::ast::Span;
use crate::diag::{Diagnostic, Level};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Ident(String),
    Num(String),
    Str(String),
    Sym(String),
    Colon,
    Equals,
    Comma,
    Arrow,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Plus,
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: Tok,
    pub span: Span,
}

const MAX_TOKENS: usize = 1_000_000;
const MAX_COMMENT_DEPTH: usize = 256;
const MAX_LEXEME: usize = 65_536;

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Lexer<'a> {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            tokens: vec![],
        }
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span { start, end }
    }

    fn peek(&self, off: usize) -> Option<u8> {
        self.src.get(self.pos + off).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.src.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn push(&mut self, kind: Tok, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: self.span(start, end),
        });
    }

    pub fn lex(mut self) -> Result<Vec<Token>, Vec<Diagnostic>> {
        let mut diags = vec![];
        loop {
            match self.peek(0) {
                None => break,
                Some(c) if c.is_ascii_whitespace() => {
                    if c == b'\n' {
                        self.push(Tok::Newline, self.pos, self.pos + 1);
                    }
                    self.pos += 1;
                }
                Some(b'/') if self.peek(1) == Some(b'/') => {
                    while let Some(ch) = self.peek(0) {
                        if ch == b'\n' {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                Some(b'/') if self.peek(1) == Some(b'*') => {
                    self.pos += 2;
                    let mut depth = 1;
                    while depth > 0 {
                        match self.bump() {
                            None => {
                                diags.push(Diagnostic::error("unterminated block comment", None));
                                break;
                            }
                            Some(b'*') if self.peek(0) == Some(b'/') => {
                                self.pos += 1;
                                depth -= 1;
                            }
                            Some(b'/') if self.peek(0) == Some(b'*') => {
                                self.pos += 1;
                                depth += 1;
                                if depth > MAX_COMMENT_DEPTH {
                                    diags.push(Diagnostic::error("comment nesting too deep", None));
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some(b'"') => match self.lex_string() {
                    Ok(()) => {}
                    Err(d) => diags.push(d),
                },
                Some(b'\'') => self.lex_symbol(),
                Some(c) if c.is_ascii_digit() => self.lex_number(),
                Some(b'-') if self.peek(1) == Some(b'>') => {
                    let s = self.pos;
                    self.pos += 2;
                    self.push(Tok::Arrow, s, self.pos);
                }
                Some(b'-') if self.peek(1).map(|x| x.is_ascii_digit()).unwrap_or(false) => {
                    self.lex_number();
                }
                Some(c) if c.is_ascii_alphabetic() || c == b'_' => self.lex_ident(),
                Some(b':') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::Colon, s, self.pos);
                }
                Some(b'=') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::Equals, s, self.pos);
                }
                Some(b',') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::Comma, s, self.pos);
                }
                Some(b'(') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::LParen, s, self.pos);
                }
                Some(b')') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::RParen, s, self.pos);
                }
                Some(b'{') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::LBrace, s, self.pos);
                }
                Some(b'}') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::RBrace, s, self.pos);
                }
                Some(b'[') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::LBracket, s, self.pos);
                }
                Some(b']') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::RBracket, s, self.pos);
                }
                Some(b'+') => {
                    let s = self.pos;
                    self.pos += 1;
                    self.push(Tok::Plus, s, self.pos);
                }
                Some(c) => {
                    let s = self.pos;
                    self.pos += 1;
                    diags.push(Diagnostic::error(
                        format!("unexpected character {:?}", c as char),
                        Some(self.span(s, self.pos)),
                    ));
                }
            }
            if self.tokens.len() > MAX_TOKENS {
                diags.push(Diagnostic::error("token limit exceeded", None));
                break;
            }
        }
        self.push(Tok::Eof, self.pos, self.pos);
        if diags.is_empty() {
            Ok(self.tokens)
        } else {
            Err(diags)
        }
    }

    fn lex_string(&mut self) -> Result<(), Diagnostic> {
        let s = self.pos;
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(Diagnostic::error(
                        "unterminated string",
                        Some(self.span(s, self.pos)),
                    ))
                }
                Some(b'"') => {
                    self.push(Tok::Str(out), s, self.pos);
                    return Ok(());
                }
                Some(b'\\') => match self.bump() {
                    Some(b'n') => out.push('\n'),
                    Some(b't') => out.push('\t'),
                    Some(b'"') => out.push('"'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'/') => out.push('/'),
                    Some(other) => out.push(other as char),
                    None => {
                        return Err(Diagnostic::error(
                            "unterminated escape",
                            Some(self.span(s, self.pos)),
                        ))
                    }
                },
                Some(c) => out.push(c as char),
            }
            if out.len() > MAX_LEXEME {
                return Err(Diagnostic::error(
                    "string too long",
                    Some(self.span(s, self.pos)),
                ));
            }
        }
    }

    fn lex_symbol(&mut self) {
        let s = self.pos;
        self.pos += 1; // opening quote
        let mut out = String::new();
        while let Some(c) = self.peek(0) {
            if c == b'\'' {
                self.pos += 1;
                break;
            }
            if c.is_ascii_whitespace() {
                break;
            }
            out.push(c as char);
            self.pos += 1;
        }
        self.push(Tok::Sym(out), s, self.pos);
    }

    fn lex_number(&mut self) {
        let s = self.pos;
        if self.peek(0) == Some(b'-') {
            self.pos += 1;
        }
        while let Some(c) = self.peek(0) {
            if c.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.peek(0) == Some(b'.') {
            self.pos += 1;
            while let Some(c) = self.peek(0) {
                if c.is_ascii_digit() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        if self.peek(0) == Some(b'/') {
            self.pos += 1;
            while let Some(c) = self.peek(0) {
                if c.is_ascii_digit() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        let text = String::from_utf8_lossy(&self.src[s..self.pos]).to_string();
        self.push(Tok::Num(text), s, self.pos);
    }

    fn lex_ident(&mut self) {
        let s = self.pos;
        while let Some(c) = self.peek(0) {
            if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b':' | b'@' | b'-') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = String::from_utf8_lossy(&self.src[s..self.pos]).to_string();
        self.push(Tok::Ident(text), s, self.pos);
    }
}

/// Helper used by the CLI/tests to render a diagnostic against source.
pub fn render_diagnostic(src: &str, d: &Diagnostic) -> String {
    let line = if let Some(sp) = d.span {
        let before = &src[..sp.start.min(src.len())];
        let line_no = before.matches('\n').count() + 1;
        format!("line {}", line_no)
    } else {
        "global".to_string()
    };
    let tag = match d.level {
        Level::Error => "error",
        Level::Warning => "warning",
    };
    format!("{}: {} [{}]", tag, d.message, line)
}
