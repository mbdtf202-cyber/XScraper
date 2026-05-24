use crate::error::{Result, XScraperError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceManifest {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    pub ok: bool,
    pub entries: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub sha256: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct EvidenceRecorder {
    root: PathBuf,
    run_id: String,
    entries: Mutex<Vec<EvidenceRef>>,
}

impl EvidenceRecorder {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let now = Utc::now();
        let run_id = format!("{}-{}", now.format("%Y%m%dT%H%M%S%.3fZ"), std::process::id());
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root).map_err(|source| XScraperError::io(&root, source))?;
        Ok(Self { root, run_id, entries: Mutex::new(Vec::new()) })
    }

    pub fn record_json(&self, kind: &str, value: &Value) -> Result<EvidenceRef> {
        let redacted = redact_json_value(value.clone());
        let bytes = serde_json::to_vec_pretty(&redacted)?;
        self.write_entry(kind, "json", &bytes)
    }

    pub fn record_text(&self, kind: &str, value: &str) -> Result<EvidenceRef> {
        self.write_entry(kind, "txt", redact_text(value).as_bytes())
    }

    pub fn finish(&self, ok: bool) -> Result<EvidenceManifest> {
        let manifest = EvidenceManifest {
            run_id: self.run_id.clone(),
            created_at: Utc::now(),
            ok,
            entries: self.entries.lock().map(|items| items.clone()).unwrap_or_default(),
        };
        let path = self.root.join("manifest.json");
        let bytes = serde_json::to_vec_pretty(&manifest)?;
        fs::write(&path, bytes).map_err(|source| XScraperError::io(path, source))?;
        Ok(manifest)
    }

    fn write_entry(&self, kind: &str, extension: &str, bytes: &[u8]) -> Result<EvidenceRef> {
        let id = format!(
            "{}-{}",
            sanitize_segment(kind),
            self.entries.lock().map(|v| v.len()).unwrap_or(0) + 1
        );
        let filename = format!("{id}.{extension}");
        let path = self.root.join(&filename);
        fs::write(&path, bytes).map_err(|source| XScraperError::io(&path, source))?;
        let sha256 = hex_sha256(bytes);
        let reference = EvidenceRef {
            id,
            kind: kind.to_string(),
            path: filename,
            sha256,
            created_at: Utc::now(),
        };
        if let Ok(mut entries) = self.entries.lock() {
            entries.push(reference.clone());
        }
        Ok(reference)
    }
}

pub fn redact_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let key_lc = key.to_ascii_lowercase();
                    let value = if is_sensitive_key(&key_lc) {
                        Value::String("<redacted>".into())
                    } else {
                        redact_json_value(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_json_value).collect()),
        Value::String(value) => Value::String(redact_text(&value)),
        other => other,
    }
}

pub fn redact_text(raw: &str) -> String {
    raw.lines().map(redact_line).collect::<Vec<_>>().join("\n")
}

fn redact_line(line: &str) -> String {
    let trimmed = line.trim_start();
    let prefix_len = line.len() - trimmed.len();
    let prefix = &line[..prefix_len];
    let lower = trimmed.to_ascii_lowercase();

    if lower.starts_with("authorization:") {
        return "authorization: <redacted>".into();
    }
    if lower.starts_with("authorization=") {
        return "authorization=<redacted>".into();
    }

    let mut value = redact_proxy_tokens(trimmed);
    value = redact_named_secret(&value, "auth_token=");
    value = redact_named_secret(&value, "ct0=");
    if value.to_ascii_lowercase().starts_with("bearer ") {
        value = "Bearer <redacted>".into();
    }

    if prefix.is_empty() { value } else { format!("{prefix}{value}") }
}

fn redact_named_secret(raw: &str, marker: &str) -> String {
    let mut rest = raw;
    let mut output = String::new();
    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(idx) = lower.find(marker) else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..idx]);
        output.push_str(marker);
        output.push_str("<redacted>");
        let value_start = idx + marker.len();
        let tail = &rest[value_start..];
        let value_end = tail
            .find(|ch: char| ch == ';' || ch == '&' || ch.is_whitespace())
            .unwrap_or(tail.len());
        rest = &tail[value_end..];
    }
    output
}

fn redact_proxy_tokens(raw: &str) -> String {
    raw.split_whitespace().map(redact_proxy_token).collect::<Vec<_>>().join(" ")
}

fn redact_proxy_token(token: &str) -> String {
    let Some(scheme_end) = token.find("://") else {
        return token.to_string();
    };
    let Some(at) = token[scheme_end + 3..].find('@').map(|idx| idx + scheme_end + 3) else {
        return token.to_string();
    };
    format!("{}<redacted>@{}", &token[..scheme_end + 3], &token[at + 1..])
}

fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key,
        "authorization"
            | "cookie"
            | "cookies"
            | "set-cookie"
            | "auth_token"
            | "ct0"
            | "x-csrf-token"
            | "password"
            | "email_password"
            | "proxy"
    )
}

fn sanitize_segment(raw: &str) -> String {
    raw.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_ascii_lowercase()
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
