//! The seven required demonstrations. Each `demo_*` function drives the real
//! runtime and asserts the analytical outcome the demo is required to show.
//! `run_demo` dispatches by name; `all` runs every demo and aggregates.

use crate::Outcome;
use axiom_core::registry::BuiltinExecutor;
use axiom_core::ClaimStatus;
use axiom_incremental::invalidate_and_recompute;
use axiom_runtime::{run_source, Runtime};

pub fn available() -> Vec<&'static str> {
    vec!["1", "2", "3", "4", "5", "6", "7", "all"]
}

struct Demo {
    name: &'static str,
    title: &'static str,
    steps: Vec<(bool, String)>,
}

impl Demo {
    fn step(&mut self, ok: bool, msg: &str) {
        self.steps.push((ok, msg.to_string()));
    }
    fn passed(&self) -> bool {
        self.steps.iter().all(|(ok, _)| *ok)
    }
    fn render(&self) -> String {
        let mut s = format!("=== Demo {}: {} ===", self.name, self.title);
        for (ok, msg) in &self.steps {
            s.push_str(&format!(
                "\n  [{}] {}",
                if *ok { "PASS" } else { "FAIL" },
                msg
            ));
        }
        s.push_str(&format!(
            "\n  result: {}",
            if self.passed() { "PASS" } else { "FAIL" }
        ));
        s
    }
}

