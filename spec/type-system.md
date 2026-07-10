# Axiom IR — Type System

This document specifies the initial practical type system for Axiom claims. The type system
is intentionally small and exact; it is the vocabulary a claim's `ty` field may take.

## 1. Values

A value `v` is one of:

| Constructor | Rust | Notes |
|---|---|---|
| `Bool(b)` | `Value::Bool` | classical proposition |
| `Num(n)` | `Value::Num` | exact numeric (see below) |
| `Str(s)` | `Value::Str` | opaque text |
| `Sym(s)` | `Value::Sym` | symbolic atom |
| `Quantity(q)` | `Value::Quantity` | numeric value carrying a unit |
| `Interval{lo,hi}` | `Value::Interval` | numeric interval `[lo, hi]` |
| `Relation(r)` | `Value::Relation` | equality/inequality/membership/temporal |
| `Record(fields)` | `Value::Record` | named, typed fields |
| `Extension{ns,name,version,data}` | `Value::Extension` | opaque extension value |

### Exact numerics

The normative numeric type `Num` is one of:

* `Int(i128)` — signed integer.
* `Rational{num, den}` with `den > 0` and `gcd(num, den) = 1` — reduced rational.
* `Decimal{mantissa, scale}` with `scale ≤ 38` — fixed-scale decimal, value
  `mantissa * 10^-scale`.

All arithmetic is exact: `checked_add`, `checked_sub`, `checked_mul`, `checked_div` return
a `NumError` on overflow or division by zero rather than producing an approximate result.
Floating point is **forbidden** in the normative path. Canonical text forms are `i<n>`,
`r<num>/<den>`, `d<mantissa>/<scale>` (e.g. `i3`, `r1/2`, `d15/1`).

### Quantities and units

A `Quantity` is `(value: Num, unit: Unit)`. A `Unit` is a map of base-unit symbol to integer
exponent, canonicalized by dropping zero exponents. Quantities may be added or subtracted
only when their dimensions match (`same_dimension`); multiplication/division combine
exponents. Unit mismatch is a `TypeError`, not a silent coercion.

### Relations

```
Relation ::= Eq(v, v) | Neq(v, v) | Lt | Le | Gt | Ge
           | InSet(v, [v]) | NotInSet(v, [v])
           | TemporalAt{when: Num, claim: v}
```

## 2. Types

A claim type `τ` is one of:

| Type | Syntax | Value carrier | Notes |
|---|---|---|---|
| Boolean proposition | `bool` | `Bool` | |
| Signed integer | `int` | `Num` | rejects negative on `uint` |
| Unsigned integer | `uint` | `Num` | `signed=false` |
| Decimal | `decimal` | `Num` | |
| Rational | `rational` | `Num` | |
| String | `string` | `Str` | |
| Symbol | `symbol` | `Sym` | |
| Quantity | `quantity` | `Quantity` | with units |
| Numeric interval | `interval` | `Interval` | `[lo, hi]` |
| Equality/inequality/membership | `relation` | `Relation` | |
| Structured record | `record { f: τ, ... }` | `Record` | field-ordered, named |
| Temporal assertion | `temporal` | `Relation(TemporalAt)` | proposition at a logical time |
| Extension type | `ns:name@version` | `Value::Extension` | externally defined |

## 3. Type checking

`value.check(τ)` is a total function returning `Ok(())` or a precise `TypeError`. Rules:

* `Bool` accepts `Bool`.
* `Int{signed}` accepts `Num`; `uint` rejects negative `Num`.
* `Decimal`, `Rational` accept any `Num`.
* `String`, `Symbol`, `Quantity`, `NumericInterval`, `Relation` accept their carrier.
* `Record{fields}` accepts a `Record` of equal arity, matching field names, with each field
  value accepted by its declared field type.
* `Extension{ns,name,version}` accepts only a `Value::Extension` with matching
  `ns`, `name`, `version`.

## 4. Identity and canonicalization

Three distinct identities are tracked per claim:

* **semantic identity** `semantic_id = content_id(Claim, (ty, value))` — the proposition.
* **node identity** `id = content_id(Claim, (label, context_id))` — the addressable node.
* **source span** — for human diagnostics only, never hashed into identity.
* **label** — a human-readable symbol, never hashed into identity.

Equivalent serialization MUST NOT create different semantic hashes: `canonical_bytes`
recursively sorts object keys, so two structurally equal values produce identical canonical
bytes and identical `semantic_id`. Canonicalization is explicit and deterministic. Axiom
does NOT claim general semantic equivalence where it is undecidable; node identity is the
fallback.

## 5. Extension values

`Value::Extension{ns, name, version, data}` carries opaque bytes defined by an extension.
Its `structural_type` is the matching `Type::Extension`. The core treats extension values
as opaque for arithmetic and comparison; equality is bytewise. An extension that redefines
comparison MUST declare so in its specification and MUST NOT silently alter core semantics.
