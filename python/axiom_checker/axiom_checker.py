"""Minimal *independent* Axiom IR checker (no Rust dependency).

This is a from-scratch Python re-implementation of a SUBSET of the Axiom IR
semantics. It exists to *independently* validate part of the conformance
corpus: it parses plain-text ``.axiom`` fixtures, executes them, and reports the
verified claim set, obligation states, contradiction count, or the error
substring — matching each fixture's ``.expect.json`` contract.

It deliberately does NOT use the Rust crates. It reads only the fixtures and
their expectations, so a bug in the Rust implementation cannot mask a bug here
(and vice versa).

Supported surface
------------------
``module``, ``evidence`` (with ``trust``/``provider``/``signature``), ``assert``,
``observe``, ``assume``, ``derive`` (``core.add``/``sub``/``mul``/``div``,
``core.qadd``/``qsub``/``qmul``/``qdiv``, ``core.eq``/``neq``/``lt``/``le``/``gt``/
``ge``, ``core.interval``, ``core.not``), ``verify``, ``require``, ``discharge``.

Out of scope (raises :class:`UnsupportedFeature` so the runner can skip those
fixtures): contexts (``branch``/``merge``/``ctx``), contradictions
(``contradict``), incremental invalidation (``invalidate``), attest/challenge,
and external calls + receipt replay (``call``).
"""

from __future__ import annotations

import re
from dataclasses import dataclass


class AxiomError(Exception):
    """A reasoning error — makes an INVALID fixture fail."""


class UnsupportedFeature(Exception):
    """A language feature this minimal checker does not implement (skip it)."""


@dataclass
class Obl:
    kind: str
    mandatory: bool
    state: str  # "pending" | "satisfied"


# (category, mandatory-obligation-kind). TypeCompat / DimensionalConsistency are
# advisory and auto-satisfied; only NumericBounds blocks verification.
_OP_INFO = {
    "core.add": ("num", None),
    "core.sub": ("num", None),
    "core.mul": ("num", None),
    "core.div": ("num", "NumericBounds"),
    "core.qadd": ("q", None),
    "core.qsub": ("q", None),
    "core.qmul": ("q", None),
    "core.qdiv": ("q", None),
    "core.eq": ("cmp", None),
    "core.neq": ("cmp", None),
    "core.lt": ("cmp", None),
    "core.le": ("cmp", None),
    "core.gt": ("cmp", None),
    "core.ge": ("cmp", None),
    "core.interval": ("interval", None),
    "core.not": ("not", None),
}


def _op_info(op: str):
    full = op if "." in op else "core." + op
    return _OP_INFO.get(full), full


