use crate::error::{Result, XScraperError};
use crate::gql::{default_features, merge_json};
use crate::operations::OperationRequest;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedGraphqlRequest {
    pub op: String,
    pub operation: String,
    pub method: String,
    pub url: String,
    pub variables: Value,
    pub features: Value,
    #[serde(rename = "fieldToggles")]
    pub field_toggles: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserDriftReport {
    pub ok: bool,
    pub matches: Vec<BrowserOperationMatch>,
    #[serde(rename = "unexpectedRemote")]
    pub unexpected_remote: Vec<ObservedGraphqlRequest>,
    #[serde(rename = "missingLocal")]
    pub missing_local: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserOperationMatch {
    pub operation: String,
    pub local: String,
    pub observed: String,
    #[serde(rename = "opMatches")]
    pub op_matches: bool,
    #[serde(rename = "variableDiffs")]
    pub variable_diffs: Vec<String>,
    #[serde(rename = "featureDiffs")]
    pub feature_diffs: Vec<String>,
    #[serde(rename = "fieldToggleDiffs")]
    pub field_toggle_diffs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BrowserProbeConfig {
    pub url: String,
    pub cdp_url: Option<String>,
    pub chrome_path: Option<String>,
    pub user_data_dir: Option<std::path::PathBuf>,
    pub cookies: Vec<BrowserCookie>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
}

impl BrowserProbeConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            cdp_url: None,
            chrome_path: None,
            user_data_dir: None,
            cookies: Vec::new(),
            timeout: Duration::from_secs(20),
        }
    }
}

impl BrowserCookie {
    pub fn x_com(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            domain: ".x.com".into(),
            path: "/".into(),
            secure: true,
        }
    }

    fn to_cdp_value(&self) -> Value {
        json!({
            "name": self.name,
            "value": self.value,
            "domain": self.domain,
            "path": self.path,
            "secure": self.secure
        })
    }
}

pub fn parse_graphql_xhr_events(events: &[Value]) -> Result<Vec<ObservedGraphqlRequest>> {
    let mut requests = Vec::new();
    for event in events {
        let Some(request) = event.pointer("/params/request") else {
            continue;
        };
        if let Some(parsed) = parse_request_value(request)? {
            requests.push(parsed);
        }
    }
    Ok(requests)
}

pub fn build_browser_drift_report(
    local: Vec<OperationRequest>,
    observed: Vec<ObservedGraphqlRequest>,
) -> BrowserDriftReport {
    let local_by_operation = local
        .into_iter()
        .map(|request| (request.queue.to_string(), request))
        .collect::<BTreeMap<_, _>>();
    let observed_by_operation = observed
        .iter()
        .cloned()
        .map(|request| (request.operation.clone(), request))
        .collect::<BTreeMap<_, _>>();

    let mut matches = Vec::new();
    let mut missing_local = Vec::new();
    for (operation, local) in &local_by_operation {
        let Some(observed) = observed_by_operation.get(operation) else {
            missing_local.push(operation.clone());
            continue;
        };
        matches.push(compare_operation(local, observed));
    }

    let unexpected_remote = observed
        .into_iter()
        .filter(|request| !local_by_operation.contains_key(&request.operation))
        .collect::<Vec<_>>();
    let ok = missing_local.is_empty()
        && matches.iter().all(|item| {
            item.op_matches
                && item.variable_diffs.is_empty()
                && item.feature_diffs.is_empty()
                && item.field_toggle_diffs.is_empty()
        });

    BrowserDriftReport { ok, matches, unexpected_remote, missing_local }
}

