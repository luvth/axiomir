# Axiom IR — Python bridge (`axiom-ir`)

A thin, honest Python wrapper around the compiled **`axiom` CLI** — the
developer tool for [Axiom IR](https://github.com/axiom-ir/axiom), a
proof-carrying intermediate representation and deterministic runtime for machine
reasoning.

The bridge **shells out** to the `axiom` binary, asks for its structured
`--json` output, and returns the decoded payload. It reimplements **none** of
Axiom's semantics: every result is exactly what the CLI emitted. If the CLI
returns a non-zero exit code, the bridge raises `AxiomError` carrying the
decoded JSON payload (when present), the exit code, and any captured stderr.

Pure standard library only — **no third-party dependencies** (`subprocess`,
`json`, `pathlib`, `shutil`, `os`).

## Requirements

- Python >= 3.10
- A compiled `axiom` binary (see below)

## 1. Build the Rust CLI

From the repository root:

```bash
cargo build -p axiom-cli          # debug binary at target/debug/axiom
# or, for a release build:
cargo build --release -p axiom-cli
```

## 2. Locate the binary (automatic)

The bridge resolves the binary in this order, without any configuration:

1. An explicit path passed to `Axiom(binary="...")`.
2. The `AXIOM_BIN` environment variable.
3. `shutil.which("axiom")` — a binary already on `PATH`.
4. `<repo>/target/release/axiom`.
5. `<repo>/target/debug/axiom`.

If none is found, `Axiom()` raises a clear `AxiomError`.

To force a specific binary, set the env var or pass it explicitly:

```bash
export AXIOM_BIN=/abs/path/to/axiom
```

## 3. Install / use the package

From this directory (`bindings/python/`):

```bash
pip install .
```

This also installs a `axiom-py` console script that runs `doctor` and prints
the JSON report.

You can also use the package in place (no install) by adding `bindings/python`
to `sys.path` — that is how the bundled `examples/demo.py` and
`tests/test_bridge.py` work.

## Public API

```python
from axiom import Axiom, AxiomError, find_binary

client = Axiom()                  # raises AxiomError if no binary found
summary = client.check("m.axiom")   # -> dict (the CLI's JSON payload)
```

Every method mirrors a CLI command and returns the parsed JSON object:

| Method | CLI command |
| --- | --- |
| `check(path)` | `axiom check <path> --json` |
| `run(path, caps=None, builtin_tool=False, emit_receipts=None)` | `axiom run ... --json` |
| `verify(path)` | `axiom verify <path> --json` |
| `explain(path, claim)` | `axiom explain <path> <claim> --json` |
| `trace(path, claim)` | `axiom trace <path> <claim> --json` |
| `contradictions(path)` | `axiom contradictions <path> --json` |
| `invalidate(path, node)` | `axiom invalidate <path> <node> --json` |
| `replay(receipt_log)` | `axiom replay <receipt_log> --json` |
| `diff(a, b)` | `axiom diff <a> <b> --json` |
| `graph(path)` | `axiom graph <path> --json` |
| `inspect(path, node)` | `axiom inspect <path> <node> --json` |
| `conform(fixtures="conformance")` | `axiom conform --fixtures <dir> --json` |
| `fmt(path)` | `axiom fmt <path> --json` |
| `demo(which="all")` | `axiom demo <which> --json` |
| `doctor()` | `axiom doctor --json` |

## Example

```python
import tempfile
from pathlib import Path
from axiom import Axiom

module_src = '''\
module example "1"
assert a = 2 : rational
assert b = 3 : rational
derive out = add(a, b) : rational
verify out
'''

client = Axiom()

with tempfile.TemporaryDirectory() as tmp:
    path = Path(tmp) / "example.axiom"
    path.write_text(module_src)

    # check: parse + report verified claims
    summary = client.check(path)
    print("verified:", summary["verified"])          # ['out']

    # explain: why does `out` hold?
    expl = client.explain(path, "out")
    print("derivation:", expl["derivation"])          # {'op': 'core.add', ...}

    # contradictions: list witnesses
    contradictions = client.contradictions(path)
    print("contradiction count:", contradictions["count"])

    # invalidate: perturb a node and see the incremental report
    report = client.invalidate(path, "a")
    print("invalidated:", report["invalidated"])
    print("changes:", report["changes"])
```

## Running the bundled demo and tests

```bash
# end-to-end demonstration (writes a temp module, drives the CLI)
python3 bindings/python/examples/demo.py

# tests — skip gracefully if the binary cannot be located
python3 -m pytest bindings/python/tests
# or, with no pytest installed:
python3 bindings/python/tests/test_bridge.py
```
