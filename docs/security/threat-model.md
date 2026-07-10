# Axiom IR — Threat Model

## Operating assumption

All model-generated modules are **hostile**. We assume an adversary controls the
textual or programmatic module input and will attempt to compromise the host,
forge trust, or extract secrets. Axiom's job is to execute such input in a way
that (a) preserves the integrity of the proof-carrying invariants, (b) never
performs an unauthorized side effect, and (c) fails closed rather than silently
accepting invalid reasoning.

The defense is layered: a pure, I/O-free core (`axiom-core`) enforces the
transition calculus; `axiom-encoding` provides domain-separated content
addressing; `axiom-runtime` injects all side effects through an explicit
capability-gated executor; and `axiom-parser` rejects malformed input at the
syntactic boundary. We enumerate each required-protection threat below with its
concrete control, the implementing crate/function, and any residual risk.

## Trusted computing base

The following crates are **pure and I/O-free** and constitute the auditable TCB:

* `axiom-core` (`crates/axiom-core/src/lib.rs`) — the semantic objects,
  transition calculus, `Module::verify`, `Module::check_verification_invariant`,
  and `Receipt::verify_integrity`. Performs no file, network, or process I/O.
  External operations reach it only as a receipt or an executor callback.
* `axiom-encoding` (`crates/axiom-encoding/src/lib.rs`) — canonical JSON and
  domain-separated content addressing (`content_id`, `hash_domain`, `Domain`).
  No I/O.
* `axiom-types` (`crates/axiom-types/`) — value, number, unit, and uncertainty
  types. Pure.

`axiom-runtime` does all of its I/O exclusively through an injected
`ExternalExecutor` and an explicit `Capability`/receipt configuration. By default
it installs `ReplayOnlyExecutor` and grants **no** capabilities. `axiom-parser`
is purely syntactic with no model heuristics, so it cannot be steered by content
semantics.

## Capability model and default-deny posture

External operations are classified by `OpClass` (`PureInternal`,
`VerifiedExternal`, `UnverifiedExternal`, `NonDeterministic`, `Privileged`) and
declare the `Capability` values they require: `Capability::Net(String)`,
`Capability::FsRead(String)`, `Capability::Tool(String)`,
`Capability::Privileged(String)` (defined in `axiom-core/src/lib.rs`).

The runtime holds its granted capabilities as a flat string set
(`Runtime::caps: BTreeSet<String>`). A `call` statement is executed in
`Runtime::exec_call`: before invoking the executor it checks
`self.caps.contains(capability)`, returning `RuntimeError::CapabilityDenied`
otherwise. The default `Runtime::new` grants nothing, and the default executor
(`ReplayOnlyExecutor`, `crates/axiom-runtime/src/external.rs`) refuses every
live call with `RuntimeError::ReplayMissingReceipt`. A module therefore **cannot
escalate** beyond the capabilities a host explicitly grants via `Runtime::grant`
or the CLI `--cap` flag, and cannot perform any live external effect unless the
host installs an executor (e.g. `Runtime::with_builtin_tool`).

## Threat catalog

### Malformed syntax
* **Attack:** garbage, truncated, or grammatically invalid source.
* **Defense:** `axiom_parser::parse_module` / `Parser::parse` return
  `Err(Vec<Diagnostic>)`; execution never begins on a module that fails to parse.
  Unknown instructions and value constructors produce explicit diagnostics.
* **Crate/function:** `axiom-parser` (`parser.rs`, `lexer.rs`).
* **Residual risk:** low; parser is line-oriented and fully syntactic.

### Parser resource exhaustion
* **Attack:** input engineered to consume unbounded lexer/parser resources.
* **Defense:** The lexer enforces hard bounds: `MAX_TOKENS = 1_000_000`,
  `MAX_COMMENT_DEPTH = 256`, `MAX_LEXEME = 65_536`, and per-source-size limits
  via token count. Exceeding any emits a diagnostic and aborts.
* **Crate/function:** `axiom-parser/src/lexer.rs`.
* **Residual risk:** bounded; a module may still be up to ~1M tokens, which is
  fully parsed and executed (see "denial-of-service modules" below).

### Deeply nested structures
* **Attack:** a value such as `eq(eq(eq(...)))` or a record `{{{{...}}}}` with
  very deep nesting, intended to blow the call stack during recursive parsing.
* **Defense:** the recursive-descent parser threads a `depth` counter through
  `parse_expr` / `parse_arg_list` / `parse_record`; beyond `MAX_EXPR_DEPTH`
  (256) it returns a diagnostic and fails closed instead of recursing further.
  The lexer token cap bounds total size independently. A deeply nested input is
  therefore rejected with a clean error, never with a stack overflow.
* **Crate/function:** `axiom-parser/src/parser.rs`.
* **Residual risk:** none within the token budget; nesting beyond the bound is a
  clean diagnostic. (See "denial-of-service modules" for the node-count cap.)