def parse_value(s: str):
    s = s.strip()
    if s == "true":
        return ("bool", True)
    if s == "false":
        return ("bool", False)
    m = re.match(r'q\((-?\d+)\s+"([^"]*)"\)', s)
    if m:
        return ("quantity", int(m.group(1)), m.group(2))
    m = re.match(r'rat\((-?\d+)\s+(-?\d+)\)', s)
    if m:
        num, den = int(m.group(1)), int(m.group(2))
        if den == 0:
            raise AxiomError("rational with zero denominator")
        return ("num", num // den)
    if re.match(r'-?\d+$', s):
        return ("num", int(s))
    raise AxiomError(f"parse error: cannot parse value: {s}")


def parse_module(text: str):
    # Pre-scan: fixtures that use unsupported features (contexts, contradictions,
    # incremental invalidation, external calls) are reported as UnsupportedFeature
    # so the runner can SKIP them rather than parse-fail midway.
    _UNSUPPORTED = ("branch", "merge", "contradict", "invalidate",
                    "attest", "challenge", "call")
    stmts = []
    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        if line.startswith(_UNSUPPORTED):
            raise UnsupportedFeature(line)
        if line.startswith("module "):
            m = re.match(r'module\s+(\w+)\s+"([^"]*)"', line)
            if not m:
                raise AxiomError(f"parse error: expected module header, got: {line}")
            stmts.append(("module", m.group(1), m.group(2)))
        elif line.startswith("evidence "):
            m = re.match(r'evidence\s+(\w+)\s+"([^"]*)"\s+"([^"]*)"', line)
            if not m:
                raise AxiomError(f"parse error: expected evidence, got: {line}")
            label, media, content = m.group(1), m.group(2), m.group(3)
            tm = re.search(r'trust\s*=\s*(\w+)', line)
            trust = tm.group(1) if tm else "unverified"
            pm = re.search(r'provider\s*=\s*"([^"]*)"', line)
            provider = pm.group(1) if pm else None
            sm = re.search(r'signature\s*=\s*"([^"]*)"', line)
            signature = sm.group(1) if sm else None
            stmts.append(("evidence", label, media, content, trust, provider, signature))
        elif line.startswith("assert "):
            m = re.match(r'assert\s+(\w+)\s*=\s*(.+?)\s*:\s*(\w+)', line)
            if not m:
                raise AxiomError(f"parse error: expected assert, got: {line}")
            evs = re.search(r'evidence\s*\[([^\]]*)\]', line)
            evlist = [e.strip() for e in evs.group(1).split(",") if e.strip()] if evs else []
            stmts.append(("assert", m.group(1), m.group(2).strip(), m.group(3), evlist))
        elif line.startswith("observe "):
            m = re.match(r'observe\s+(\w+)\s*=\s*(.+?)\s*:\s*(\w+)(?:\s+evidence\s*\[([^\]]*)\])?', line)
            if not m:
                raise AxiomError(f"parse error: expected observe, got: {line}")
            evlist = [e.strip() for e in (m.group(4) or "").split(",") if e.strip()]
            stmts.append(("observe", m.group(1), m.group(2).strip(), m.group(3), evlist))
        elif line.startswith("assume "):
            m = re.match(r'assume\s+(\w+)\s*=\s*(.+?)\s*:\s*(\w+)\s+scope\s*"([^"]*)"', line)
            if not m:
                raise AxiomError(f"parse error: expected assume, got: {line}")
            stmts.append(("assume", m.group(1), m.group(2).strip(), m.group(3), m.group(4)))
        elif line.startswith("derive "):
            m = re.match(r'derive\s+(\w+)\s*=\s*(\w+)\s*\(([^)]*)\)\s*:\s*(\w+)', line)
            if not m:
                raise AxiomError(f"parse error: expected derive, got: {line}")
            args = [a.strip() for a in m.group(3).split(",") if a.strip()]
            stmts.append(("derive", m.group(1), m.group(2), args, m.group(4)))
        elif line.startswith("verify "):
            m = re.match(r'verify\s+(\w+)', line)
            if not m:
                raise AxiomError(f"parse error: expected verify, got: {line}")
            stmts.append(("verify", m.group(1)))
        elif line.startswith("require "):
            m = re.match(r'require\s+([\w-]+)\s+on\s+(\w+)', line)
            if not m:
                raise AxiomError(f"parse error: expected require, got: {line}")
            stmts.append(("require", m.group(1), m.group(2)))
        elif line.startswith("discharge "):
            m = re.match(r'discharge\s+(\w+)\s+by\s+(\w+)\s+as\s+(\w+)', line)
            if not m:
                raise AxiomError(f"parse error: expected discharge, got: {line}")
            stmts.append(("discharge", m.group(1), m.group(2), m.group(3)))
        elif line.startswith(("branch", "merge", "contradict", "invalidate",
                              "attest", "challenge", "call")):
            raise UnsupportedFeature(line)
        else:
            raise AxiomError(f"parse error: expected statement, got: {line}")
    return stmts


def _compare(a, b, kind):
    x = a[1] if a[0] in ("num", "quantity") else a[1]
    y = b[1] if b[0] in ("num", "quantity") else b[1]
    return {
        "core.eq": x == y, "core.neq": x != y,
        "core.lt": x < y, "core.le": x <= y,
        "core.gt": x > y, "core.ge": x >= y,
    }[kind]


def _do_derive(claims, label, op, args, typ):
    info, full_op = _op_info(op)
    if info is None:
        raise AxiomError(f"unknown operation {full_op}")
    cat, mand_kind = info
    vals = [claims[a]["value"] for a in args]

    if cat == "num":
        for v in vals:
            if v[0] != "num":
                raise AxiomError(f"type error: {full_op} expects numeric operands")
        a, b = vals[0][1], vals[1][1]
        if full_op == "core.add":
            r = a + b
        elif full_op == "core.sub":
            r = a - b
        elif full_op == "core.mul":
            r = a * b
        elif full_op == "core.div":
            if b == 0:
                raise AxiomError("division by zero")
            r = a // b
        out = ("num", r)
    elif cat == "q":
        for v in vals:
            if v[0] != "quantity":
                raise AxiomError(f"type error: {full_op} expects quantity operands")
        a, b = vals[0], vals[1]
        if full_op in ("core.qadd", "core.qsub"):
            if a[2] != b[2]:
                raise AxiomError(f"unit mismatch: {a[2]} vs {b[2]}")
            u = a[2]
            r = (a[1] + b[1]) if full_op == "core.qadd" else (a[1] - b[1])
        elif full_op == "core.qmul":
            u = f"{a[2]}*{b[2]}"
            r = a[1] * b[1]
        elif full_op == "core.qdiv":
            if b[1] == 0:
                raise AxiomError("division by zero")
            u = f"{a[2]}/{b[2]}"
            r = a[1] // b[1]
        out = ("quantity", r, u)
    elif cat == "cmp":
        for v in vals:
            if v[0] not in ("num", "quantity"):
                raise AxiomError(f"type error: {full_op} expects comparable operands")
        out = ("bool", _compare(vals[0], vals[1], full_op))
    elif cat == "interval":
        a, b = vals[0][1], vals[1][1]
        if a > b:
            raise AxiomError("interval lo > hi")
        out = ("interval", a, b)
    elif cat == "not":
        if vals[0][0] != "bool":
            raise AxiomError("not requires a bool operand")
        out = ("bool", not vals[0][1])
    else:  # pragma: no cover - defensive
        raise AxiomError(f"unsupported operation category: {cat}")

    obligations = [Obl("TypeCompat", mandatory=False, state="satisfied")]
    if mand_kind == "NumericBounds":
        obligations.append(Obl("NumericBounds", mandatory=True, state="pending"))
    elif mand_kind == "DimensionalConsistency":
        obligations.append(Obl("DimensionalConsistency", mandatory=False, state="satisfied"))
    claims[label] = {"value": out, "type": typ, "derived": True, "obligations": obligations}


def execute(stmts):
    claims = {}
    evidence = {}
    for s in stmts:
        if s[0] == "evidence":
            _, label, _media, _content, trust, provider, signature = s
            if trust == "trusted" and (not provider or not signature):
                raise AxiomError(
                    f"trusted evidence '{label}' requires a provider and a signature"
                )
            evidence[label] = {"trust": trust, "provider": provider, "signature": signature}
    for s in stmts:
        if s[0] in ("assert", "observe"):
            _, label, val, typ, evs = s
            for e in evs:
                if e not in evidence:
                    raise AxiomError(f"unknown evidence reference: {e}")
            claims[label] = {"value": parse_value(val), "type": typ,
                             "derived": False, "obligations": []}
        elif s[0] == "assume":
            _, label, val, typ, _scope = s
            claims[label] = {"value": parse_value(val), "type": typ,
                             "derived": False, "obligations": []}
    remaining = [s for s in stmts if s[0] == "derive"]
    while remaining:
        progressed = False
        for i, s in enumerate(remaining):
            _, label, op, args, typ = s
            if all(a in claims for a in args):
                _do_derive(claims, label, op, args, typ)
                remaining.pop(i)
                progressed = True
                break
        if not progressed:
            bad = remaining[0]
            missing = [a for a in bad[3] if a not in claims]
            raise AxiomError(f"unknown label: {missing[0] if missing else bad[1]}")
    for s in stmts:
        if s[0] == "require":
            _, kind, target = s
            claims[target]["obligations"].append(Obl(kind, mandatory=True, state="pending"))
        elif s[0] == "discharge":
            _, target, _by, stt = s
            if stt == "satisfied":
                for o in claims[target]["obligations"]:
                    if o.mandatory and o.state == "pending":
                        o.state = "satisfied"
    return claims


def verified_set(claims):
    out = []
    for label, c in claims.items():
        if not c["derived"]:
            continue
        mandatory = [o for o in c["obligations"] if o.mandatory]
        if all(o.state == "satisfied" for o in mandatory):
            out.append(label)
    return sorted(out)


def obligation_state(claims, label):
    c = claims[label]
    mandatory = [o for o in c["obligations"] if o.mandatory]
    if not mandatory:
        return "satisfied" if any(o.state == "satisfied" for o in c["obligations"]) else "none"
    return "satisfied" if all(o.state == "satisfied" for o in mandatory) else "pending"
