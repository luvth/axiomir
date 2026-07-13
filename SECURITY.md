# Security Policy

## Reporting a vulnerability

Axiom IR is designed to execute **untrusted, model-produced reasoning** — including modules
that are deliberately invalid, contradictory, or attempting to exceed their granted authority.
If you discover a vulnerability (e.g., a way to reach `verified` without discharging mandatory
obligations, a receipt-integrity bypass, a capability-escalation, or a denial-of-service via
malformed input), please **report it to the maintainers via a private channel; do not open
public issues for vulnerabilities.**

Please include:

- A description of the impact and the violated invariant (ideally INV-VERIFY or INV-MINIMALITY).
- A minimal `.axiom` module or receipt log that reproduces the issue.
- The expected vs. observed behavior.

We will acknowledge receipt, work on a fix, and coordinate disclosure. Axiom IR is currently at
`0.2.0` (pre-stable); treat all interfaces as subject to change.

## Threat model

Axiom IR adopts a **hostile-model assumption**: the reasoning module (and the model or agent
that produced it) is not trusted. The runtime must not silently accept invalid reasoning as
verified, must not let a contradiction erase either branch, and must not let an ungranted
external operation execute.

The full threat model is described in [`docs/security/threat-model.md`](docs/security/threat-model.md).

Key design commitments:

- **Verified does not mean proven.** "Verified" means *obligation-discharged under recorded
  receipts and the current rule versions*. Axiom is not a sound theorem prover; it is as
  trustworthy as its evidence, receipts, and operation implementations.
- **Default-deny capability posture.** The runtime's default executor is the
  `ReplayOnlyExecutor`: it can reconstruct external outputs from integrity-checked receipts but
  **cannot** invoke any live external operation. A live external operation runs only when its
  required capability (e.g., `tool:calculator`) is **explicitly granted** via `Runtime::grant`
  or `axiom run --cap`. Capabilities are never assumed.
- **Receipt integrity.** External receipts carry an integrity digest; tampering with a receipt's
  output is detected and rejected at replay time.
- **Bounded parsing.** The parser enforces resource limits (token count, comment depth, string/
  identifier length, total source size) to resist malformed-input denial of service.

## Supported versions

Only the latest `0.2.x` release line is supported during the pre-stable period.
