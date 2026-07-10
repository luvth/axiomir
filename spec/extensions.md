# Axiom IR — Extension Model

Axiom is an open standard. Extensions let independent parties add semantic types,
operations, uncertainty models, and contradiction kinds without forking the core. Extensions
MUST NOT silently alter core semantics.

## 1. Semantic versioning

* The **format version** (`format_version`, currently `"1"`) changes only on
  backward-incompatible changes to the core encoding or calculus.
* **Operation versions** (`op_version`) and **extension versions** (`version`) follow
  semantic versioning. A derivation records the exact version required to reproduce it.

## 2. Feature negotiation

A module MAY declare `required` extensions. A conformant runtime:

* recognizes the extension and proceeds, or
* responds: *"This module is syntactically valid but requires unsupported semantic
  extension X version Y"* and rejects the module with `ERR-UNSUPPORTED-EXTENSION`.

This is explicit, not silent. An unrecognized extension MUST NOT be ignored.

## 3. Extension namespaces

Extensions are addressed by `(namespace, name, version)`:

* Extension **types**: `Type::Extension { ns, name, version }`, carried by
  `Value::Extension { ns, name, version, data }` (opaque bytes).
* Extension **operations**: registered as `OperationDef` with a name containing `:`.
* Extension **uncertainty**: `Uncertainty::Extension { ns, name, version, data }`.
* Extension **contradictions**: `ContradictionKind::Extension(k)`.

Namespaces prevent collisions: two vendors may define `foo:widget` independently.

## 4. Custom types, operations, uncertainty

* A custom type is declared and its structural carrier is `Value::Extension`. The core
  treats it opaquely for arithmetic/comparison; equality is bytewise.
* A custom operation is registered with full metadata (input types, output type,
  determinism class, capabilities, generated obligations). It is executed by the host's
  extension executor, which MUST also be able to produce a receipt for replay.
* A custom uncertainty model declares its own `combine` rule; the core will not compose it
  with other models (it is an error to do so).

## 5. Required-extension declarations

A module that uses an extension MUST record the requirement so that a reader knows what it
needs. The canonical encoding includes the extension's `ns:name@version` so the requirement
is self-describing.

## 6. Canonical extension encoding

`Value::Extension` serializes as `{"ns","name","version","data"}` where `data` is the
canonical bytes of the extension's payload. `canonical_bytes` sorts keys, so the encoding is
stable. An unsupported extension's `data` is still hashed (so its id is stable) even though
the runtime cannot interpret it.

## 7. Graceful rejection

A runtime that cannot satisfy a required extension MUST:

1. parse the module (syntax is independent of semantics),
2. detect the missing extension,
3. emit a precise diagnostic naming the extension and version,
4. refuse to execute the extension-dependent transitions.

It MUST NOT silently drop the extension, substitute a default, or pretend to have executed
it.

## 8. Limits

* An extension MUST NOT redefine the meaning of a core type, operation, or status transition.
* An extension MUST provide a replay story (a receipt) if it performs external effects.
* An extension that cannot be made deterministic MUST declare `NonDeterministic` and require
  a receipt for verification.
