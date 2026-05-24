use crate::error::Result;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Proxy};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchProfile {
    #[serde(rename = "timeoutMs")]
    timeout_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(rename = "requestId", skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

impl Default for FetchProfile {
    fn default() -> Self {
        Self { timeout_ms: 30_000, proxy: None, headers: BTreeMap::new(), request_id: None }
    }
}

impl FetchProfile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_ms = timeout.as_millis().try_into().unwrap_or(u64::MAX);
        self
    }

    pub fn with_proxy(mut self, proxy: Option<String>) -> Self {
        self.proxy = proxy;
        self
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    pub fn proxy(&self) -> Option<&str> {
        self.proxy.as_deref()
    }

    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    pub fn client_for_base_url(&self, base_url: &str) -> Result<Client> {
        let mut headers = HeaderMap::new();
        for (name, value) in &self.headers {
            headers.insert(HeaderName::from_bytes(name.as_bytes())?, HeaderValue::from_str(value)?);
        }
        if let Some(request_id) = &self.request_id {
            headers.insert("x-xscraper-request-id", HeaderValue::from_str(request_id)?);
        }

        let mut builder = Client::builder()
            .timeout(self.timeout())
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::limited(10));
        if let Some(proxy) = self.proxy.as_ref()
            && should_apply_proxy(base_url)
        {
            builder = builder.proxy(Proxy::all(proxy)?);
        }
        Ok(builder.build()?)
    }
}

pub fn should_apply_proxy(base_url: &str) -> bool {
    Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
        .is_none_or(|host| !matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1"))
}
