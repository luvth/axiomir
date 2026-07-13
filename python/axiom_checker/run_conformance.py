"""Run the independent Python checker against part of the conformance corpus.

Usage:
    python python/axiom_checker/run_conformance.py [conformance_dir]

Reads ``<dir>/valid/*.axiom`` and ``<dir>/invalid/*.axiom`` (each paired with a
``<stem>.expect.json``), executes them with the independent checker, and asserts
the outcome matches the expectation. Fixtures that use unsupported features are
reported as SKIP (not failure): this checker intentionally covers a subset.

Exit code is non-zero if any supported fixture fails.
"""

from __future__ import annotations

import json
import os
import sys

from axiom_checker import (
    AxiomError,
    UnsupportedFeature,
    execute,
    obligation_state,
    parse_module,
    verified_set,
)

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_DIR = os.path.normpath(os.path.join(HERE, "..", "..", "conformance"))


def _load(path):
    with open(path) as f:
        return f.read()


def _check_valid(stem, path, expect):
    try:
        claims = execute(parse_module(_load(path)))
    except UnsupportedFeature as e:
        return "skip", f"unsupported: {e}"
    except AxiomError as e:
        return "fail", f"execution failed but expected success: {e}"

    problems = []
    if "verified" in expect:
        got = verified_set(claims)
        if got != sorted(expect["verified"]):
            problems.append(f"verified got {got} want {sorted(expect['verified'])}")
    if "obligations" in expect:
        for label, state in expect["obligations"].items():
            if label not in claims:
                problems.append(f"obligation probe: unknown claim {label}")
            elif obligation_state(claims, label) != state:
                problems.append(f"obligation probe: {label} state "
                                f"{obligation_state(claims, label)} != {state}")
    if "contradictions" in expect and expect["contradictions"] != 0:
        problems.append("contradictions: independent checker does not detect contradictions")
    if problems:
        return "fail", "; ".join(problems)
    return "pass", "ok"


def _check_invalid(stem, path, expect):
    try:
        execute(parse_module(_load(path)))
    except UnsupportedFeature as e:
        return "skip", f"unsupported: {e}"
    except AxiomError as e:
        msg = str(e)
        subs = expect.get("errors", [])
        missing = [s for s in subs if s not in msg]
        if missing:
            return "fail", f"error '{msg}' missing expected substrings {missing}"
        return "pass", f"rejected: {msg}"
    return "fail", "expected rejection but module executed"


def run(conformance_dir):
    results = []
    for kind, sub in (("valid", "valid"), ("invalid", "invalid")):
        d = os.path.join(conformance_dir, sub)
        if not os.path.isdir(d):
            continue
        for name in sorted(os.listdir(d)):
            if not name.endswith(".axiom"):
                continue
            stem = name[: -len(".axiom")]
            path = os.path.join(d, name)
            expect_path = os.path.join(d, stem + ".expect.json")
            try:
                expect = json.loads(_load(expect_path))
            except FileNotFoundError:
                results.append((stem, kind, "skip", "missing expectation"))
                continue
            if kind == "valid":
                status, detail = _check_valid(stem, path, expect)
            else:
                status, detail = _check_invalid(stem, path, expect)
            results.append((stem, kind, status, detail))
    return results


def main():
    conformance_dir = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DIR
    results = run(conformance_dir)
    passed = skipped = failed = 0
    width = max((len(r[0]) for r in results), default=0)
    for stem, kind, status, detail in results:
        mark = {"pass": "PASS", "skip": "SKIP", "fail": "FAIL"}[status]
        print(f"  [{mark}] {stem:<{width}} ({kind}) {detail}")
        if status == "pass":
            passed += 1
        elif status == "skip":
            skipped += 1
        else:
            failed += 1
    total = passed + failed
    print()
    print(f"independent Python checker: {passed} passed, {failed} failed, "
          f"{skipped} skipped (of {len(results)} fixtures)")
    if failed:
        print("RESULT: FAIL")
        sys.exit(1)
    print("RESULT: PASS (all supported fixtures agree with the reference expectations)")
    sys.exit(0)


if __name__ == "__main__":
    main()
