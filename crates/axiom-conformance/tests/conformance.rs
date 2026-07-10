//! Integration test: run the full conformance suite and assert zero failures.

#[test]
fn conformance_suite_passes() {
    let fixtures_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance");
    let report = axiom_conformance::run(fixtures_dir);

    // Emit a summary of any failures so the output is diagnosable.
    if report.failed > 0 {
        eprintln!(
            "conformance FAILED: {}/{} passed ({} failed)\n{}",
            report.passed,
            report.total,
            report.failed,
            report.summary()
        );
    }

    assert!(
        report.passed > 0,
        "expected at least one passing fixture, but passed={}",
        report.passed
    );
    assert_eq!(
        report.failed,
        0,
        "{} fixture(s) failed (total={}, passed={})\n{}",
        report.failed,
        report.total,
        report.passed,
        report.summary()
    );
    assert_eq!(
        report.total,
        report.passed,
        "total={} != passed={} (failed={})\n{}",
        report.total,
        report.passed,
        report.failed,
        report.summary()
    );
}