### Cyclic graphs
* **Attack:** a module whose `derive`/`call` dependencies form a cycle, so the
  worklist can never make progress.
* **Defense:** `Runtime::execute` resolves dependency-bearing statements in a
  worklist; if no remaining statement can make progress it returns an error
  (`RuntimeError::UnknownLabel` reporting "cycle or genuinely missing label").
  The module fails closed; no partial or silent acceptance.
* **Crate/function:** `axiom-runtime/src/runtime.rs` (`execute`, `stmt_ready`).
* **Residual risk:** a cyclic module is rejected entirely (by design — there is
  no fixed-point semantics yet; see `docs/research/roadmap.md`).

### Hash confusion
* **Attack:** craft two different object categories (e.g. a claim and an evidence
  record) that collide under identical raw serialization, subverting identity.
* **Defense:** `content_id` / `hash_domain` prepend a domain tag
  (`domain || 0x00 || canonical_bytes`). The `Domain` enum carries 12 distinct
  tags (Claim, Evidence, Assumption, Context, Derivation, Obligation,
  Contradiction, Receipt, Module, Operation, Event, Extension), so a claim digest
  can never equal an evidence digest with identical content. Canonical bytes also
  have recursively sorted keys.
* **Crate/function:** `axiom-encoding/src/lib.rs`.
* **Residual risk:** none, assuming SHA-256 collision resistance.

### Forged receipts
* **Attack:** supply a receipt with edited output, mismatched inputs, or a
  fabricated integrity digest to make an external derivation verify.
* **Defense:** `Receipt::verify_integrity()` recomputes the integrity digest from
  recorded fields and is checked in `Module::derive` (`CoreError::ReceiptTampered`)
  and in `Runtime::exec_call`'s replay path; `Module::check_verification_invariant`
  re-validates after execution. A derivation referencing a tampered receipt
  cannot verify, and replay fails.
* **Crate/function:** `axiom-core/src/lib.rs` (`Receipt::verify_integrity`,
  `Module::derive`, `check_verification_invariant`), `runtime.rs`.
* **Residual risk:** none for integrity. (Signature validation is a separate,
  currently-unimplemented concern — see "invalid signatures".)

### Forged evidence
* **Attack:** author evidence with arbitrary `trust` classification and attach it
  to a claim, manufacturing apparent support.
* **Defense:** `Module::assert`/`discharge` require referenced evidence to exist
  (`CoreError::UnknownEvidence`); an undeclared evidence label is rejected. Axiom
  also distinguishes evidence *existence* from evidence *relevance* — attaching
  evidence records a dependency but discharges no obligation and does not by
  itself verify a claim.
* **Crate/function:** `axiom-core/src/lib.rs` (`Module::assert`, `discharge`).
* **Residual risk:** **GAP.** Evidence content and its `trust` field are
  self-asserted by the module author; the runtime performs no authenticity check
  (no signature/hash binding to a trusted provider is verified). A host that
  needs trust must inject evidence it has independently verified, rather than
  trusting evidence authored inside the module.

### Invalid signatures
* **Attack:** tamper with a receipt or evidence whose `signature` field should
  have bound it to a trusted issuer.
* **Defense:** the `signature` fields exist on `Receipt` and `Evidence`, but there
  is **no code path that validates them**. Integrity is covered by the receipt
  hash (above); evidence signatures are not checked at all.
* **Crate/function:** `axiom-core/src/lib.rs` (fields present, no verifier).
* **Residual risk:** **GAP.** Signatures are carried for forward compatibility but
  not enforced. Verification today rests on the integrity hash and on the host's
  responsibility to supply trustworthy evidence/receipts.

### Extension namespace collisions
* **Attack:** two vendors define `foo:widget`; one shadows the other.
* **Defense:** extensions are addressed by `(namespace, name, version)`
  (`Type::Extension { ns, name, version }`, `Value::Extension { ns, name, version,
  data }`, `ContradictionKind::Extension(s)`); operation names contain `:`. The
  parser resolves `ns:name@version` in `parse_type`. An unrecognized extension is
  rejected explicitly, never silently substituted (`ERR-UNSUPPORTED-EXTENSION`
  per `spec/extensions.md`).
* **Crate/function:** `axiom-core`, `axiom-parser/src/parser.rs`,
  `crates/axiom-runtime/src/convert.rs`.
* **Residual risk:** none by design.

### Capability escalation
* **Attack:** a module invokes an operation requiring a capability it was not
  granted, or forges a capability token.
* **Defense:** capabilities are only added via `Runtime::grant` (host-controlled);
  `exec_call` checks `self.caps.contains(capability)` and fails with
  `CapabilityDenied` otherwise. Privileged operations carry `OpClass::Privileged`
  and `Capability::Privileged`.
