#!/usr/bin/env bash
# Axiom IR — ultra-short reproducible demo (< 1 minute after first build).
#
# Five beats in one flow:
#   1. the producer generates a module from a structured plan
#   2. a valid proof passes  (verified)
#   3. a falsified / incomplete proof fails  (obligation gate rejects it)
#   4. a premise change invalidates ONLY its dependents
#   5. replay recovers the SAME module digest (offline, no capabilities)
#
# First run builds the `axiom` binary (~30s). Later runs finish in a few seconds.
set -euo pipefail

cd "$(dirname "$0")/.."

AXIOM="target/release/axiom"
if [ ! -x "$AXIOM" ]; then
  echo "-- building axiom (one-time) --"
  cargo build --release -p axiom-cli
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

ok()   { printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad()  { printf '  \033[31m✗\033[0m %s\n' "$1"; }
line() { echo; }

# A plan: force (10 N) * dist (2 m) = work. Verifiable.
PLAN_VALID='{"module_name":"demo","premises":[{"label":"force","value":{"Quantity":[10,"N"]},"evidence":null},{"label":"dist","value":{"Quantity":[2,"m"]},"evidence":null}],"assumptions":[],"steps":[{"label":"work","op":"qmul","inputs":["force","dist"],"requires_obligation":false,"discharge_by":null,"verify":true}]}'

# A falsified plan: same math, but the step declares a mandatory obligation it
# never discharges — so the proof is incomplete and must be rejected.
PLAN_BAD='{"module_name":"demo","premises":[{"label":"force","value":{"Quantity":[10,"N"]},"evidence":null},{"label":"dist","value":{"Quantity":[2,"m"]},"evidence":null}],"assumptions":[],"steps":[{"label":"work","op":"qmul","inputs":["force","dist"],"requires_obligation":true,"discharge_by":null,"verify":true}]}'

START="$(date +%s)"

line
echo "== Beat 1 — producer generates the module =="
"$AXIOM" produce --plan "$PLAN_VALID" --output "$TMP/valid.axiom" >/dev/null
"$AXIOM" produce --plan "$PLAN_BAD"  --output "$TMP/bad.axiom"   >/dev/null
cat "$TMP/valid.axiom"
ok "module produced from a structured plan"

line
echo "== Beat 2 — a valid proof passes =="
OUT="$("$AXIOM" verify "$TMP/valid.axiom")"
echo "$OUT"
if echo "$OUT" | grep -q 'verified: work'; then
  ok "valid proof verified (work)"
else
  bad "valid proof was NOT verified"; exit 1
fi

line
echo "== Beat 3 — a falsified (incomplete) proof fails =="
OUT="$("$AXIOM" verify "$TMP/bad.axiom")"
echo "$OUT"
if echo "$OUT" | grep -q 'verified: *$'; then
  ok "falsified proof rejected: 'work' is NOT verified"
else
  bad "falsified proof unexpectedly verified"; exit 1
fi

line
echo "== Beat 4 — a premise change invalidates ONLY its dependents =="
OUT="$("$AXIOM" invalidate "$TMP/valid.axiom" force)"
echo "$OUT"
# The dependent 'work' must be invalidated then recomputed/verified; the premise
# 'force' and the independent 'dist' must stay untouched.
if echo "$OUT" | grep -q 'work: verified -> invalidated' \
   && echo "$OUT" | grep -q 'work: invalidated -> verified' \
   && echo "$OUT" | grep -q 'preserved'; then
  ok "force change invalidated 'work' (dependent) then recomputed; 'force'/'dist' untouched"
else
  bad "invalidation frontier incorrect"; exit 1
fi

line
echo "== Beat 5 — replay recovers the SAME digest =="
"$AXIOM" run --emit-receipts "$TMP/receipts.json" "$TMP/valid.axiom" >/dev/null
OUT="$("$AXIOM" replay "$TMP/receipts.json")"
echo "$OUT"
if echo "$OUT" | grep -q 'identical: *true'; then
  ok "replay digest == live digest (offline, capability-free)"
else
  bad "replay digest mismatch"; exit 1
fi

END="$(date +%s)"
line
echo "== ALL BEATS OK in $((END - START))s =="
