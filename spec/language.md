# Axiom IR — Textual Language

Axiom Modules are authored in a readable, compact, deterministic textual language. The
language is designed for source control and human inspection; it is *not* a natural-language
prompt and the parser uses no model-specific heuristics.

## 1. Design principles

* **Line-oriented.** Each instruction occupies one line, terminated by a newline.
* **Deterministic.** Identical source → identical AST → identical canonical form.
* **Unambiguous.** No operator precedence ambiguity; expressions are parenthesized.
* **Parseable without heuristics.** The grammar is fully syntactic; reference resolution
  and type checking happen in the runtime.
* **Source-controllable.** Declarations are explicit; there is no implicit magic.

## 2. Lexical structure

Tokens: identifiers, numbers (int/decimal/rational text), strings (`"..."`), symbols
(`'atom`), punctuation `( ) [ ] { } , : = + @ .`, keywords (`module`, `assert`, `observe`,
`assume`, `derive`, `require`, `discharge`, `verify`, `challenge`, `contradict`, `branch`,
`merge`, `invalidate`, `attest`, `call`, `evidence`, `trust`, `scope`, `ctx`, `as`, `on`,
`by`, `because`, `from`, `with`, `receipt`, `cap`, `uncertainty`, `exact`, `unknown`,
`conflicting`, `probability`, `numeric`, `weight`, `external`), and `true`/`false`.

Numbers: `i<n>` (int), `r<num>/<den>` (rational), `d<mantissa>/<scale>` (decimal), or plain
decimal/integer literals (`3.14`, `2/4`, `21.5`). The lexer preserves the literal text; the
runtime's `Num::parse` interprets it exactly.

Depth and length limits are enforced by the runtime/parser to bound resource use on hostile
input (see [`docs/security/threat-model.md`](../docs/security/threat-model.md)).

## 3. Grammar (EBNF)

```
module      := 'module' ident string newline stmt*
stmt        := evidence | assert | observe | assume | derive | require
             | discharge | verify | challenge | contradict | branch | merge
             | invalidate | attest | call

evidence    := 'evidence' ident string [string] ['trust=' ident]
assert      := 'assert' ident '=' expr ':' type
               ['evidence' '[' [ident (',' ident)*] ']']
               ['uncertainty' uncertainty] ['ctx' ident]
observe     := 'observe' ident '=' expr ':' type
               ['evidence' '[' [ident (',' ident)*] ']'] ['uncertainty' uncertainty]
assume      := 'assume' ident '=' expr ':' type 'scope' string ['ctx' ident]
derive      := 'derive' ident '=' ident '(' [ident (',' ident)*] ')' ':' type
               ['receipt' ident]
require     := 'require' ident 'on' ident
discharge   := 'discharge' ident 'by' ident ['as' ident]
verify      := 'verify' ident
challenge   := 'challenge' ident
contradict  := 'contradict' ident ident 'as' ident
branch      := 'branch' ident 'from' ident ['with' ident (',' ident)*]
merge       := 'merge' ident '=' ident '+' ident
invalidate  := 'invalidate' ident 'because' string
attest      := 'attest' ident
call        := 'call' ident '=' ident '(' [ident (',' ident)*] ')' ':' type 'cap' string

expr        := 'true' | 'false' | num | string | '\'' atom
             | 'q' '(' expr string ')'                ; quantity
             | 'interval' '(' expr expr ')'           ; numeric interval
             | 'eq'|'neq'|'lt'|'le'|'gt'|'ge' '(' expr (',' expr)+ ')'
             | 'inset'|'notinset' '(' expr (',' expr)+ ')'
             | '{' (ident ':' expr (',' ident ':' expr)*) '}'   ; record
             | ident                                  ; label reference (input only)

type        := 'bool' | 'int' | 'uint' | 'rational' | 'decimal' | 'string'
             | 'symbol' | 'quantity' | 'interval' | 'relation' | 'temporal'
             | 'record' '{' (ident ':' type (',' ident ':' type)*) '}'
             | ident ':' ident '@' ident             ; extension type

uncertainty := 'exact' | 'unknown' | 'conflicting'
             | 'probability' '(' num ',' num ')'
             | 'numeric' '(' num ',' num ')'
             | 'weight' '(' num ')'
             | 'external' '(' string ')'
```

## 4. Abstract syntax tree

The parser produces a `ModuleAst { name, version, stmts: Vec<Stmt> }`. Each `Stmt` carries a
source `Span` (start/end byte offsets) for diagnostics. The AST is syntactic only: label
references are not resolved and types are not checked at parse time.

## 5. Semantic analysis

The runtime resolves each label to a content-addressed `Id`, converts the AST to core values
(`convert_expr`, `convert_type`, `convert_uncertainty`), and executes each statement in
order. Reference order matters: a claim must be declared before it is used as a derivation
input. Resolution failure yields a precise error tied to the source span.

## 6. Formatter and canonical printer

`format_module` renders an AST back to canonical text. The formatter is **idempotent**:
`format(format(ast)) == format(ast)`. It is also a **round-trip**: `parse(format(ast))`
reproduces a structurally equal AST, and the canonical form is independent of original
source formatting (modulo semantic-irrelevant whitespace). `axiom fmt` applies it; `axiom
check` validates without rewriting.

## 7. Example

```
module thermostat "1"

evidence thermometer "text/plain" "ambient 21.5C" trust=trusted

observe ambient = q(21.5 "C") : quantity evidence [thermometer]
observe limit  = q(30.0 "C") : quantity

derive total = qadd(ambient, limit) : quantity

require dimensional-consistency on total
verify total
```

This module parses, type-checks, executes, and yields a `verified` `total` claim with a
complete derivation and a satisfied dimensional-consistency obligation.

## 8. Limitations

* The textual language is one surface; the canonical interchange format
  ([`encoding.md`](encoding.md)) is the portable object. The language round-trips to the
  same canonical form but is not the canonical bytes.
* Label references are resolved by the runtime, not the parser; a forward reference is a
  runtime error, not a parse error.