* **Crate/function:** `axiom-runtime/src/runtime.rs` (`grant`, `exec_call`).
* **Residual risk:** none within the module's power; a host may over-grant, which
  is the host's decision, not the module's.

### Unsafe external calls
* **Attack:** induce a live side effect (network, FS, shell) through the runtime.
* **Defense:** default `ReplayOnlyExecutor` refuses all live calls. Live calls
  require an explicit executor. The only built-in live executor
  (`ToolCalculator`) handles exactly one operation, `tool.calculator`, performing
  an exact numeric sum with `checked_add` — no FS, network, or shell.
* **Crate/function:** `axiom-runtime/src/external.rs`.
* **Residual risk:** a custom `ExternalExecutor` provided by the host is
  host-trusted and must be sandboxed by the host; the framework provides the
  capability gate but not a sandbox.

### Path traversal / command injection
* **Attack:** an external operation reads/writes outside a sandbox or executes a
  shell.
* **Defense:** the reference has **no** filesystem or shell executor. `call`
  builds operation metadata inline and routes to the (default replay-only or
  explicitly-installed) executor; no path or command string is ever interpreted.
* **Crate/function:** `axiom-runtime/src/external.rs`, `runtime.rs` (`exec_call`).
* **Residual risk:** only reachable through a host-supplied executor, which is
  gated by the declared `Capability::FsRead(path)` / `Capability::Net` and remains
  the host's responsibility to sandbox.

### Uncontrolled recursion
* **Attack:** structures or algorithms that recurse without bound.
* **Defense:** execution cycle detection is bounded (the worklist errors on no
  progress); `invalidate_and_recompute` uses BFS over the dependency graph; and
  the parser enforces `MAX_EXPR_DEPTH` on expression/record nesting (see
  "deeply nested structures").
* **Crate/function:** `axiom-runtime/src/runtime.rs`, `crates/axiom-incremental`,
  `axiom-parser/src/parser.rs`.
* **Residual risk:** bounded by input size and the nesting cap.

### Arithmetic overflow
* **Attack:** supply operands that overflow/underflow or divide by zero.
* **Defense:** `BuiltinExecutor::exec` uses `checked_add`/`checked_sub`/
  `checked_mul` (returning `ExecError::Value`) and `checked_div` (returning
  `DivZero`); `ToolCalculator` uses `checked_add`. Numeric values are exact
  (`Num` is arbitrary-precision in `axiom-types`), so the normative path has no
  IEEE-754 rounding.
* **Crate/function:** `axiom-core/src/registry.rs`, `runtime/src/external.rs`.
* **Residual risk:** none for builtins — overflow/zero-division returns an error,
  the claim fails to derive, and stays unverified.

### Denial-of-service modules
* **Attack:** a large but well-formed module that consumes excessive CPU/memory.
* **Defense:** the lexer token cap (`MAX_TOKENS = 1_000_000`) bounds input size
  during parsing, and `Runtime::execute` enforces `MAX_MODULE_STMTS = 100_000`
  on the number of statements, failing closed ("module too large") before
  execution begins. `Runtime::detect_contradictions` is pairwise O(n²) over a
  context's claims but is only invoked on demand (not automatically during
  `execute`), bounding its cost.
* **Crate/function:** `axiom-parser/src/lexer.rs`, `runtime.rs` (`execute`,
  `detect_contradictions`).
* **Residual risk:** bounded. A module at the statement cap is parsed and fully
  executed; automatic contradiction detection is not run by `execute()` or by
  the `axiom contradictions` command — only `contradict ... as <kind>`
  statements populate the contradiction set unless the caller explicitly invokes
  `Runtime::detect_contradictions(ctx)`.

### Maliciously large invalidation fronts
* **Attack:** a single change that triggers recomputation of an enormous
  dependent set.
* **Defense:** `invalidate_and_recompute` computes exactly the transitive
  dependency frontier (`frontier(n)`) and invalidates no claim outside it
  (INV-MINIMALITY). Unrelated verified claims are preserved. The cost scales with
  the module graph size, which is bounded by the token cap.
* **Crate/function:** `crates/axiom-incremental/src/lib.rs`.
* **Residual risk:** bounded by module size; cost is inherent to the request.

## Honest limitations (summary)

1. **Evidence is self-attested** — the runtime checks existence, not authenticity;
   trust must be supplied by the host.
2. **Signatures are not validated** — `Receipt::signature` and
   `Evidence::signature` are carried but unenforced; integrity relies on the
   receipt hash.
3. **Automatic contradiction detection is opt-in** — it is a public method, not
   part of default execution.

The parser recursion-depth limit (`MAX_EXPR_DEPTH`) and the module statement
cap (`MAX_MODULE_STMTS`) close the two resource-exhaustion gaps previously
listed here. None of the remaining limitations allow a hostile module to
*silently* verify invalid reasoning or to perform an unauthorized side effect;
they are trust-provenance concerns that should be closed as the engine matures.
