use crate::account::Account;
use crate::error::{Result, XScraperError};
use crate::utils::now_utc;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AccountStore {
    path: PathBuf,
}

impl AccountStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn connect(&self) -> Result<Connection> {
        let conn = Connection::open(&self.path)?;
        conn.busy_timeout(Duration::from_secs(30))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&conn)?;
        Ok(conn)
    }

    pub fn add_account(&self, account: &Account) -> Result<bool> {
        let conn = self.connect()?;
        let updated = conn.execute(
            "INSERT OR IGNORE INTO accounts (
                username, password, email, email_password, user_agent, active,
                locks, stats, headers, cookies, proxy, error_msg, last_used, mfa_code
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                account.username,
                account.password,
                account.email,
                account.email_password,
                account.user_agent,
                account.active,
                account.locks_json()?,
                account.stats_json()?,
                account.headers_json()?,
                account.cookies_json()?,
                account.proxy,
                account.error_msg,
                account.last_used.map(|dt| dt.to_rfc3339()),
                account.mfa_code,
            ],
        )?;
        Ok(updated == 1)
    }

    pub fn save(&self, account: &Account) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO accounts (
                username, password, email, email_password, user_agent, active,
                locks, stats, headers, cookies, proxy, error_msg, last_used, mfa_code
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(username) DO UPDATE SET
                password = excluded.password,
                email = excluded.email,
                email_password = excluded.email_password,
                user_agent = excluded.user_agent,
                active = excluded.active,
                locks = excluded.locks,
                stats = excluded.stats,
                headers = excluded.headers,
                cookies = excluded.cookies,
                proxy = excluded.proxy,
                error_msg = excluded.error_msg,
                last_used = excluded.last_used,
                mfa_code = excluded.mfa_code",
            params![
                account.username,
                account.password,
                account.email,
                account.email_password,
                account.user_agent,
                account.active,
                account.locks_json()?,
                account.stats_json()?,
                account.headers_json()?,
                account.cookies_json()?,
                account.proxy,
                account.error_msg,
                account.last_used.map(|dt| dt.to_rfc3339()),
                account.mfa_code,
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, username: &str) -> Result<Account> {
        self.get_optional(username)?.ok_or_else(|| XScraperError::AccountNotFound(username.into()))
    }

    pub fn get_optional(&self, username: &str) -> Result<Option<Account>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare("SELECT * FROM accounts WHERE username = ?1")?;
        Ok(stmt.query_row(params![username], row_to_account).optional()?)
    }

    pub fn get_all(&self) -> Result<Vec<Account>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare("SELECT * FROM accounts ORDER BY username COLLATE NOCASE")?;
        let rows = stmt.query_map([], row_to_account)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn delete_accounts(&self, usernames: &[String]) -> Result<usize> {
        let conn = self.connect()?;
        let mut deleted = 0;
        for username in usernames {
            deleted +=
                conn.execute("DELETE FROM accounts WHERE username = ?1", params![username])?;
        }
        Ok(deleted)
    }

    pub fn delete_inactive(&self) -> Result<usize> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM accounts WHERE active = 0", []).map_err(Into::into)
    }

    pub fn set_active(&self, username: &str, active: bool) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "UPDATE accounts SET active = ?1, error_msg = NULL WHERE username = ?2",
            params![active, username],
        )?;
        Ok(())
    }

    pub fn mark_inactive(&self, username: &str, message: Option<&str>) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "UPDATE accounts SET active = 0, error_msg = ?1 WHERE username = ?2",
            params![message, username],
        )?;
        Ok(())
    }

    pub fn reset_locks(&self) -> Result<()> {
        let conn = self.connect()?;
        conn.execute("UPDATE accounts SET locks = '{}'", [])?;
        Ok(())
    }

    pub fn get_for_queue(&self, queue: &str) -> Result<Option<Account>> {
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = now_utc();
        let mut stmt = tx.prepare("SELECT * FROM accounts ORDER BY username COLLATE NOCASE")?;
        let rows = stmt.query_map([], row_to_account)?;
        let accounts =
            rows.collect::<std::result::Result<Vec<_>, _>>().map_err(XScraperError::from)?;

        let Some(mut account) = accounts.into_iter().find(|account| {
            account.active && account.locks.get(queue).is_none_or(|unlock_at| unlock_at <= &now)
        }) else {
            return Ok(None);
        };

        account.last_used = Some(now);
        account.locks.insert(queue.to_string(), now + chrono::Duration::minutes(15));
        tx.execute(
            "UPDATE accounts SET locks = ?1, last_used = ?2 WHERE username = ?3",
            params![
                account.locks_json()?,
                account.last_used.map(|dt| dt.to_rfc3339()),
                account.username
            ],
        )?;
        drop(stmt);
        tx.commit()?;

        Ok(Some(account))
    }

    pub fn unlock(&self, username: &str, queue: &str, req_count: i64) -> Result<()> {
        let mut account = self.get(username)?;
        account.locks.remove(queue);
        *account.stats.entry(queue.to_string()).or_default() += req_count;
        account.last_used = Some(now_utc());
        self.save(&account)
    }

    pub fn lock_until(
        &self,
        username: &str,
        queue: &str,
        unlock_at: DateTime<Utc>,
        req_count: i64,
    ) -> Result<()> {
        let mut account = self.get(username)?;
        account.locks.insert(queue.to_string(), unlock_at);
        *account.stats.entry(queue.to_string()).or_default() += req_count;
        account.last_used = Some(now_utc());
        self.save(&account)
    }

    pub fn stats(&self) -> Result<PoolStats> {
        let accounts = self.get_all()?;
        let now = now_utc();
        let mut locked = BTreeMap::new();

        for account in &accounts {
            for (queue, unlock_at) in &account.locks {
                if unlock_at > &now {
                    *locked.entry(queue.clone()).or_insert(0) += 1;
                }
            }
        }

        Ok(PoolStats {
            total: accounts.len(),
            active: accounts.iter().filter(|account| account.active).count(),
            inactive: accounts.iter().filter(|account| !account.active).count(),
            locked,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub total: usize,
    pub active: usize,
    pub inactive: usize,
    pub locked: BTreeMap<String, usize>,
}

impl PoolStats {
    pub fn rows(&self) -> Vec<BTreeMap<String, String>> {
        self.locked
            .iter()
            .map(|(queue, locked)| {
                BTreeMap::from([
                    ("queue".into(), queue.clone()),
                    ("locked".into(), locked.to_string()),
                    ("available".into(), self.active.saturating_sub(*locked).to_string()),
                ])
            })
            .collect()
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS accounts (
            username TEXT PRIMARY KEY NOT NULL COLLATE NOCASE,
            password TEXT NOT NULL,
            email TEXT NOT NULL COLLATE NOCASE,
            email_password TEXT NOT NULL,
            user_agent TEXT NOT NULL,
            active INTEGER DEFAULT 0 NOT NULL,
            locks TEXT DEFAULT '{}' NOT NULL,
            stats TEXT DEFAULT '{}' NOT NULL,
            headers TEXT DEFAULT '{}' NOT NULL,
            cookies TEXT DEFAULT '{}' NOT NULL,
            proxy TEXT DEFAULT NULL,
            error_msg TEXT DEFAULT NULL,
            last_used TEXT DEFAULT NULL,
            mfa_code TEXT DEFAULT NULL
        );",
    )?;
    Ok(())
}

fn row_to_account(row: &Row<'_>) -> rusqlite::Result<Account> {
    let locks_raw: String = row.get("locks")?;
    let stats_raw: String = row.get("stats")?;
    let headers_raw: String = row.get("headers")?;
    let cookies_raw: String = row.get("cookies")?;
    let last_used_raw: Option<String> = row.get("last_used")?;

    let locks = Account::parse_locks(&locks_raw).map_err(to_sql_error)?;
    let stats = Account::parse_stats(&stats_raw).map_err(to_sql_error)?;
    let headers = crate::utils::parse_json_string_map(&headers_raw).map_err(to_sql_error)?;
    let cookies = crate::utils::parse_json_string_map(&cookies_raw).map_err(to_sql_error)?;
    let last_used = last_used_raw
        .map(|raw| {
            DateTime::parse_from_rfc3339(&raw)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(to_sql_error)
        })
        .transpose()?;

    Ok(Account {
        username: row.get("username")?,
        password: row.get("password")?,
        email: row.get("email")?,
        email_password: row.get("email_password")?,
        user_agent: row.get("user_agent")?,
        active: row.get::<_, bool>("active")?,
        locks,
        stats,
        headers,
        cookies,
        mfa_code: row.get("mfa_code")?,
        proxy: row.get("proxy")?,
        error_msg: row.get("error_msg")?,
        last_used,
    })
}

fn to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
