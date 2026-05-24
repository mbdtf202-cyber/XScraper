use crate::account::{Account, default_user_agent};
use crate::error::{Result, XScraperError};
use crate::login::{LoginConfig, login};
use crate::storage::{AccountEvent, AccountEventInput, AccountStore, PoolStats};
use crate::utils::{now_utc, parse_cookies};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AccountsPool {
    store: AccountStore,
    raise_when_no_account: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AddAccount {
    pub username: String,
    pub password: String,
    pub email: String,
    pub email_password: String,
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub cookies: Option<String>,
    pub mfa_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountInfo {
    pub username: String,
    pub logged_in: bool,
    pub active: bool,
    pub last_used: Option<DateTime<Utc>>,
    pub total_req: i64,
    pub error_msg: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginSummary {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PoolHealthReport {
    pub total: usize,
    pub active: usize,
    pub inactive: usize,
    pub queues: BTreeMap<String, QueueHealth>,
    pub accounts: Vec<AccountHealth>,
    pub proxies: Vec<ProxyHealth>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct QueueHealth {
    pub active: usize,
    pub locked: usize,
    pub available: usize,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct AccountHealth {
    pub username: String,
    pub logged_in: bool,
    pub active: bool,
    #[serde(rename = "healthScore")]
    pub health_score: i64,
    #[serde(rename = "lastUsed")]
    pub last_used: Option<DateTime<Utc>>,
    #[serde(rename = "totalReq")]
    pub total_req: i64,
    #[serde(rename = "eventCounts")]
    pub event_counts: BTreeMap<String, i64>,
    pub reasons: Vec<String>,
    #[serde(rename = "lockedQueues")]
    pub locked_queues: Vec<QueueLockHealth>,
    #[serde(rename = "errorMsg")]
    pub error_msg: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ProxyHealth {
    pub proxy: String,
    pub score: i64,
    pub successes: i64,
    pub failures: i64,
    #[serde(rename = "eventCounts")]
    pub event_counts: BTreeMap<String, i64>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct QueueLockHealth {
    pub queue: String,
    #[serde(rename = "unlockAt")]
    pub unlock_at: DateTime<Utc>,
    #[serde(rename = "secondsRemaining")]
    pub seconds_remaining: i64,
}

impl AccountsPool {
    pub fn new(db_file: impl Into<PathBuf>) -> Self {
        Self { store: AccountStore::new(db_file), raise_when_no_account: false }
    }

    pub fn with_raise_when_no_account(mut self, value: bool) -> Self {
        self.raise_when_no_account = value;
        self
    }

    pub fn store(&self) -> &AccountStore {
        &self.store
    }

    pub fn load_from_file(
        &self,
        file_path: impl Into<PathBuf>,
        line_format: &str,
    ) -> Result<usize> {
        let file_path = file_path.into();
        let raw = std::fs::read_to_string(&file_path)
            .map_err(|source| XScraperError::io(file_path.clone(), source))?;
        let delimiter = guess_delimiter(line_format)?;
        let tokens = line_format.split(delimiter).collect::<Vec<_>>();

        for required in ["username", "password", "email", "email_password"] {
            if !tokens.contains(&required) {
                return Err(XScraperError::InvalidLineFormat(line_format.into()));
            }
        }

        let mut added = 0;
        for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let values = line.split(delimiter).map(str::trim).collect::<Vec<_>>();
            if values.len() < tokens.len() {
                return Err(XScraperError::InvalidAccountLine(line.into()));
            }

            let mut fields = BTreeMap::new();
            for (token, value) in tokens.iter().zip(values.iter()) {
                if *token != "_" {
                    fields.insert(*token, (*value).to_string());
                }
            }

            let account = AddAccount {
                username: take_field(&fields, "username", line_format)?,
                password: take_field(&fields, "password", line_format)?,
                email: take_field(&fields, "email", line_format)?,
                email_password: take_field(&fields, "email_password", line_format)?,
                user_agent: fields.get("user_agent").cloned(),
                proxy: fields.get("proxy").cloned(),
                cookies: fields.get("cookies").cloned(),
                mfa_code: fields.get("mfa_code").cloned(),
            };
            if self.add_account(account)? {
                added += 1;
            }
        }

        Ok(added)
    }

    pub fn add_account(&self, input: AddAccount) -> Result<bool> {
        let cookies = input.cookies.as_deref().map(parse_cookies).transpose()?.unwrap_or_default();
        let account = Account::new(
            input.username,
            input.password,
            input.email,
            input.email_password,
            input.user_agent,
            input.proxy,
            cookies,
            input.mfa_code,
        );
        self.store.add_account(&account)
    }

    pub fn add_cookie_account(
        &self,
        username: impl Into<String>,
        cookies: impl Into<String>,
    ) -> Result<bool> {
        let username = username.into();
        self.add_account(AddAccount {
            username: username.clone(),
            password: String::new(),
            email: format!("{username}@local.invalid"),
            email_password: String::new(),
            user_agent: Some(default_user_agent()),
            cookies: Some(cookies.into()),
            ..AddAccount::default()
        })
    }

    pub fn get(&self, username: &str) -> Result<Account> {
        self.store.get(username)
    }

    pub fn get_all(&self) -> Result<Vec<Account>> {
        self.store.get_all()
    }

    pub fn save(&self, account: &Account) -> Result<()> {
        self.store.save(account)
    }

    pub fn get_account(&self, username: &str) -> Result<Option<Account>> {
        self.store.get_optional(username)
    }

    pub fn delete_accounts(&self, usernames: &[String]) -> Result<usize> {
        self.store.delete_accounts(usernames)
    }

    pub fn delete_inactive(&self) -> Result<usize> {
        self.store.delete_inactive()
    }

    pub fn set_active(&self, username: &str, active: bool) -> Result<()> {
        self.store.set_active(username, active)
    }

    pub fn reset_locks(&self) -> Result<()> {
        self.store.reset_locks()
    }

    pub fn unlock_account(&self, username: &str) -> Result<usize> {
        self.store.unlock_account(username)
    }

    pub fn get_for_queue(&self, queue: &str) -> Result<Option<Account>> {
        self.store.get_for_queue(queue)
    }

    pub async fn get_for_queue_or_wait(&self, queue: &str) -> Result<Option<Account>> {
        loop {
            if let Some(account) = self.get_for_queue(queue)? {
                return Ok(Some(account));
            }

            if self.raise_when_no_account
                || std::env::var("XSCRAPER_RAISE_WHEN_NO_ACCOUNT").is_ok_and(|value| {
                    matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes")
                })
            {
                return Err(XScraperError::NoAccount { queue: queue.to_string() });
            }

            if self.store.stats()?.active == 0 {
                tracing::warn!("no active accounts; stopping queue {queue}");
                return Ok(None);
            }

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    pub fn unlock(&self, username: &str, queue: &str, req_count: i64) -> Result<()> {
        self.store.unlock(username, queue, req_count)
    }

    pub fn lock_until(
        &self,
        username: &str,
        queue: &str,
        unlock_at: DateTime<Utc>,
        req_count: i64,
    ) -> Result<()> {
        self.store.lock_until(username, queue, unlock_at, req_count)
    }

    pub fn mark_inactive(&self, username: &str, message: Option<&str>) -> Result<()> {
        self.store.mark_inactive(username, message)
    }

    pub fn record_account_event(&self, event: AccountEventInput) -> Result<()> {
        self.store.record_account_event(event)
    }

    pub fn account_events(&self) -> Result<Vec<AccountEvent>> {
        self.store.account_events()
    }

    pub fn stats(&self) -> Result<PoolStats> {
        self.store.stats()
    }

    pub fn accounts_info(&self) -> Result<Vec<AccountInfo>> {
        let mut rows = self
            .get_all()?
            .into_iter()
            .map(|account| AccountInfo {
                username: account.username,
                logged_in: account.headers.contains_key("authorization")
                    || account.cookies.contains_key("ct0"),
                active: account.active,
                last_used: account.last_used,
                total_req: account.stats.values().sum(),
                error_msg: account.error_msg.map(|msg| msg.chars().take(60).collect()),
            })
            .collect::<Vec<_>>();

        rows.sort_by(|a, b| {
            b.active
                .cmp(&a.active)
                .then_with(|| b.last_used.cmp(&a.last_used))
                .then_with(|| a.username.to_lowercase().cmp(&b.username.to_lowercase()))
        });
        Ok(rows)
    }

    pub fn health_report(&self) -> Result<PoolHealthReport> {
        let accounts = self.get_all()?;
        let events = self.account_events()?;
        let now = now_utc();
        let active = accounts.iter().filter(|account| account.active).count();
        let inactive = accounts.len().saturating_sub(active);
        let mut queue_names = BTreeSet::new();

        for account in &accounts {
            for queue in account.stats.keys().chain(account.locks.keys()) {
                queue_names.insert(queue.clone());
            }
        }

        let queues = queue_names
            .into_iter()
            .map(|queue| {
                let locked = accounts
                    .iter()
                    .filter(|account| {
                        account.active
                            && account.locks.get(&queue).is_some_and(|unlock_at| unlock_at > &now)
                    })
                    .count();
                (queue, QueueHealth { active, locked, available: active.saturating_sub(locked) })
            })
            .collect();

        let mut events_by_account = BTreeMap::<String, Vec<AccountEvent>>::new();
        for event in &events {
            events_by_account
                .entry(event.username.to_ascii_lowercase())
                .or_default()
                .push(event.clone());
        }

        let mut account_rows = accounts
            .into_iter()
            .map(|account| {
                let account_events = events_by_account
                    .get(&account.username.to_ascii_lowercase())
                    .cloned()
                    .unwrap_or_default();
                let mut locked_queues = account
                    .locks
                    .iter()
                    .filter(|(_, unlock_at)| *unlock_at > &now)
                    .map(|(queue, unlock_at)| QueueLockHealth {
                        queue: queue.clone(),
                        unlock_at: *unlock_at,
                        seconds_remaining: (*unlock_at - now).num_seconds().max(0),
                    })
                    .collect::<Vec<_>>();
                locked_queues.sort_by(|left, right| left.queue.cmp(&right.queue));
                let event_counts = event_counts(&account_events);
                let (health_score, reasons) = account_health_score(
                    account.active,
                    &event_counts,
                    account.error_msg.as_deref(),
                );

                AccountHealth {
                    username: account.username,
                    logged_in: account.headers.contains_key("authorization")
                        || account.cookies.contains_key("ct0"),
                    active: account.active,
                    health_score,
                    last_used: account.last_used,
                    total_req: account.stats.values().sum(),
                    event_counts,
                    reasons,
                    locked_queues,
                    error_msg: account.error_msg,
                }
            })
            .collect::<Vec<_>>();
        account_rows.sort_by(|left, right| {
            btree_bool(right.active)
                .cmp(&btree_bool(left.active))
                .then_with(|| right.last_used.cmp(&left.last_used))
                .then_with(|| left.username.to_lowercase().cmp(&right.username.to_lowercase()))
        });

        Ok(PoolHealthReport {
            total: account_rows.len(),
            active,
            inactive,
            queues,
            accounts: account_rows,
            proxies: proxy_health(&events),
        })
    }

    pub async fn login_account(&self, mut account: Account, config: LoginConfig) -> Result<bool> {
        let success = login(&mut account, config).await?;
        self.save(&account)?;
        Ok(success)
    }

    pub async fn login_all(
        &self,
        usernames: Option<&[String]>,
        config: LoginConfig,
    ) -> Result<LoginSummary> {
        let accounts = if let Some(usernames) = usernames {
            let mut accounts = Vec::new();
            for username in usernames {
                if let Some(account) = self.get_account(username)? {
                    accounts.push(account);
                }
            }
            accounts
        } else {
            self.get_all()?
                .into_iter()
                .filter(|account| !account.active && account.error_msg.is_none())
                .collect()
        };

        let total = accounts.len();
        let mut success = 0;
        let mut failed = 0;
        for account in accounts {
            if self.login_account(account, config).await.unwrap_or(false) {
                success += 1;
            } else {
                failed += 1;
            }
        }

        Ok(LoginSummary {
            total,
            success,
            failed,
            message: "login flow completed; cookie accounts are active when ct0 is present".into(),
        })
    }

    pub async fn relogin(&self, usernames: &[String], config: LoginConfig) -> Result<LoginSummary> {
        for username in usernames {
            if let Some(mut account) = self.get_account(username)? {
                account.active = false;
                account.locks.clear();
                account.last_used = None;
                account.error_msg = None;
                account.headers.clear();
                account.cookies.clear();
                account.user_agent = default_user_agent();
                self.save(&account)?;
            }
        }
        self.login_all(Some(usernames), config).await
    }

    pub async fn relogin_failed(&self, config: LoginConfig) -> Result<LoginSummary> {
        let usernames = self
            .get_all()?
            .into_iter()
            .filter(|account| !account.active && account.error_msg.is_some())
            .map(|account| account.username)
            .collect::<Vec<_>>();
        self.relogin(&usernames, config).await
    }
}

fn btree_bool(value: bool) -> u8 {
    u8::from(value)
}

fn event_counts(events: &[AccountEvent]) -> BTreeMap<String, i64> {
    let mut counts = BTreeMap::new();
    for event in events {
        *counts.entry(event.outcome.clone()).or_insert(0) += 1;
    }
    counts
}

fn account_health_score(
    active: bool,
    event_counts: &BTreeMap<String, i64>,
    error_msg: Option<&str>,
) -> (i64, Vec<String>) {
    let mut score = if active { 100 } else { 50 };
    let mut reasons = Vec::new();
    for (outcome, count) in event_counts {
        match outcome.as_str() {
            "success" => {}
            "rate_limited" => {
                score -= 30 * count;
                reasons.push(format!("rate limit events: {count}"));
            }
            "auth_denied" | "auth_failed" => {
                score -= 60 * count;
                reasons.push(format!("auth failures: {count}"));
            }
            "proxy_failed" => {
                score -= 25 * count;
                reasons.push(format!("proxy failures: {count}"));
            }
            "request_failed" => {
                score -= 15 * count;
                reasons.push(format!("request failures: {count}"));
            }
            other => {
                score -= 10 * count;
                reasons.push(format!("{other} events: {count}"));
            }
        }
    }
    if let Some(error_msg) = error_msg
        && !error_msg.is_empty()
    {
        score -= 20;
        reasons.push(format!("account error: {}", error_msg.chars().take(80).collect::<String>()));
    }
    (score.clamp(0, 100), reasons)
}

fn proxy_health(events: &[AccountEvent]) -> Vec<ProxyHealth> {
    let mut by_proxy = BTreeMap::<String, Vec<AccountEvent>>::new();
    for event in events {
        let Some(proxy) = event.proxy.as_ref().filter(|proxy| !proxy.is_empty()) else {
            continue;
        };
        by_proxy.entry(proxy.clone()).or_default().push(event.clone());
    }

    let mut rows = by_proxy
        .into_iter()
        .map(|(proxy, events)| {
            let successes = events.iter().filter(|event| event.outcome == "success").count() as i64;
            let failures = events.len() as i64 - successes;
            let event_counts = event_counts(&events);
            let score = (100 - failures * 25).clamp(0, 100);
            let reasons =
                if failures > 0 { vec![format!("proxy failures: {failures}")] } else { Vec::new() };
            ProxyHealth { proxy, score, successes, failures, event_counts, reasons }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.successes.cmp(&left.successes))
            .then_with(|| left.proxy.cmp(&right.proxy))
    });
    rows
}

fn guess_delimiter(line_format: &str) -> Result<char> {
    line_format
        .chars()
        .find(|ch| !ch.is_ascii_alphanumeric() && *ch != '_')
        .ok_or_else(|| XScraperError::InvalidLineFormat(line_format.into()))
}

fn take_field(fields: &BTreeMap<&str, String>, key: &str, line_format: &str) -> Result<String> {
    fields.get(key).cloned().ok_or_else(|| XScraperError::InvalidLineFormat(line_format.into()))
}
