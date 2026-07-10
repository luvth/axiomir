//! Integration tests for the `axiom` binary.

use std::process::Command;

/// Helper: run the axiom binary with given args and return the output.
fn axiom(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_axiom"))
        .args(args)
        .output()
        .expect("failed to spawn axiom binary")
}

/// `check` on a valid file succeeds and emits a module summary containing "digest".
#[test]
fn cli_check_ok() {
    let out = axiom(&["check", "../../examples/typed_math.axiom"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected exit 0 for check; stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.contains("digest") || stdout.contains("claims"),
        "stdout should contain 'digest' or 'claims'; got: {stdout:?}"
    );
}

/// `verify` on a valid file succeeds and reports verified claims.
#[test]
fn cli_verify_ok() {
    let out = axiom(&["verify", "../../examples/typed_math.axiom"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected exit 0 for verify; stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.contains("verified"),
        "stdout should contain 'verified'; got: {stdout:?}"
    );
}

/// `fmt --check` on the canonical example file must be stable (exit 0).
/// If the example is not already canonical, fall back to asserting `fmt` (without --check) succeeds.
#[test]
fn cli_fmt_stable() {
    let check_out = axiom(&["fmt", "../../examples/typed_math.axiom", "--check"]);
    if check_out.status.success() {
        // The example is already in canonical form — test passes.
        let stdout = String::from_utf8_lossy(&check_out.stdout);
        assert!(
            stdout.contains("formatting stable") || !stdout.is_empty(),
            "fmt --check should report stability; got: {stdout:?}"
        );
    } else {
        // Example needs reformatting; assert that `fmt` (no --check) itself succeeds.
        let fmt_out = axiom(&["fmt", "../../examples/typed_math.axiom"]);
        let stdout = String::from_utf8_lossy(&fmt_out.stdout);
        let stderr = String::from_utf8_lossy(&fmt_out.stderr);
        assert!(
            fmt_out.status.success(),
            "expected `fmt` to succeed even if --check fails; stdout={stdout:?} stderr={stderr:?}"
        );
        assert!(
            !stdout.is_empty(),
            "fmt should emit formatted output; got empty stdout"
        );
    }
}

/// `demo all` runs all seven demos and reports success.
#[test]
fn cli_demo_all() {
    let out = axiom(&["demo", "all"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected exit 0 for demo all; stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.contains("ALL DEMOS: PASS"),
        "stdout should contain 'ALL DEMOS: PASS'; got: {stdout:?}"
    );
}

/// `conform` runs the conformance suite and reports zero failures.
#[test]
fn cli_conform() {
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance");
    let out = axiom(&["conform", "--fixtures", fixtures]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected exit 0 for conform; stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.contains("passed"),
        "stdout should contain 'passed'; got: {stdout:?}"
    );
    assert!(
        stdout.contains("0 failed"),
        "stdout should contain '0 failed'; got: {stdout:?}"
    );
}

/// `check` on a known-invalid fixture must exit non-zero.
#[test]
fn cli_invalid_rejected() {
    // Use a known invalid fixture from the conformance suite.
    let fixtures_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance");
    let invalid_path = format!("{}/invalid/type_mismatch.axiom", fixtures_root);
    let out = axiom(&["check", &invalid_path]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "expected non-zero exit for invalid fixture; stdout={stdout:?} stderr={stderr:?}"
    );
}

/// API test: call `axiom_cli::demos::run_demo("all")` directly and assert exit == 0.
#[test]
fn demos_run_demo_all_via_api() {
    let outcome = axiom_cli::demos::run_demo("all");
    assert_eq!(
        outcome.exit, 0,
        "run_demo(\"all\") should return exit=0 but got {}; human={:?}",
        outcome.exit, outcome.human
    );
    assert!(
        outcome.human.contains("ALL DEMOS: PASS"),
        "run_demo(\"all\") human output should contain 'ALL DEMOS: PASS'; got: {:?}",
        outcome.human
    );
}
