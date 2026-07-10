# Axiom IR — Canonical Encoding and Content Addressing

This document specifies the provider-neutral canonical format: the deterministic
serialization, the domain-separated content-addressed identifiers, and the hashing rules.
The same semantic module MUST produce the same canonical bytes across runs, platforms, and
implementations.

## 1. Canonical JSON

Axiom uses **canonical JSON** with the following rules:

* Object keys are **recursively sorted** in UTF-8 byte order.
* No insignificant whitespace is emitted (separators `,` and `:` with no surrounding space).
* Unicode is preserved as UTF-8 (not escaped to `\uXXXX`), except the JSON-required escapes
  for `"`, `\`, control characters, and newlines.
* Numbers are encoded as their exact textual form; integers and rationals/decimals carry
  their `i`/`r`/`d` prefix (see the type system) so no precision is lost.
* Maps (objects) are encoded as JSON objects; order is never significant.

`canonical_bytes(v)` is a total function: `canonical_bytes(a) == canonical_bytes(b)` iff `a`
and `b` are structurally equal after key sorting.

## 2. Content-addressed identifiers

Every addressable object has an `Id` of the textual form:

```
<domain>.1.<hex-sha256-32>
```

The domain tag disambiguates object categories. The digest is computed as:

```
digest = SHA-256( domain_tag || 0x00 || canonical_bytes(object) )
```

`domain_tag` is one of: `claim`, `evidence`, `assumption`, `context`, `derivation`,
`obligation`, `contradiction`, `receipt`, `module`, `operation`, `event`, `extension`.

### Domain separation

Because the domain tag is prepended before hashing, **structurally different object
categories can never collide through identical raw serialization**. A claim whose content
equals an evidence node's content hashes to two distinct ids (`claim.1....` vs
`evidence.1....`). This prevents a hash-confusion attack where an attacker supplies a value
that matches an unrelated object's digest.

## 3. Identity classes

Axiom distinguishes several identity categories. Each participates in its own digest:

| Identity | Over what | Domain |
|---|---|---|
| semantic content hash (`semantic_id`) | `(type, value)` of a claim | `claim` |
| execution instance / node id (`id`) | `(label, context_id)` of a claim | `claim` |
| source-level symbol (`label`) | not hashed | — |
| receipt id | `(op, op_version, provider, logical_time, inputs_hash, output_hash, integrity)` | `receipt` |
| context id | `(parent, label, assumptions)` or root marker | `context` |
| module id | digest of all stable node sets | `module` |

Equivalent serialization of a proposition yields the same `semantic_id` regardless of which
claim node carries it.

## 4. Version identifiers

Every module carries `format_version` (the Axiom format major version, currently `"1"`).
Every operation carries a semantic version `op_version`. A derivation records `op_version`
and the runtime's `runtime_version` so that reproduction is exact. An independent
implementation MUST record the same version strings to reproduce identical ids.

## 5. Extension fields and unknown-field behavior

Core objects serialize with explicit fields. An extension value carries opaque bytes
`data` together with its `ns`, `name`, `version`. A receiver that does not implement an
extension MUST reject the module with `ERR-UNSUPPORTED-EXTENSION` rather than silently
ignore the extension (see [`extensions.md`](extensions.md)). The reference implementation
does not silently drop unknown core fields; canonical JSON is fixed-schema for core objects.

## 6. Hashing properties

* **Stable:** identical inputs → identical digest (asserted by canonicalization tests).
* **Deterministic across platforms:** no floating point, no map-order dependence, no
  locale-dependent formatting.
* **Second-preimage resistant:** SHA-256; an attacker cannot forge an id for a different
  object without breaking SHA-256.
* **Domain-isolated:** cross-category collisions are impossible by construction.

## 7. Test vectors

All digests below are `SHA-256(domain || 0x00 || canonical_bytes(object))`. They are
authoritative; an independent implementation MUST reproduce them.

### Vector 1 — canonical byte stability

Object `{"b":1,"a":2}` canonicalizes to `{"a":2,"b":1}` (bytes
`b'{"a":2,"b":1}'`). Its claim id is:

```
claim.1.477c7f13f3c2a5929f180821df4e0fd82e58c8996e7c893996b1735c4ff7ef96
```

Swapping key order (`{"a":2,"b":1}`) yields identical canonical bytes and identical id —
proving order-independence.

### Vector 2 — domain separation

Object `{"x":1}` hashed under `claim` and `evidence` yields distinct ids:

```
claim.1.  477c...   (see vector 1 structure; computed for {"x":1})
evidence.1.7951eb27421950d951a93b1658fd169d3d4fe26446461de10b9d028772a3e5cb
```

(Concretely: `evidence.1.7951eb27421950d951a93b1658fd169d3d4fe26446461de10b9d028772a3e5cb`
for `{"x":1}`.)

### Vector 3 — number encoding

* `i3` is the canonical text of the integer 3.
* `r1/2` is the canonical text of the rational 1/2 (never `0.5`, which would be `d5/1`).
* `d15/1` is the canonical text of 1.5.

### Vector 4 — module digest

The module digest is `SHA-256(module || 0x00 || concat(sorted canonical bytes of claims,
evidence, derivations, obligations, contradictions))`. The conformance corpus
([`conformance.md`](conformance.md)) pins exact module digests for reference fixtures; an
independent implementation reproduces them when it reaches an identical committed state.

## 8. Why not CBOR or a custom binary?

Canonical JSON is chosen for implementability and auditability: any language can produce it,
humans can inspect it, and the sorting rule is unambiguous. The determinism class of every
operation is documented; floating point is excluded from the normative path, so JSON number
encoding never introduces rounding. If a future version needs a binary encoding, it MUST be
defined as a deterministic projection of the same canonical JSON bytes.