pub async fn capture_graphql_xhr_events(config: BrowserProbeConfig) -> Result<Vec<Value>> {
    let mut launched = None;
    let endpoint = match config.cdp_url.clone() {
        Some(url) => url,
        None => {
            let (mut child, endpoint) = launch_chrome(&config)?;
            if let Err(error) = wait_for_cdp(&endpoint, config.timeout).await {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            launched = Some(child);
            endpoint
        }
    };
    let result = capture_from_cdp(&endpoint, &config.url, &config.cookies, config.timeout).await;
    if let Some(mut child) = launched {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

async fn capture_from_cdp(
    endpoint: &str,
    target_url: &str,
    cookies: &[BrowserCookie],
    timeout: Duration,
) -> Result<Vec<Value>> {
    let tab = open_cdp_tab(endpoint, target_url).await?;
    let ws_url = tab
        .get("webSocketDebuggerUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| XScraperError::Config("CDP webSocketDebuggerUrl missing".into()))?;
    let (mut socket, _) = tokio_tungstenite::connect_async(ws_url)
        .await
        .map_err(|error| XScraperError::Config(format!("CDP websocket connect failed: {error}")))?;
    let mut next_id = 1;
    send_cdp(&mut socket, next_id, "Network.enable", json!({})).await?;
    next_id += 1;
    if !cookies.is_empty() {
        let values = cookies.iter().map(BrowserCookie::to_cdp_value).collect::<Vec<_>>();
        send_cdp(&mut socket, next_id, "Network.setCookies", json!({ "cookies": values })).await?;
        next_id += 1;
    }
    send_cdp(&mut socket, next_id, "Page.enable", json!({})).await?;
    next_id += 1;
    send_cdp(&mut socket, next_id, "Page.navigate", json!({ "url": target_url })).await?;

    let deadline = Instant::now() + timeout;
    let mut events = Vec::new();
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Some(message) =
            tokio::time::timeout(remaining.min(Duration::from_millis(500)), socket.next())
                .await
                .ok()
                .flatten()
        else {
            continue;
        };
        let message =
            message.map_err(|error| XScraperError::Config(format!("CDP read failed: {error}")))?;
        if !message.is_text() {
            continue;
        }
        let value: Value = serde_json::from_str(message.to_text().unwrap_or_default())?;
        if value.get("method").and_then(Value::as_str) == Some("Network.requestWillBeSent")
            && value
                .pointer("/params/request/url")
                .and_then(Value::as_str)
                .is_some_and(|url| url.contains("/i/api/graphql/"))
        {
            events.push(value);
        }
        if events.iter().any(|event| {
            event
                .pointer("/params/request/url")
                .and_then(Value::as_str)
                .is_some_and(|url| url.contains("/SearchTimeline"))
        }) && target_url.contains("/search")
        {
            break;
        }
    }
    let _ = close_cdp_tab(endpoint, tab.get("id").and_then(Value::as_str)).await;
    Ok(events)
}

async fn send_cdp<S>(socket: &mut S, id: i64, method: &str, params: Value) -> Result<()>
where
    S: SinkExt<tokio_tungstenite::tungstenite::Message> + Unpin,
    <S as futures::Sink<tokio_tungstenite::tungstenite::Message>>::Error: std::fmt::Display,
{
    let payload = json!({ "id": id, "method": method, "params": params }).to_string();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(payload.into()))
        .await
        .map_err(|error| XScraperError::Config(format!("CDP send failed: {error}")))
}

async fn open_cdp_tab(endpoint: &str, target_url: &str) -> Result<Value> {
    let url =
        format!("{}/json/new?{}", endpoint.trim_end_matches('/'), urlencoding::encode(target_url));
    reqwest::Client::new()
        .put(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(Into::into)
}

async fn close_cdp_tab(endpoint: &str, target_id: Option<&str>) -> Result<()> {
    let Some(target_id) = target_id else {
        return Ok(());
    };
    let url = format!("{}/json/close/{target_id}", endpoint.trim_end_matches('/'));
    reqwest::Client::new().get(url).send().await?.error_for_status()?;
    Ok(())
}

fn launch_chrome(config: &BrowserProbeConfig) -> Result<(Child, String)> {
    let port = pick_debug_port();
    let chrome = config
        .chrome_path
        .clone()
        .or_else(|| std::env::var("XSCRAPER_CHROME").ok())
        .unwrap_or_else(default_chrome_path);
    let user_data_dir = config.user_data_dir.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("xscraper-cdp-{}-{port}", std::process::id()))
    });
    let child = Command::new(&chrome)
        .arg(format!("--remote-debugging-port={port}"))
        .arg(format!("--user-data-dir={}", user_data_dir.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("about:blank")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| XScraperError::io(chrome, source))?;
    let endpoint = format!("http://127.0.0.1:{port}");
    Ok((child, endpoint))
}

async fn wait_for_cdp(endpoint: &str, timeout: Duration) -> Result<()> {
    let client = reqwest::Client::builder().timeout(Duration::from_millis(500)).build()?;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if client
            .get(format!("{}/json/version", endpoint.trim_end_matches('/')))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err(XScraperError::Config(format!("CDP endpoint did not start at {endpoint}")))
}

fn pick_debug_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|listener| listener.local_addr().ok())
        .map(|addr| addr.port())
        .unwrap_or(9222)
}

