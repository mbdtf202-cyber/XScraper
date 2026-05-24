use crate::account::Account;
use crate::error::{Result, XScraperError};
use crate::fetch_profile::FetchProfile;
use crate::pool::AccountsPool;
use crate::storage::AccountEventInput;
use crate::xclid::XClientTransactionIdGenerator;
use chrono::Utc;
use reqwest::{Client, Method, StatusCode};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use url::Url;

#[derive(Debug, Clone)]
pub struct QueueClient {
    pool: AccountsPool,
    queue: String,
    proxy: Option<String>,
    base_url: String,
    timeout: std::time::Duration,
    xclid: Arc<Mutex<BTreeMap<String, XClientTransactionIdGenerator>>>,
}

#[derive(Debug)]
struct RequestContext {
    account: Account,
    client: Client,
    proxy: Option<String>,
    req_count: i64,
}

#[derive(Debug)]
pub struct QueueResponse {
    pub account_username: String,
    pub status: StatusCode,
    pub headers: reqwest::header::HeaderMap,
    pub value: Value,
}

impl QueueClient {
    pub fn new(pool: AccountsPool, queue: impl Into<String>) -> Self {
        Self {
            pool,
            queue: queue.into(),
            proxy: None,
            base_url: "https://x.com".into(),
            timeout: std::time::Duration::from_secs(30),
            xclid: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_proxy(mut self, proxy: Option<String>) -> Self {
        self.proxy = proxy;
        self
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn get(
        &self,
        url: &str,
        params: Vec<(String, String)>,
    ) -> Result<Option<QueueResponse>> {
        let mut session = self.open().await?;
        let response = session.get(url, params).await;
        let close_result = session.close().await;
        match (response, close_result) {
            (Ok(response), Ok(())) => Ok(response),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    pub async fn post_json(&self, url: &str, body: Value) -> Result<Option<QueueResponse>> {
        let mut session = self.open().await?;
        let response = session.post_json(url, body).await;
        let close_result = session.close().await;
        match (response, close_result) {
            (Ok(response), Ok(())) => Ok(response),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    pub async fn open(&self) -> Result<QueueSession> {
        Ok(QueueSession { client: self.clone(), ctx: self.context().await? })
    }

    async fn context(&self) -> Result<Option<RequestContext>> {
        let Some(account) = self.pool.get_for_queue_or_wait(&self.queue).await? else {
            return Ok(None);
        };

        let proxy = self
            .proxy
            .clone()
            .or_else(|| std::env::var("XSCRAPER_PROXY").ok())
            .or_else(|| account.proxy.clone());
        let mut profile = FetchProfile::new().with_timeout(self.timeout).with_proxy(proxy.clone());
        let headers = account.http_headers()?;
        for (name, value) in &headers {
            profile = profile.with_header(name.as_str(), value.to_str().unwrap_or_default());
        }

        Ok(Some(RequestContext {
            account,
            client: profile.client_for_base_url(&self.base_url)?,
            proxy,
            req_count: 0,
        }))
    }

    async fn request_with_context(
        &self,
        ctx: &mut RequestContext,
        request: QueueRequest<'_>,
    ) -> std::result::Result<QueueResponse, RequestDecision> {
        let url = self.normalize_url(request.url())?;
        let method = request.method();
        let transaction_id = self
            .transaction_id(&ctx.account.username, &ctx.client, method.as_str(), url.path())
            .await;
        let builder = ctx.client.request(method, url);
        let builder = match request {
            QueueRequest::Get { params, .. } => builder.query(params),
            QueueRequest::PostJson { body, .. } => builder.json(body),
        };
        let response = match builder.header("x-client-transaction-id", transaction_id).send().await
        {
            Ok(response) => response,
            Err(error) => {
                let outcome = if ctx.proxy.is_some() { "proxy_failed" } else { "request_failed" };
                let _ = self.record_event(ctx, outcome, None, None, None, Some(error.to_string()));
                return Err(RequestDecision::Abort(error.to_string()));
            }
        };

        let status = response.status();
        let headers = response.headers().clone();
        let value = match response.json::<Value>().await {
            Ok(value) => value,
            Err(error) if status.is_success() => {
                let message = format!("invalid JSON response for successful request: {error}");
                let _ = self.record_event(
                    ctx,
                    "request_failed",
                    Some(status),
                    None,
                    None,
                    Some(message.clone()),
                );
                return Err(RequestDecision::Abort(message));
            }
            Err(_) => Value::Object(Default::default()),
        };

        let inspection = inspect_response(status, &headers, &value);
        if let Err(decision) = check_response(&ctx.account.username, status, &headers, &value) {
            let _ = self.record_event(
                ctx,
                inspection.outcome,
                Some(status),
                inspection.x_error_code,
                Some((inspection.rate_remaining, inspection.rate_reset)),
                Some(inspection.message),
            );
            return Err(decision);
        }

        ctx.req_count += 1;
        let _ = self.record_event(
            ctx,
            "success",
            Some(status),
            inspection.x_error_code,
            Some((inspection.rate_remaining, inspection.rate_reset)),
            None,
        );
        Ok(QueueResponse { account_username: ctx.account.username.clone(), status, headers, value })
    }

    async fn transaction_id(
        &self,
        username: &str,
        client: &Client,
        method: &str,
        path: &str,
    ) -> String {
        if std::env::var("XSCRAPER_DISABLE_XCLID")
            .is_ok_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        {
            return pseudo_transaction_id(method, path);
        }

        if let Some(generator) =
            self.xclid.lock().ok().and_then(|cache| cache.get(username).cloned())
        {
            return generator.calc(method, path);
        }

        match XClientTransactionIdGenerator::create(client).await {
            Ok(generator) => {
                let id = generator.calc(method, path);
                if let Ok(mut cache) = self.xclid.lock() {
                    cache.insert(username.to_string(), generator);
                }
                id
            }
            Err(error) => {
                tracing::debug!("xclid generation failed, using fallback: {error}");
                pseudo_transaction_id(method, path)
            }
        }
    }

    fn normalize_url(&self, url: &str) -> std::result::Result<Url, RequestDecision> {
        if url.starts_with("http://") || url.starts_with("https://") {
            return Url::parse(url).map_err(|error| RequestDecision::Abort(error.to_string()));
        }

        let base = Url::parse(&self.base_url)
            .map_err(|error| RequestDecision::Abort(error.to_string()))?;
        base.join(url).map_err(|error| RequestDecision::Abort(error.to_string()))
    }

    fn record_event(
        &self,
        ctx: &RequestContext,
        outcome: &str,
        status: Option<StatusCode>,
        x_error_code: Option<i64>,
        rate: Option<(i64, i64)>,
        message: Option<String>,
    ) -> Result<()> {
        self.pool.record_account_event(AccountEventInput {
            username: ctx.account.username.clone(),
            queue: self.queue.clone(),
            operation: Some(self.queue.clone()),
            outcome: outcome.to_string(),
            status: status.map(|status| i64::from(status.as_u16())),
            x_error_code,
            proxy: ctx.proxy.clone(),
            rate_remaining: rate.map(|(remaining, _)| remaining).filter(|value| *value >= 0),
            rate_reset: rate.map(|(_, reset)| reset).filter(|value| *value >= 0),
            message,
            evidence_ref: None,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum QueueRequest<'a> {
    Get { url: &'a str, params: &'a [(String, String)] },
    PostJson { url: &'a str, body: &'a Value },
}

impl QueueRequest<'_> {
    fn method(&self) -> Method {
        match self {
            Self::Get { .. } => Method::GET,
            Self::PostJson { .. } => Method::POST,
        }
    }

    fn url(&self) -> &str {
        match self {
            Self::Get { url, .. } | Self::PostJson { url, .. } => url,
        }
    }
}

#[derive(Debug)]
pub struct QueueSession {
    client: QueueClient,
    ctx: Option<RequestContext>,
}

impl QueueSession {
    pub async fn get(
        &mut self,
        url: &str,
        params: Vec<(String, String)>,
    ) -> Result<Option<QueueResponse>> {
        self.request(QueueRequest::Get { url, params: &params }).await
    }

    pub async fn post_json(&mut self, url: &str, body: Value) -> Result<Option<QueueResponse>> {
        self.request(QueueRequest::PostJson { url, body: &body }).await
    }

    async fn request(&mut self, request: QueueRequest<'_>) -> Result<Option<QueueResponse>> {
        let mut unknown_retry = 0;
        loop {
            if self.ctx.is_none() {
                self.ctx = self.client.context().await?;
            }
            let Some(ctx) = self.ctx.as_mut() else {
                return Ok(None);
            };

            match self.client.request_with_context(ctx, request).await {
                Ok(response) => return Ok(Some(response)),
                Err(RequestDecision::RetrySame(reason)) => {
                    unknown_retry += 1;
                    if unknown_retry >= 3 {
                        let username = ctx.account.username.clone();
                        let req_count = ctx.req_count;
                        let unlock_at = Utc::now() + chrono::Duration::minutes(15);
                        self.client.pool.lock_until(
                            &username,
                            &self.client.queue,
                            unlock_at,
                            req_count,
                        )?;
                        self.ctx = None;
                        return Err(XScraperError::RequestAborted(format!(
                            "queue {} failed after {unknown_retry} retries for account {username}: {reason}",
                            self.client.queue
                        )));
                    }
                }
                Err(RequestDecision::RetryNewAccount) => {
                    self.ctx = None;
                    continue;
                }
                Err(RequestDecision::Abort(message)) => {
                    return Err(XScraperError::RequestAborted(message));
                }
            }
        }
    }

    pub async fn close(&mut self) -> Result<()> {
        if let Some(ctx) = self.ctx.take() {
            self.client.pool.unlock(&ctx.account.username, &self.client.queue, ctx.req_count)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
enum RequestDecision {
    RetrySame(String),
    RetryNewAccount,
    Abort(String),
}

#[derive(Debug, Clone)]
struct ResponseInspection {
    outcome: &'static str,
    rate_remaining: i64,
    rate_reset: i64,
    x_error_code: Option<i64>,
    message: String,
}

fn check_response(
    username: &str,
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    value: &Value,
) -> std::result::Result<(), RequestDecision> {
    let remaining = headers
        .get("x-rate-limit-remaining")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(-1);
    let reset = headers
        .get("x-rate-limit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(-1);

    let err_msg = error_message(value).unwrap_or_else(|| "OK".into());
    tracing::debug!(
        status = %status,
        username,
        remaining,
        reset,
        error = %err_msg,
        "x api response"
    );

    if remaining == 0 && reset > 0 {
        return Err(RequestDecision::RetryNewAccount);
    }

    if err_msg.starts_with("(88) Rate limit exceeded") && remaining > 0 {
        return Err(RequestDecision::RetryNewAccount);
    }

    if err_msg.starts_with("(326) Authorization: Denied by access control")
        || err_msg.starts_with("(32) Could not authenticate you")
        || (err_msg == "OK" && status == StatusCode::FORBIDDEN)
    {
        return Err(RequestDecision::RetryNewAccount);
    }

    if err_msg.starts_with("(131) Dependency: Internal error")
        && !(status == StatusCode::OK && value.pointer("/data/user").is_some())
    {
        return Err(RequestDecision::Abort(err_msg));
    }

    if !status.is_success() {
        return Err(RequestDecision::RetrySame(format!(
            "status={status} remaining={remaining} reset={reset} error={err_msg}"
        )));
    }

    Ok(())
}

fn inspect_response(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    value: &Value,
) -> ResponseInspection {
    let rate_remaining = headers
        .get("x-rate-limit-remaining")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(-1);
    let rate_reset = headers
        .get("x-rate-limit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(-1);
    let x_error_code = first_error_code(value);
    let message = error_message(value).unwrap_or_else(|| "OK".into());
    let outcome = if rate_remaining == 0
        || x_error_code == Some(88)
        || message.starts_with("(88) Rate limit exceeded")
        || status == StatusCode::TOO_MANY_REQUESTS
    {
        "rate_limited"
    } else if x_error_code == Some(326)
        || x_error_code == Some(32)
        || message.starts_with("(326) Authorization: Denied by access control")
        || message.starts_with("(32) Could not authenticate you")
        || (message == "OK" && status == StatusCode::FORBIDDEN)
    {
        if x_error_code == Some(32) { "auth_failed" } else { "auth_denied" }
    } else if !status.is_success() {
        "request_failed"
    } else {
        "success"
    };

    ResponseInspection { outcome, rate_remaining, rate_reset, x_error_code, message }
}

fn error_message(value: &Value) -> Option<String> {
    let errors = value.get("errors")?.as_array()?;
    let mut messages = Vec::new();
    for error in errors {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(-1);
        let message = error.get("message").and_then(Value::as_str).unwrap_or("unknown");
        messages.push(format!("({code}) {message}"));
    }
    (!messages.is_empty()).then(|| messages.join("; "))
}

fn first_error_code(value: &Value) -> Option<i64> {
    value.get("errors")?.as_array()?.iter().find_map(|error| error.get("code")?.as_i64())
}

fn pseudo_transaction_id(method: &str, path: &str) -> String {
    let mut seed = method.bytes().map(u64::from).sum::<u64>();
    for byte in path.bytes() {
        seed = seed.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    format!("xscraper-{seed:x}")
}
