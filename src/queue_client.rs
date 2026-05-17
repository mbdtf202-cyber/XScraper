use crate::account::Account;
use crate::error::{Result, XScraperError};
use crate::pool::AccountsPool;
use crate::xclid::XClientTransactionIdGenerator;
use chrono::Utc;
use reqwest::{Client, Proxy, StatusCode};
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
    xclid: Arc<Mutex<BTreeMap<String, XClientTransactionIdGenerator>>>,
}

#[derive(Debug)]
struct RequestContext {
    account: Account,
    client: Client,
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

    pub async fn get(
        &self,
        url: &str,
        params: Vec<(String, String)>,
    ) -> Result<Option<QueueResponse>> {
        let mut session = self.open().await?;
        let response = session.get(url, params).await;
        session.close().await?;
        response
    }

    pub async fn open(&self) -> Result<QueueSession> {
        Ok(QueueSession { client: self.clone(), ctx: self.context().await? })
    }

    async fn context(&self) -> Result<Option<RequestContext>> {
        let Some(account) = self.pool.get_for_queue_or_wait(&self.queue).await? else {
            return Ok(None);
        };

        let mut builder = Client::builder()
            .default_headers(account.http_headers()?)
            .redirect(reqwest::redirect::Policy::limited(10));
        let proxy = self
            .proxy
            .clone()
            .or_else(|| std::env::var("XSCRAPER_PROXY").ok())
            .or_else(|| account.proxy.clone());
        if let Some(proxy) = proxy {
            builder = builder.proxy(Proxy::all(proxy)?);
        }

        Ok(Some(RequestContext { account, client: builder.build()?, req_count: 0 }))
    }

    async fn request_with_context(
        &self,
        ctx: &mut RequestContext,
        url: &str,
        params: &[(String, String)],
    ) -> std::result::Result<QueueResponse, RequestDecision> {
        let url = self.normalize_url(url)?;
        let transaction_id =
            self.transaction_id(&ctx.account.username, &ctx.client, "GET", url.path()).await;
        let response = ctx
            .client
            .get(url)
            .query(params)
            .header("x-client-transaction-id", transaction_id)
            .send()
            .await
            .map_err(|error| RequestDecision::Abort(error.to_string()))?;

        let status = response.status();
        let headers = response.headers().clone();
        let value =
            response.json::<Value>().await.unwrap_or_else(|_| Value::Object(Default::default()));

        check_response(&ctx.account.username, status, &headers, &value).map_err(|decision| {
            match decision {
                RequestDecision::RetryNewAccount | RequestDecision::RetrySame => decision,
                RequestDecision::Abort(message) => RequestDecision::Abort(message),
            }
        })?;

        ctx.req_count += 1;
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
        let mut unknown_retry = 0;
        loop {
            if self.ctx.is_none() {
                self.ctx = self.client.context().await?;
            }
            let Some(ctx) = self.ctx.as_mut() else {
                return Ok(None);
            };

            match self.client.request_with_context(ctx, url, &params).await {
                Ok(response) => return Ok(Some(response)),
                Err(RequestDecision::RetrySame) => {
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
                        return Ok(None);
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
    RetrySame,
    RetryNewAccount,
    Abort(String),
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
        return Err(RequestDecision::RetrySame);
    }

    Ok(())
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

fn pseudo_transaction_id(method: &str, path: &str) -> String {
    let mut seed = method.bytes().map(u64::from).sum::<u64>();
    for byte in path.bytes() {
        seed = seed.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    format!("xscraper-{seed:x}")
}