fn default_chrome_path() -> String {
    if cfg!(target_os = "macos") {
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into()
    } else if cfg!(target_os = "windows") {
        "chrome.exe".into()
    } else {
        "google-chrome".into()
    }
}

fn parse_request_value(request: &Value) -> Result<Option<ObservedGraphqlRequest>> {
    let Some(url) = request.get("url").and_then(Value::as_str) else {
        return Ok(None);
    };
    if !url.contains("/graphql/") {
        return Ok(None);
    }
    let parsed = Url::parse(url).map_err(|error| XScraperError::Config(error.to_string()))?;
    let Some((op, operation)) = parse_graphql_op(&parsed) else {
        return Ok(None);
    };
    let method = request.get("method").and_then(Value::as_str).unwrap_or("GET").to_string();
    let mut variables = Value::Object(Default::default());
    let mut features = Value::Object(Default::default());
    let mut field_toggles = Value::Object(Default::default());

    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "variables" => variables = parse_json_query_value(&value)?,
            "features" => features = parse_json_query_value(&value)?,
            "fieldToggles" => field_toggles = parse_json_query_value(&value)?,
            _ => {}
        }
    }
    if let Some(post_data) = request.get("postData").and_then(Value::as_str) {
        let body: Value = serde_json::from_str(post_data)?;
        if let Some(value) = body.get("variables") {
            variables = value.clone();
        }
        if let Some(value) = body.get("features") {
            features = value.clone();
        }
        if let Some(value) = body.get("fieldToggles") {
            field_toggles = value.clone();
        }
    }

    Ok(Some(ObservedGraphqlRequest {
        op,
        operation,
        method,
        url: url.to_string(),
        variables,
        features,
        field_toggles,
    }))
}

fn parse_graphql_op(url: &Url) -> Option<(String, String)> {
    let segments = url.path_segments()?.collect::<Vec<_>>();
    let idx = segments.iter().position(|segment| *segment == "graphql")?;
    let query_id = *segments.get(idx + 1)?;
    let operation = *segments.get(idx + 2)?;
    Some((format!("{query_id}/{operation}"), operation.to_string()))
}

fn parse_json_query_value(raw: &str) -> Result<Value> {
    serde_json::from_str(raw).map_err(Into::into)
}

fn compare_operation(
    local: &OperationRequest,
    observed: &ObservedGraphqlRequest,
) -> BrowserOperationMatch {
    let local_features = merge_json(default_features(), local.features.clone());
    BrowserOperationMatch {
        operation: observed.operation.clone(),
        local: local.op.to_string(),
        observed: observed.op.clone(),
        op_matches: local.op == observed.op,
        variable_diffs: compare_observed_keys("variables", &local.variables, &observed.variables),
        feature_diffs: compare_observed_keys("features", &local_features, &observed.features),
        field_toggle_diffs: compare_observed_keys(
            "fieldToggles",
            local.field_toggles.as_ref().unwrap_or(&Value::Object(Default::default())),
            &observed.field_toggles,
        ),
    }
}

fn compare_observed_keys(label: &str, local: &Value, observed: &Value) -> Vec<String> {
    let Some(observed_map) = observed.as_object() else {
        return Vec::new();
    };
    observed_map
        .iter()
        .filter_map(|(key, observed_value)| {
            let local_value = local.get(key);
            (local_value != Some(observed_value))
                .then(|| format!("{label}.{key}: local={local_value:?} observed={observed_value}"))
        })
        .collect()
}
