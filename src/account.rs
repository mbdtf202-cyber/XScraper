use crate::error::Result;
use crate::utils::{json_string_map, parse_json_string_map};
use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const BEARER_TOKEN: &str = "Bearer AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub username: String,
    pub password: String,
    pub email: String,
    pub email_password: String,
    pub user_agent: String,
    pub active: bool,
    pub locks: BTreeMap<String, DateTime<Utc>>,
    pub stats: BTreeMap<String, i64>,
    pub headers: BTreeMap<String, String>,
    pub cookies: BTreeMap<String, String>,
    pub mfa_code: Option<String>,
    pub proxy: Option<String>,
    pub error_msg: Option<String>,
    pub last_used: Option<DateTime<Utc>>,
}

impl Account {
    pub fn new(
        username: impl Into<String>,
        password: impl Into<String>,
        email: impl Into<String>,
        email_password: impl Into<String>,
        user_agent: Option<String>,
        proxy: Option<String>,
        cookies: BTreeMap<String, String>,
        mfa_code: Option<String>,
    ) -> Self {
        let active = cookies.contains_key("ct0");

        Self {
            username: username.into(),
            password: password.into(),
            email: email.into(),
            email_password: email_password.into(),
            user_agent: user_agent.unwrap_or_else(default_user_agent),
            active,
            locks: BTreeMap::new(),
            stats: BTreeMap::new(),
            headers: BTreeMap::new(),
            cookies,
            mfa_code,
            proxy,
            error_msg: None,
            last_used: None,
        }
    }

    pub fn cookie_header(&self) -> String {
        self.cookies
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn http_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        for (key, value) in &self.headers {
            if let (Ok(name), Ok(value)) =
                (HeaderName::from_bytes(key.as_bytes()), HeaderValue::from_str(value))
            {
                headers.insert(name, value);
            }
        }

        headers.insert("user-agent", HeaderValue::from_str(&self.user_agent).unwrap());
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert("authorization", HeaderValue::from_static(BEARER_TOKEN));
        headers.insert("x-twitter-active-user", HeaderValue::from_static("yes"));
        headers.insert("x-twitter-client-language", HeaderValue::from_static("en"));

        if !self.cookies.is_empty() {
            headers.insert("cookie", HeaderValue::from_str(&self.cookie_header()).unwrap());
        }
        if let Some(ct0) = self.cookies.get("ct0") {
            headers.insert("x-csrf-token", HeaderValue::from_str(ct0).unwrap());
        }

        Ok(headers)
    }

    pub(crate) fn locks_json(&self) -> Result<String> {
        let locks =
            self.locks.iter().map(|(key, value)| (key.clone(), value.to_rfc3339())).collect();
        json_string_map(&locks)
    }

    pub(crate) fn headers_json(&self) -> Result<String> {
        json_string_map(&self.headers)
    }

    pub(crate) fn cookies_json(&self) -> Result<String> {
        json_string_map(&self.cookies)
    }

    pub(crate) fn stats_json(&self) -> Result<String> {
        serde_json::to_string(&self.stats).map_err(Into::into)
    }

    pub(crate) fn parse_locks(raw: &str) -> Result<BTreeMap<String, DateTime<Utc>>> {
        let values = parse_json_string_map(raw)?;
        values
            .into_iter()
            .map(|(key, value)| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|dt| (key, dt.with_timezone(&Utc)))
                    .map_err(Into::into)
            })
            .collect()
    }

    pub(crate) fn parse_stats(raw: &str) -> Result<BTreeMap<String, i64>> {
        if raw.trim().is_empty() {
            return Ok(BTreeMap::new());
        }
        serde_json::from_str(raw).map_err(Into::into)
    }
}

pub fn default_user_agent() -> String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15".to_string()
}
