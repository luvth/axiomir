//! Conformance corpus and runner for Axiom IR.
//!
//! The runner loads fixtures from `<dir>/valid/*.axiom` and `<dir>/invalid/*.axiom`,
//! each paired with a `<stem>.expect.json` describing the expected outcome. It executes
//! each fixture with the reference runtime and compares the result against the pinned
//! expectations. An independent implementation reproduces the same outcomes from the
//! plain-text fixtures plus the JSON expectations, without reading Rust source.

use axiom_core::registry::BuiltinExecutor;
use axiom_core::ObligationState;
use axiom_incremental::invalidate_and_recompute;
use axiom_parser::parse_module;
use axiom_runtime::{run_source, Runtime};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct InvalidationExpect {
    root: String,
    invalidated: Vec<String>,
    preserved: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Expectation {
    /// Expected verified claim labels (exact set).
    verified: Option<Vec<String>>,
    /// Expected (claim label -> obligation state) probes.
    obligations: Option<BTreeMap<String, String>>,
    /// Expected contradiction count.
    contradictions: Option<usize>,
    /// Expected invalidation frontier for a changed root.
    invalidation: Option<InvalidationExpect>,
    /// If true, assert live digest == receipt-only replay digest.
    replay: Option<bool>,
    /// For invalid fixtures: substrings that must appear in the error.
    errors: Option<Vec<String>>,
    /// Optional pinned module digest.
    digest: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FixtureResult {
    pub name: String,
    pub kind: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConformanceReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub fixtures: Vec<FixtureResult>,
}

impl ConformanceReport {
    pub fn summary(&self) -> String {
        self.fixtures
            .iter()
            .filter(|f| !f.passed)
            .map(|f| format!("  FAIL {} ({}): {}", f.name, f.kind, f.detail))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn run(fixtures_dir: &str) -> ConformanceReport {
    let base = Path::new(fixtures_dir);
    let mut fixtures = vec![];

    // Valid fixtures.
    let valid_dir = base.join("valid");
    if let Ok(entries) = fs::read_dir(&valid_dir) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("axiom") {
                continue;
            }
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let expect_path = valid_dir.join(format!("{}.expect.json", stem));
            let expect: Expectation = match fs::read_to_string(&expect_path) {
                Ok(t) => match serde_json::from_str(&t) {
                    Ok(x) => x,
                    Err(err) => {
                        fixtures.push(FixtureResult {
                            name: stem,
                            kind: "valid".into(),
                            passed: false,
                            detail: format!("bad expectation json: {}", err),
                        });
                        continue;
                    }
                },
                Err(_) => {
                    fixtures.push(FixtureResult {
                        name: stem,
                        kind: "valid".into(),
                        passed: false,
                        detail: "missing expectation file".into(),
                    });
                    continue;
                }
            };
            fixtures.push(run_valid(&stem, &path, &expect));
        }
    }

    // Invalid fixtures.
    let invalid_dir = base.join("invalid");
    if let Ok(entries) = fs::read_dir(&invalid_dir) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("axiom") {
                continue;
            }
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let expect_path = invalid_dir.join(format!("{}.expect.json", stem));
            let expect: Expectation = match fs::read_to_string(&expect_path) {
                Ok(t) => serde_json::from_str(&t).unwrap_or(Expectation {
                    verified: None,
                    obligations: None,
                    contradictions: None,
                    invalidation: None,
                    replay: None,
                    errors: None,
                    digest: None,
                }),
                Err(_) => Expectation {
                    verified: None,
                    obligations: None,
                    contradictions: None,
                    invalidation: None,
                    replay: None,
                    errors: None,
                    digest: None,
                },
            };
            fixtures.push(run_invalid(&stem, &path, &expect));
        }
    }

    let total = fixtures.len();
    let passed = fixtures.iter().filter(|f| f.passed).count();
    let failed = total - passed;
    ConformanceReport {
        total,
        passed,
        failed,
        fixtures,
    }
}