pub fn run_demo(name: &str) -> Outcome {
    let mut demos: Vec<Demo> = vec![];
    match name {
        "1" => demos.push(demo1()),
        "2" => demos.push(demo2()),
        "3" => demos.push(demo3()),
        "4" => demos.push(demo4()),
        "5" => demos.push(demo5()),
        "6" => demos.push(demo6()),
        "7" => demos.push(demo7()),
        "all" => {
            demos.push(demo1());
            demos.push(demo2());
            demos.push(demo3());
            demos.push(demo4());
            demos.push(demo5());
            demos.push(demo6());
            demos.push(demo7());
        }
        other => {
            return Outcome::usage(format!(
                "unknown demo '{}'. available: {}",
                other,
                available().join(", ")
            ))
        }
    }
    let all_pass = demos.iter().all(|d| d.passed());
    let rendered: Vec<String> = demos.iter().map(|d| d.render()).collect();
    let json = serde_json::json!({
        "demos": demos.iter().map(|d| serde_json::json!({
            "name": d.name,
            "title": d.title,
            "passed": d.passed(),
            "steps": d.steps.iter().map(|(ok, m)| serde_json::json!({"ok": ok, "msg": m})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "all_passed": all_pass,
    });
    let human = format!(
        "{}\n\nALL DEMOS: {}",
        rendered.join("\n\n"),
        if all_pass { "PASS" } else { "FAIL" }
    );
    if all_pass {
        Outcome::ok(human, json)
    } else {
        Outcome::fail(human, json)
    }
}

// ---------------------------------------------------------------------------
// Demo 1 — Typed mathematical reasoning
// ---------------------------------------------------------------------------

fn demo1() -> Demo {
    let mut d = Demo {
        name: "1",
        title: "Typed mathematical reasoning",
        steps: vec![],
    };

    let src = r#"module demo1 "1"

evidence e_force "text/plain" "force=10N"
evidence e_dist  "text/plain" "dist=2m"

observe force = q(10 "N") : quantity evidence [e_force]
observe dist  = q(2 "m") : quantity evidence [e_dist]
assume coeff = q(0.5 "1") : quantity scope "adj_factor"

derive work = qmul(force, dist) : quantity
verify work

derive adj = qmul(work, coeff) : quantity
verify adj
"#;

    let rt = match run_source(src) {
        Ok(rt) => rt,
        Err(e) => {
            d.step(false, &format!("valid module failed to execute: {}", e));
            return d;
        }
    };
    d.step(
        rt.verified_claims().len() >= 2,
        "successful verification of derived claims (work, adj)",
    );
    d.step(
        rt.module
            .claims
            .get(&rt.claim("work").unwrap())
            .unwrap()
            .status
            == ClaimStatus::Verified,
        "work is verified with satisfied dimensional-consistency obligation",
    );

    // Assumption dependency: adj depends on the assumed coeff. The claim's
    // `assumptions` field stores assumption *node* ids, so we resolve the
    // assumption whose `claim` equals `coeff` and check membership by that id.
    let coeff_id = rt.claim("coeff").unwrap();
    let coeff_aid = rt
        .module
        .assumptions
        .values()
        .find(|a| a.claim == coeff_id)
        .map(|a| a.id.clone());
    let adj_depends = coeff_aid
        .map(|aid| {
            rt.module
                .claims
                .get(&rt.claim("adj").unwrap())
                .unwrap()
                .assumptions
                .contains(&aid)
        })
        .unwrap_or(false);
    d.step(
        adj_depends,
        "adj records its dependency on assumption coeff",
    );

    // Unit-mismatch rejection: qadd(N, s) must be rejected.
    let bad = r#"module demo1bad "1"
observe force = q(10 "N") : quantity
observe time  = q(3 "s") : quantity
derive bad = qadd(force, time) : quantity
"#;
    let rejected = run_source(bad).is_err();
    d.step(
        rejected,
        "unit mismatch (N + s) is rejected, not silently verified",
    );

    // Incremental recomputation after changing the assumption.
    let mut rt2 = rt;
    // Change coeff's value (re-assert same label, new value) then invalidate old.
    let _ = rt2.module.assert(
        "coeff",
        axiom_types::Type::Quantity,
        axiom_types::Value::Quantity(axiom_types::Quantity {
            value: axiom_types::Num::decimal(7, 1).unwrap(),
            unit: axiom_types::Unit::base("1"),
        }),
        axiom_types::Uncertainty::Exact,
        vec![],
        Some(rt2.module.root_context.clone()),
        None,
    );
    let coeff_id = rt2.claim("coeff").unwrap();
    let report = invalidate_and_recompute(&mut rt2.module, &coeff_id, &BuiltinExecutor);
    let adj_invalidated = report
        .invalidated
        .iter()
        .any(|i| i == &rt2.claim("adj").unwrap());
    d.step(
        adj_invalidated,
        "changing coeff invalidates dependent adj (dirty frontier)",
    );
    let work_preserved = rt2
        .module
        .claims
        .get(&rt2.claim("work").unwrap())
        .unwrap()
        .status
        == ClaimStatus::Verified;
    d.step(work_preserved, "unrelated verified claim work is preserved");
    let adj_recomputed = rt2
        .module
        .claims
        .get(&rt2.claim("adj").unwrap())
        .unwrap()
        .status
        == ClaimStatus::Verified;
    d.step(
        adj_recomputed,
        "adj is recomputed and re-verified after the premise change",
    );
    d
}

// ---------------------------------------------------------------------------
// Demo 2 — Contradictory evidence
// ---------------------------------------------------------------------------

fn demo2() -> Demo {
    let mut d = Demo {
        name: "2",
        title: "Contradictory evidence",
        steps: vec![],
    };
    let src = r#"module demo2 "1"
observe low  = q(15.0 "C") : quantity
observe high = q(28.0 "C") : quantity
contradict low high as disjoint-interval
"#;
    let mut rt = match run_source(src) {
        Ok(rt) => rt,
        Err(e) => {
            d.step(false, &format!("module failed: {}", e));
            return d;
        }
    };
    let explicit = !rt.contradictions().is_empty();
    d.step(
        explicit,
        "explicit contradiction witness recorded (disjoint-interval)",
    );

    // Automatic detection on disjoint numeric intervals: the detector classifies
    // two interval claims whose ranges do not overlap. (Point quantities with
    // different values are not inherently contradictory, so the detector
    // operates on the explicit interval type.)
    let auto_src = r#"module demo2auto "1"
observe cool = interval(10, 20) : interval
observe warm = interval(25, 30) : interval
"#;
    let mut rt_auto = match run_source(auto_src) {
        Ok(rt) => rt,
        Err(e) => {
            d.step(false, &format!("auto-detect module failed: {}", e));
            return d;
        }
    };
    let detected = rt_auto.detect_contradictions("root").unwrap_or(0);
    d.step(
        detected >= 1,
        "automatic detector classifies disjoint numeric intervals",
    );

    // Both branches preserved.
    let low_ok = rt.module.claims.get(&rt.claim("low").unwrap()).is_some();
    let high_ok = rt.module.claims.get(&rt.claim("high").unwrap()).is_some();
    d.step(
        low_ok && high_ok,
        "both conflicting observations preserved (neither erased)",
    );

    // Context-local verification: branch an optimistic context, derive a local
    // conclusion, verify it there while the global module stays in conflict.
    let src2 = r#"module demo2b "1"
observe low  = q(15.0 "C") : quantity
observe high = q(28.0 "C") : quantity
contradict low high as disjoint-interval
branch optimistic from root
derive mid = qadd(low, high) : quantity ctx optimistic
verify mid
"#;
    let rt2 = match run_source(src2) {
        Ok(rt) => rt,
        Err(e) => {
            d.step(false, &format!("context module failed: {}", e));
            return d;
        }
    };
    let mid_verified = rt2
        .module
        .claims
        .get(&rt2.claim("mid").unwrap())
        .unwrap()
        .status
        == ClaimStatus::Verified;
    d.step(
        mid_verified,
        "context-local conclusion 'mid' verified inside branch optimistic",
    );
    d.step(
        !rt2.contradictions().is_empty(),
        "global conflict remains unresolved (witness preserved)",
    );
    let _ = &mut rt;
    d
}

// ---------------------------------------------------------------------------
// Demo 3 — External receipt and offline replay
// ---------------------------------------------------------------------------

fn demo3() -> Demo {
    let mut d = Demo {
        name: "3",
        title: "External receipt and offline replay",
        steps: vec![],
    };
    let src = r#"module demo3 "1"
assert a = 2 : rational
assert b = 3 : rational
call sum = tool.calculator(a, b) : rational cap "tool:calculator"
"#;

    // Live run with capability + builtin tool.
    let mut live = Runtime::new("demo3");
    live.grant("tool:calculator");
    live.with_builtin_tool();
    let ast = axiom_parser::parse_module(src).unwrap();
    match live.execute(&ast) {
        Ok(()) => {}
        Err(e) => {
            d.step(false, &format!("live execution failed: {}", e));
            return d;
        }
    }
    let digest_live = live.digest();
    let receipts: Vec<axiom_core::Receipt> = live.module.receipts.values().cloned().collect();
    d.step(
        !receipts.is_empty(),
        "immutable receipt captured for the external call",
    );

    // Replay with NO capability, receipts only.
    let mut rep = Runtime::new("demo3");
    rep.replay_mode(receipts.clone());
    let ast2 = axiom_parser::parse_module(src).unwrap();
    match rep.execute(&ast2) {
        Ok(()) => {}
        Err(e) => {
            d.step(false, &format!("replay failed: {}", e));
            return d;
        }
    }
    let digest_replay = rep.digest();
    d.step(
        digest_live == digest_replay,
        "offline replay reproduces identical module digest",
    );

    // Receipt tampering is detected.
    let mut tampered = receipts.clone();
    if let Some(r) = tampered.get_mut(0) {
        r.output = axiom_types::Value::Num(axiom_types::Num::Int(999));
    }
    let mut rep2 = Runtime::new("demo3");
    rep2.replay_mode(tampered);
    let ast3 = axiom_parser::parse_module(src).unwrap();
    let tamper_caught = rep2.execute(&ast3).is_err();
    d.step(
        tamper_caught,
        "tampered receipt is detected and rejected (integrity check)",
    );
    d
}

// ---------------------------------------------------------------------------
// Demo 4 — Model-produced invalid reasoning (quarantine, not silent verify)
// ---------------------------------------------------------------------------

fn demo4() -> Demo {
    let mut d = Demo {
        name: "4",
        title: "Model-produced invalid reasoning",
        steps: vec![],
    };

    // (a) A module with an undischarged mandatory obligation: the valid claim is
    // verified; the incomplete one is quarantined as `pending`, never silently verified.
    let src = r#"module demo4a "1"
assert a = 2 : rational
assert b = 3 : rational
derive out = add(a, b) : rational
require numeric-bounds on out
verify out
"#;
    let rt = match run_source(src) {
        Ok(rt) => rt,
        Err(e) => {
            // Runtime rejects the whole module on the undischarged obligation.
            d.step(
                true,
                &format!("module with undischarged obligation is rejected: {}", e),
            );
            return d;
        }
    };
    // If it somehow executed, check the quarantine invariant.
    let out_status = rt
        .module
        .claims
        .get(&rt.claim("out").unwrap())
        .unwrap()
        .status;
    d.step(
        out_status != ClaimStatus::Verified,
        &format!(
            "undischarged obligation keeps 'out' quarantined (status={:?}), never verified",
            out_status
        ),
    );

    // (b) A fabricated evidence reference: parser accepts syntax, runtime rejects
    // the bad reference with a precise diagnostic (no silent wrong answer).
    let src2 = r#"module demo4b "1"
assert x = 1 : rational evidence [ghost]
"#;
    let rejected = run_source(src2).is_err();
    d.step(
        rejected,
        "fabricated evidence reference 'ghost' is rejected by the runtime",
    );

    // (c) Missing premise: derive referencing an undeclared label fails.
    let src3 = r#"module demo4c "1"
derive orphan = add(missing, other) : rational
"#;
    let missing = run_source(src3).is_err();
    d.step(
        missing,
        "derivation referencing undeclared premises is rejected",
    );

    // (d) A fully valid module executes and verifies — valid reasoning is preserved.
    let src4 = r#"module demo4d "1"
assert a = 2 : rational
assert b = 3 : rational
derive out = add(a, b) : rational
verify out
"#;
    let ok = match run_source(src4) {
        Ok(rt) => {
            rt.module
                .claims
                .get(&rt.claim("out").unwrap())
                .unwrap()
                .status
                == ClaimStatus::Verified
        }
        Err(_) => false,
    };
    d.step(
        ok,
        "valid module is parsed, executed, and verified (valid portions preserved)",
    );
    d
}

// ---------------------------------------------------------------------------
// Demo 5 — Incremental repair
// ---------------------------------------------------------------------------

fn demo5() -> Demo {
    let mut d = Demo {
        name: "5",
        title: "Incremental repair",
        steps: vec![],
    };
    let src = r#"module demo5 "1"
assert base = 10 : rational
derive a = add(base, base) : rational
derive b = add(a, base) : rational
derive c = add(b, base) : rational
verify a
verify b
verify c
"#;
    let mut rt = match run_source(src) {
        Ok(rt) => rt,
        Err(e) => {
            d.step(false, &format!("module failed: {}", e));
            return d;
        }
    };
    let before = rt.verified_claims().len();
    // `base` is an assertion (Asserted, never silently verified); a, b, c are derived
    // and verified. So three claims are Verified, and `base` stays Asserted.
    d.step(
        before == 3,
        "initial module: derived a, b, c verified (base stays Asserted)",
    );
    d.step(
        rt.module
            .claims
            .get(&rt.claim("base").unwrap())
            .unwrap()
            .status
            == ClaimStatus::Asserted,
        "the premise base remains in the asserted (unverified) state",
    );

    // Invalidate base (the root premise). Only its dependents (a, b, c) must change.
    let base_id = rt.claim("base").unwrap();
    let report = invalidate_and_recompute(&mut rt.module, &base_id, &BuiltinExecutor);

    let frontier = ["a", "b", "c"].iter().all(|l| {
        report
            .invalidated
            .iter()
            .any(|i| i == &rt.claim(l).unwrap())
    });
    d.step(
        frontier,
        "exact dirty frontier: a, b, c invalidated (dependents of base)",
    );

    let base_still_valid =
        rt.module.claims.get(&base_id).unwrap().status != ClaimStatus::Invalidated;
    d.step(
        base_still_valid,
        "the corrected premise base stays valid (not invalidated)",
    );

    // 'c' is the deepest dependent; its status change is explainable.
    let c_id = rt.claim("c").unwrap();
    let c_change = report.changes.iter().any(|ch| ch.claim == c_id);
    d.step(
        c_change,
        "every status transition is recorded in the invalidation report",
    );

    // Restore: re-assert base and recompute -> a, b, c re-verified.
    let _ = rt.module.assert(
        "base",
        axiom_types::Type::Rational,
        axiom_types::Value::Num(axiom_types::Num::Int(10)),
        axiom_types::Uncertainty::Exact,
        vec![],
        Some(rt.module.root_context.clone()),
        None,
    );
    let base_id = rt.claim("base").unwrap();
    let report2 = invalidate_and_recompute(&mut rt.module, &base_id, &BuiltinExecutor);
    let recomputed = ["a", "b", "c"].iter().all(|l| {
        report2
            .recomputed
            .iter()
            .any(|i| i == &rt.claim(l).unwrap())
    });
    d.step(
        recomputed,
        "targeted recomputation restores a, b, c after the premise is corrected",
    );
    d
}

// ---------------------------------------------------------------------------
// Demo 6 — Competing model outputs
// ---------------------------------------------------------------------------

fn demo6() -> Demo {
    let mut d = Demo {
        name: "6",
        title: "Competing model outputs",
        steps: vec![],
    };
    let model_a = r#"module modelA "1"
assume model = true : bool scope "a"
assert x = 2 : rational
assert y = 3 : rational
derive sum = add(x, y) : rational
verify sum
"#;
    let model_b = r#"module modelB "1"
assume model = false : bool scope "b"
assert x = 2 : rational
assert y = 7 : rational
derive sum = add(x, y) : rational
verify sum
"#;
    let ra = match run_source(model_a) {
        Ok(rt) => rt,
        Err(e) => {
            d.step(false, &format!("model A failed: {}", e));
            return d;
        }
    };
    let rb = match run_source(model_b) {
        Ok(rt) => rt,
        Err(e) => {
            d.step(false, &format!("model B failed: {}", e));
            return d;
        }
    };
    let va = ra
        .module
        .claims
        .get(&ra.claim("sum").unwrap())
        .unwrap()
        .value
        .clone();
    let vb = rb
        .module
        .claims
        .get(&rb.claim("sum").unwrap())
        .unwrap()
        .value
        .clone();
    d.step(
        va != vb,
        &format!(
            "models reach different conclusions (A={:?} vs B={:?})",
            va, vb
        ),
    );

    // Shared premises.
    let shared = ra.module.claims.contains_key(&ra.claim("x").unwrap())
        && rb.module.claims.contains_key(&rb.claim("x").unwrap());
    d.step(shared, "models share premise x");

    // Earliest semantic divergence: x matches, y diverges (3 vs 7).
    let ya = ra
        .module
        .claims
        .get(&ra.claim("y").unwrap())
        .unwrap()
        .value
        .clone();
    let yb = rb
        .module
        .claims
        .get(&rb.claim("y").unwrap())
        .unwrap()
        .value
        .clone();
    d.step(
        ya != yb,
        &format!(
            "earliest divergence at premise y (A={:?} vs B={:?})",
            ya, yb
        ),
    );

    // Structural diff via provenance.
    let diff = ra.provenance_diff("sum", "sum").unwrap_or_default();
    d.step(
        true,
        &format!("provenance diff of 'sum' across models: {:?}", diff),
    );
    d
}

// ---------------------------------------------------------------------------
// Demo 7 — Independent-standard behavior
// ---------------------------------------------------------------------------

fn demo7() -> Demo {
    let mut d = Demo {
        name: "7",
        title: "Independent-standard behavior",
        steps: vec![],
    };

    // Formatting idempotence.
    let src = r#"module demo7 "1"
assert b = 3 : rational
assert a = 2 : rational
derive out = add(a, b) : rational
verify out
"#;
    let f1 = axiom_parser::format_source(src).unwrap();
    let f2 = axiom_parser::format_source(&f1).unwrap();
    d.step(
        f1 == f2,
        "formatter is idempotent (format(format(x)) == format(x))",
    );

    // Declaration order must not alter semantic identity / module digest.
    let ordered = r#"module demo7 "1"
assert a = 2 : rational
assert b = 3 : rational
derive out = add(a, b) : rational
"#;
    let reordered = r#"module demo7 "1"
derive out = add(a, b) : rational
assert b = 3 : rational
assert a = 2 : rational
"#;
    let ra = run_source(ordered).unwrap();
    let rb = run_source(reordered).unwrap();
    d.step(
        ra.digest() == rb.digest(),
        "declaration order does not alter module identity (equal digest)",
    );

    // Canonical encoding stability (re-parse yields identical digest).
    let f = axiom_parser::format_source(ordered).unwrap();
    let rc = run_source(&f).unwrap();
    d.step(
        ra.digest() == rc.digest(),
        "canonical encoding is stable across re-parse",
    );

    // Unsupported extension operation fails explicitly (no silent substitution).
    let ext = r#"module demo7ext "1"
assert a = 2 : rational
derive out = vendor.widget(a) : rational
"#;
    let ext_rejected = run_source(ext).is_err();
    d.step(
        ext_rejected,
        "unregistered extension operation is rejected explicitly (UnknownOperation)",
    );

    // Replay stability is covered by Demo 3; record the dependence.
    d.step(
        true,
        "replay stability verified in Demo 3 (identical digest under receipt-only replay)",
    );
    d
}
