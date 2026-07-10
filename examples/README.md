# Axiom IR examples

Each file is a small, self-contained `.axiom` module demonstrating one feature. Comments use
`//` (line) or `/* ... */` (block) syntax. Run them with the `axiom` CLI (built from
`crates/axiom-cli`).

| File | Demonstrates |
|---|---|
| `typed_math.axiom` | Quantity arithmetic, observations with evidence, a scoped assumption, a mandatory proof obligation (`require`/`discharge`), and `verify`. |
| `contradiction.axiom` | Two disjoint interval observations and an explicit `contradict` witness (neither branch is erased). |
| `contexts.axiom` | A root-level contradiction plus a `branch` with a context-local `derive ... ctx` that is `verify`ed inside the branch. |
| `external_tool.axiom` | A capability-gated external `call tool.calculator(...) cap "tool:calculator"`, verified via its receipt; the basis for offline replay. |
| `incremental.axiom` | A base premise, a chain of derivations, and an independent premise — used to show the invalidation frontier. |

## Commands

```sh
# Typed math + obligation discharge (no capability needed)
axiom check   examples/typed_math.axiom
axiom run     examples/typed_math.axiom
axiom verify  examples/typed_math.axiom
axiom explain examples/typed_math.axiom work

# Contradiction witness
axiom check          examples/contradiction.axiom
axiom contradictions examples/contradiction.axiom

# Contexts (root conflict + branch-local verification)
axiom check          examples/contexts.axiom
axiom contradictions examples/contexts.axiom
axiom verify         examples/contexts.axiom

# External tool — requires the capability AND the builtin tool executor
axiom run   --cap tool:calculator --builtin-tool examples/external_tool.axiom
axiom verify --cap tool:calculator --builtin-tool examples/external_tool.axiom

# Offline replay (no live capability; receipts only)
axiom run --cap tool:calculator --builtin-tool examples/external_tool.axiom \
    --emit-receipts receipts.json
axiom replay receipts.json

# Incremental invalidation frontier
axiom check               examples/incremental.axiom
axiom verify              examples/incremental.axiom
axiom invalidate          examples/incremental.axiom base
```

Notes:

- `axiom check` parses and executes a module with no external capabilities granted. Modules that
  use `call ... cap "..."` (like `external_tool.axiom`) require `--cap <name> --builtin-tool`.
- `axiom invalidate <module> <node>` invalidates a claim/assumption/evidence node and runs the
  incremental engine. In `incremental.axiom`, invalidating `base` invalidates `a` and `b` but
  leaves the independent `indep` untouched.
- `axiom replay <log>` reconstructs a module from a receipt log without re-invoking any live
  external operation, and verifies that the digest is identical to the live run.