fn run_valid(stem: &str, path: &Path, expect: &Expectation) -> FixtureResult {
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return FixtureResult {
                name: stem.into(),
                kind: "valid".into(),
                passed: false,
                detail: format!("read: {}", e),
            }
        }
    };

    // Replay fixture: live run with capability + builtin tool, then receipt-only replay.
    if expect.replay == Some(true) {
        return run_replay_fixture(stem, &src, expect);
    }

    let mut rt = match run_source(&src) {
        Ok(rt) => rt,
        Err(e) => {
            return FixtureResult {
                name: stem.into(),
                kind: "valid".into(),
                passed: false,
                detail: format!("execution failed: {}", e),
            }
        }
    };
    let mut problems = vec![];

    if let Some(want) = &expect.verified {
        let got: Vec<String> = rt
            .verified_claims()
            .iter()
            .filter_map(|id| rt.module.claims.get(id).map(|c| c.label.clone()))
            .collect();
        if &got != want {
            problems.push(format!("verified set: got {:?} want {:?}", got, want));
        }
    }
    if let Some(want) = expect.contradictions {
        if rt.contradictions().len() != want {
            problems.push(format!(
                "contradictions: got {} want {}",
                rt.contradictions().len(),
                want
            ));
        }
    }
    if let Some(probes) = &expect.obligations {
        for (label, state) in probes {
            let id = match rt.claim(label) {
                Ok(id) => id,
                Err(_) => {
                    problems.push(format!("obligation probe: unknown claim {}", label));
                    continue;
                }
            };
            let c = &rt.module.claims[&id];
            let has = c.obligations.iter().any(|oid| {
                rt.module
                    .obligations
                    .get(oid)
                    .map(|o| state_str(o.state) == *state)
                    .unwrap_or(false)
            });
            if !has {
                problems.push(format!(
                    "obligation probe: {} has no obligation in state {}",
                    label, state
                ));
            }
        }
    }
    if let Some(inv) = &expect.invalidation {
        let root = match rt.claim(&inv.root) {
            Ok(id) => id,
            Err(_) => {
                problems.push(format!("invalidation root {} unknown", inv.root));
                return finish(stem, problems);
            }
        };
        let report = invalidate_and_recompute(&mut rt.module, &root, &BuiltinExecutor, &[]);
        let invalidated_labels: Vec<String> = report
            .invalidated
            .iter()
            .filter_map(|id| rt.module.claims.get(id).map(|c| c.label.clone()))
            .collect();
        let preserved_labels: Vec<String> = report
            .preserved
            .iter()
            .filter_map(|id| rt.module.claims.get(id).map(|c| c.label.clone()))
            .collect();
        for want in &inv.invalidated {
            if !invalidated_labels.contains(want) {
                problems.push(format!(
                    "invalidation: expected {} invalidated, got {:?}",
                    want, invalidated_labels
                ));
            }
        }
        for want in &inv.preserved {
            if !preserved_labels.contains(want) {
                problems.push(format!(
                    "invalidation: expected {} preserved, got {:?}",
                    want, preserved_labels
                ));
            }
        }
    }
    if let Some(want_digest) = &expect.digest {
        let got = rt.digest().as_str();
        if &got != want_digest {
            problems.push(format!("digest: got {} want {}", got, want_digest));
        }
    }
    finish(stem, problems)
}

fn run_replay_fixture(stem: &str, src: &str, expect: &Expectation) -> FixtureResult {
    let ast = match parse_module(src) {
        Ok(a) => a,
        Err(d) => {
            return FixtureResult {
                name: stem.into(),
                kind: "valid".into(),
                passed: false,
                detail: format!(
                    "parse: {:?}",
                    d.iter().map(|x| &x.message).collect::<Vec<_>>()
                ),
            }
        }
    };
    let mut live = Runtime::new("replay");
    live.grant("tool:calculator");
    live.with_builtin_tool();
    if let Err(e) = live.execute(&ast) {
        return FixtureResult {
            name: stem.into(),
            kind: "valid".into(),
            passed: false,
            detail: format!("live exec: {}", e),
        };
    }
    let digest_live = live.digest();
    let receipts: Vec<axiom_core::Receipt> = live.module.receipts.values().cloned().collect();
    let mut rep = Runtime::new("replay");
    rep.replay_mode(receipts);
    if let Err(e) = rep.execute(&ast) {
        return FixtureResult {
            name: stem.into(),
            kind: "valid".into(),
            passed: false,
            detail: format!("replay exec: {}", e),
        };
    }
    let mut problems = vec![];
    if digest_live != rep.digest() {
        problems.push("replay digest differs from live digest".into());
    }
    if let Some(want) = &expect.verified {
        let got: Vec<String> = rep
            .verified_claims()
            .iter()
            .filter_map(|id| rep.module.claims.get(id).map(|c| c.label.clone()))
            .collect();
        if &got != want {
            problems.push(format!("verified set: got {:?} want {:?}", got, want));
        }
    }
    finish(stem, problems)
}

fn run_invalid(stem: &str, path: &Path, expect: &Expectation) -> FixtureResult {
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return FixtureResult {
                name: stem.into(),
                kind: "invalid".into(),
                passed: false,
                detail: format!("read: {}", e),
            }
        }
    };
    let result = run_source(&src);
    match result {
        Ok(_) => FixtureResult {
            name: stem.into(),
            kind: "invalid".into(),
            passed: false,
            detail: "expected rejection but module executed".into(),
        },

        Err(e) => {
            let msg = e.to_string();
            if let Some(subs) = &expect.errors {
                let missing: Vec<&String> = subs.iter().filter(|s| !msg.contains(*s)).collect();
                if missing.is_empty() {
                    FixtureResult {
                        name: stem.into(),
                        kind: "invalid".into(),
                        passed: true,
                        detail: format!("rejected: {}", msg),
                    }
                } else {
                    FixtureResult {
                        name: stem.into(),
                        kind: "invalid".into(),
                        passed: false,
                        detail: format!(
                            "error '{}' missing expected substrings {:?}",
                            msg, missing
                        ),
                    }
                }
            } else {
                FixtureResult {
                    name: stem.into(),
                    kind: "invalid".into(),
                    passed: true,
                    detail: format!("rejected: {}", msg),
                }
            }
        }
    }
}

fn finish(stem: &str, problems: Vec<String>) -> FixtureResult {
    if problems.is_empty() {
        FixtureResult {
            name: stem.into(),
            kind: "valid".into(),
            passed: true,
            detail: "ok".into(),
        }
    } else {
        FixtureResult {
            name: stem.into(),
            kind: "valid".into(),
            passed: false,
            detail: problems.join("; "),
        }
    }
}

fn state_str(s: ObligationState) -> &'static str {
    match s {
        ObligationState::Pending => "pending",
        ObligationState::Satisfied => "satisfied",
        ObligationState::Failed => "failed",
        ObligationState::Waived => "waived",
        ObligationState::Unsupported => "unsupported",
    }
}
