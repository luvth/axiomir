#!/usr/bin/env python3
"""End-to-end demonstration of the stdlib-only Axiom IR Python bridge.

This script writes a small, valid Axiom module to a temporary file, then
exercises the bridge end to end:

  * ``check`` / ``verify`` / ``explain``
  * ``contradictions``
  * ``invalidate`` (printing the structured incremental report)
  * ``conform`` (printing the pass count)

It requires only the Python standard library and a compiled ``axiom`` CLI.
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

# Make the package importable regardless of where this script is invoked from.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from axiom import Axiom, AxiomError, find_binary  # noqa: E402


MODULE_SOURCE = """\
module demo_bridge "1"
assert a = 2 : rational
assert b = 3 : rational
derive out = add(a, b) : rational
verify out
"""


def _repo_root() -> Path:
    """Walk up from this file until a directory containing ``Cargo.toml``."""
    here = Path(__file__).resolve()
    for parent in [here, *here.parents]:
        if (parent / "Cargo.toml").is_file():
            return parent
    return Path.cwd()


def _print_section(title: str) -> None:
    print(f"\n== {title} ==")


def main() -> int:
    binary = find_binary()
    if binary is None:
        print(
            "Could not locate the 'axiom' binary. Build it with "
            "'cargo build -p axiom-cli' (or set AXIOM_BIN).",
            file=sys.stderr,
        )
        return 2

    client = Axiom(binary=binary)
    print(f"Using axiom binary: {client.binary}")

    with tempfile.TemporaryDirectory() as tmp:
        module = Path(tmp) / "demo.axiom"
        module.write_text(MODULE_SOURCE)

        _print_section("check")
        summary = client.check(module)
        print("verified claims:", summary.get("verified"))
        print("claim_count:", summary.get("claim_count"))
        print("contradiction_count:", summary.get("contradiction_count"))

        _print_section("verify")
        ver = client.verify(module)
        print("verified:", ver.get("verified"))
        print("not_verified:", ver.get("not_verified"))

        _print_section("explain(out)")
        expl = client.explain(module, "out")
        print("label:", expl.get("label"))
        print("status:", expl.get("status"))
        print("type:", expl.get("type"))
        print("value:", expl.get("value"))
        print("derivation:", expl.get("derivation"))

        _print_section("contradictions")
        contradictions = client.contradictions(module)
        print("count:", contradictions.get("count"))

        _print_section("invalidate(a)")
        report = client.invalidate(module, "a")
        if report is None:
            print("(no structured report returned for this node)")
        else:
            print("root:", report.get("root"))
            print("invalidated:", report.get("invalidated"))
            print("recomputed:", report.get("recomputed"))
            print("preserved_count:", report.get("preserved_count"))
            print("changes:")
            for change in report.get("changes", []):
                print(
                    f"  - {change.get('label')}: "
                    f"{change.get('from')} -> {change.get('to')} "
                    f"({change.get('reason')})"
                )

        _print_section("conform")
        conformance_dir = _repo_root() / "conformance"
        result = client.conform(fixtures=str(conformance_dir))
        print(
            f"conformance: {result.get('passed')}/{result.get('total')} "
            f"passed ({result.get('failed')} failed)"
        )

    print("\nDemo complete.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AxiomError as exc:
        print(f"axiom error: {exc}", file=sys.stderr)
        raise SystemExit(exc.exit_code or 1)
