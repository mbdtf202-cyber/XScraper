use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use tempfile::tempdir;
use xscraper::gql::OP_SEARCH_TIMELINE;

#[test]
fn doctor_report_outputs_machine_readable_health_and_xclid_state() {
    let dir = tempdir().unwrap();
    let assert = Command::cargo_bin("xscraper")
        .unwrap()
        .current_dir(dir.path())
        .args(["--db", "accounts.db", "doctor", "report", "--json"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let report: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["ok"], false);
    assert!(report["accountPool"]["accounts"].is_array());
    assert_eq!(report["xclid"]["failureStage"], "chunk-map");
}

#[test]
fn doctor_browser_drift_compares_saved_cdp_events() {
    let dir = tempdir().unwrap();
    let events = dir.path().join("events.json");
    std::fs::write(
        &events,
        format!(
            r#"[{{"params":{{"request":{{"url":"https://x.com/i/api/graphql/{OP_SEARCH_TIMELINE}?variables=%7B%22rawQuery%22%3A%22rust%22%2C%22product%22%3A%22Latest%22%7D","method":"GET"}}}}}}]"#
        ),
    )
    .unwrap();

    Command::cargo_bin("xscraper")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "doctor",
            "browser-drift",
            "--events",
            events.to_str().unwrap(),
            "--operation",
            "search",
            "--target",
            "rust",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}
