use serde_json::json;
use xscraper::evidence::{EvidenceRecorder, redact_json_value, redact_text};

#[test]
fn evidence_redaction_removes_cookie_auth_and_proxy_secrets() {
    let raw = "cookie: ct0=csrf; auth_token=secret\nauthorization: Bearer token\nproxy=http://user:pass@127.0.0.1:8080";

    let redacted = redact_text(raw);

    assert!(!redacted.contains("secret"));
    assert!(!redacted.contains("Bearer token"));
    assert!(!redacted.contains("user:pass"));
    assert!(redacted.contains("auth_token=<redacted>"));
    assert!(redacted.contains("authorization: <redacted>"));
}

#[test]
fn evidence_recorder_writes_manifest_and_redacted_json_payload() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = EvidenceRecorder::new(dir.path()).unwrap();

    let reference = recorder
        .record_json(
            "graphql-error",
            &json!({
                "operation": "SearchTimeline",
                "headers": {
                    "cookie": "ct0=csrf; auth_token=secret",
                    "x-rate-limit-remaining": "0"
                },
                "errors": [{"code": 88, "message": "Rate limit exceeded"}]
            }),
        )
        .unwrap();
    let manifest = recorder.finish(true).unwrap();

    let payload = std::fs::read_to_string(dir.path().join(&reference.path)).unwrap();
    assert!(payload.contains("SearchTimeline"));
    assert!(!payload.contains("secret"));
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(manifest.entries[0].kind, "graphql-error");
    assert!(manifest.ok);
}

#[test]
fn redact_json_value_handles_nested_sensitive_fields() {
    let redacted = redact_json_value(json!({
        "cookie": "ct0=csrf; auth_token=secret",
        "nested": {"authorization": "Bearer abc", "safe": "value"}
    }));

    assert_eq!(redacted["nested"]["safe"], "value");
    assert_eq!(redacted["nested"]["authorization"], "<redacted>");
    assert!(!redacted.to_string().contains("secret"));
}
