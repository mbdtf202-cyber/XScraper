use crate::browser_probe::{
    BrowserDriftReport, build_browser_drift_report, parse_graphql_xhr_events,
};
use crate::error::{Result, XScraperError};
use crate::operations::operation_request;
use crate::pool::PoolHealthReport;
use crate::xclid::{XclidAssetDiagnostics, diagnose_from_html};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsReport {
    pub ok: bool,
    #[serde(rename = "accountPool")]
    pub account_pool: PoolHealthReport,
    pub xclid: XclidAssetDiagnostics,
    #[serde(rename = "browserDrift", skip_serializing_if = "Option::is_none")]
    pub browser_drift: Option<BrowserDriftReport>,
}

pub fn build_report(
    account_pool: PoolHealthReport,
    xclid: XclidAssetDiagnostics,
    browser_drift: Option<BrowserDriftReport>,
) -> DiagnosticsReport {
    let ok = account_pool.accounts.iter().all(|account| account.health_score >= 70)
        && account_pool.proxies.iter().all(|proxy| proxy.score >= 70)
        && xclid.ok
        && browser_drift.as_ref().is_none_or(|report| report.ok);
    DiagnosticsReport { ok, account_pool, xclid, browser_drift }
}

pub fn browser_drift_report_from_events_file(
    path: &Path,
    operation: &str,
    target: &str,
    kv: Option<Value>,
) -> Result<BrowserDriftReport> {
    let raw = std::fs::read_to_string(path).map_err(|source| XScraperError::io(path, source))?;
    let value: Value = serde_json::from_str(&raw)?;
    let events = value.as_array().ok_or_else(|| {
        XScraperError::Config("browser events file must contain a JSON array".into())
    })?;
    let observed = parse_graphql_xhr_events(events)?;
    let local = operation_request(operation, target, kv)
        .ok_or_else(|| XScraperError::Config(format!("unknown operation: {operation}")))?;
    Ok(build_browser_drift_report(vec![local], observed))
}

pub fn offline_xclid_report() -> XclidAssetDiagnostics {
    diagnose_from_html("")
}
