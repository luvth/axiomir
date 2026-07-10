"""Tests for the Axiom IR Python bridge.

These tests drive the real ``axiom`` CLI through the thin bridge. If the binary
cannot be located they skip (rather than fail) so the suite is usable in
environments without a build.

Run with pytest::

    python3 -m pytest bindings/python/tests

or directly, with no third-party packages required::

    python3 bindings/python/tests/test_bridge.py
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

# Allow running the test file directly (``python3 test_bridge.py``) by making
# the package importable regardless of the current working directory.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

try:  # pytest is optional at import time
    import pytest
except ImportError:  # pragma: no cover - exercised only without pytest
    pytest = None


class _Skipped(Exception):
    """Local stand-in for ``pytest.skip`` when pytest is unavailable."""


def _skip(msg: str):
    if pytest is not None:
        pytest.skip(msg)
    raise _Skipped(msg)


from axiom import Axiom, AxiomError, find_binary  # noqa: E402


MODULE_SOURCE = """\
module test_bridge "1"
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


def _make_client() -> Axiom:
    binary = find_binary()
    if binary is None:
        _skip("axiom binary not found; build with 'cargo build -p axiom-cli'")
    return Axiom(binary=binary)


def _write_module() -> Path:
    directory = Path(tempfile.mkdtemp())
    module = directory / "test_module.axiom"
    module.write_text(MODULE_SOURCE)
    return module


def test_locate_binary():
    binary = find_binary()
    # Either we can find it (and it is a real file) or it is None — never a
    # dangling/non-existent path.
    assert binary is None or Path(binary).is_file()


def test_doctor():
    client = _make_client()
    report = client.doctor()
    assert isinstance(report, dict)
    assert "healthy" in report
    assert report["healthy"] is True


def test_demo_all():
    client = _make_client()
    result = client.demo("all")
    assert isinstance(result, dict)
    assert result.get("all_passed") is True


def test_conform():
    client = _make_client()
    result = client.conform(fixtures=str(_repo_root() / "conformance"))
    assert result.get("failed") == 0
    assert result.get("passed") == result.get("total")


def test_explain_keys():
    client = _make_client()
    module = _write_module()
    result = client.explain(module, "out")
    for key in ("label", "status", "type", "value", "context", "derivation"):
        assert key in result, f"missing expected key: {key}"
    assert result["label"] == "out"
    assert result["status"] == "verified"


if __name__ == "__main__":
    # Minimal runner so the file works without pytest installed.
    import traceback

    tests = [
        v
        for k, v in sorted(globals().items())
        if k.startswith("test_") and callable(v)
    ]
    failed, skipped = 0, 0
    for test in tests:
        try:
            test()
            print(f"PASS {test.__name__}")
        except _Skipped as exc:
            skipped += 1
            print(f"SKIP {test.__name__}: {exc}")
        except Exception as exc:  # noqa: BLE001
            failed += 1
            print(f"FAIL {test.__name__}: {exc}")
            traceback.print_exc()
    print(f"\n{failed} failed, {skipped} skipped")
    sys.exit(1 if failed else 0)
