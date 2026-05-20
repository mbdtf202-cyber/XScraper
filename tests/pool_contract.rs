use chrono::Utc;
use std::collections::BTreeMap;
use tempfile::{TempDir, tempdir};
use xscraper::Account;
use xscraper::pool::{AccountsPool, AddAccount};
use xscraper::storage::AccountStore;

fn pool() -> (TempDir, AccountsPool) {
    let dir = tempdir().unwrap();
    let db = dir.path().join("test.db");
    let pool = AccountsPool::new(db);
    (dir, pool)
}

fn add_account(pool: &AccountsPool, username: &str, password: &str) {
    pool.add_account(AddAccount {
        username: username.into(),
        password: password.into(),
        email: format!("{username}@example.com"),
        email_password: "email_pass".into(),
        ..AddAccount::default()
    })
    .unwrap();
}

#[test]
fn add_accounts_preserves_first_username_case_insensitively() {
    let (_dir, pool) = pool();
    add_account(&pool, "user1", "pass1");
    let account = pool.get("user1").unwrap();
    assert_eq!(account.username, "user1");
    assert_eq!(account.password, "pass1");

    add_account(&pool, "user1", "pass2");
    let account = pool.get("user1").unwrap();
    assert_eq!(account.password, "pass1");

    add_account(&pool, "USER1", "pass2");
    let account = pool.get("user1").unwrap();
    assert_eq!(account.username, "user1");
    assert_eq!(account.password, "pass1");

    add_account(&pool, "user2", "pass2");
    assert_eq!(pool.get_all().unwrap().len(), 2);
}

#[test]
fn get_all_and_save_round_trip_accounts() {
    let (_dir, pool) = pool();
    assert!(pool.get_all().unwrap().is_empty());

    add_account(&pool, "user1", "pass1");
    add_account(&pool, "user2", "pass2");
    let accounts = pool.get_all().unwrap();
    assert_eq!(accounts[0].username, "user1");
    assert_eq!(accounts[1].username, "user2");

    let mut account = pool.get("user1").unwrap();
    account.password = "pass3".into();
    pool.save(&account).unwrap();
    assert_eq!(pool.get("user1").unwrap().password, "pass3");
}

#[test]
fn get_for_queue_and_unlock_round_trip_queue_state() {
    let (_dir, pool) = pool();
    add_account(&pool, "user1", "pass1");
    pool.set_active("user1", true).unwrap();

    let account = pool.get_for_queue("SearchTimeline").unwrap().unwrap();
    assert!(account.active);
    assert!(account.locks.contains_key("SearchTimeline"));
    assert!(pool.get_for_queue("SearchTimeline").unwrap().is_none());

    pool.unlock("user1", "SearchTimeline", 2).unwrap();
    let account = pool.get_for_queue("SearchTimeline").unwrap().unwrap();
    assert!(account.locks.contains_key("SearchTimeline"));
    assert_eq!(account.stats["SearchTimeline"], 2);

    let unlock_at = Utc::now() + chrono::Duration::minutes(1);
    pool.lock_until("user1", "SearchTimeline", unlock_at, 1).unwrap();
    let account = pool.get("user1").unwrap();
    assert_eq!(account.locks["SearchTimeline"].timestamp(), unlock_at.timestamp());
    assert!(pool.get_for_queue("SearchTimeline").unwrap().is_none());
}

#[test]
fn stats_and_delete_report_pool_state() {
    let (_dir, pool) = pool();
    let stats = pool.stats().unwrap();
    assert_eq!(stats.total, 0);
    assert_eq!(stats.active, 0);
    assert_eq!(stats.inactive, 0);

    add_account(&pool, "user1", "pass1");
    let stats = pool.stats().unwrap();
    assert_eq!(stats.total, 1);
    assert_eq!(stats.active, 0);
    assert_eq!(stats.inactive, 1);

    pool.set_active("user1", true).unwrap();
    let _ = pool.get_for_queue("SearchTimeline").unwrap();
    let stats = pool.stats().unwrap();
    assert_eq!(stats.total, 1);
    assert_eq!(stats.active, 1);
    assert_eq!(stats.locked["SearchTimeline"], 1);

    pool.set_active("user1", false).unwrap();
    assert_eq!(pool.delete_inactive().unwrap(), 1);
    assert!(pool.get_all().unwrap().is_empty());
}

#[test]
fn invalid_header_material_returns_error_instead_of_panicking() {
    let mut cookies = BTreeMap::new();
    cookies.insert("ct0".into(), "bad\r\ncsrf".into());

    let account = Account::new(
        "user1",
        "pass1",
        "email@example.com",
        "email_pass",
        Some("bad\r\nagent".into()),
        None,
        cookies,
        None,
    );

    assert!(account.http_headers().is_err());
}

#[test]
fn account_store_loads_rows_with_legacy_empty_or_truncated_json_fields() {
    let dir = tempdir().unwrap();
    let store = AccountStore::new(dir.path().join("legacy.db"));
    let conn = rusqlite::Connection::open(store.path()).unwrap();
    conn.execute_batch(
        "CREATE TABLE accounts (
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
    )
    .unwrap();
    conn.execute(
        "INSERT INTO accounts VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        rusqlite::params![
            "legacy",
            "pass",
            "legacy@example.com",
            "email-pass",
            "ua",
            true,
            r#"{"SearchTimeline":"2026-05-17T20:11:04.680527+00:00""#,
            r#"{"SearchTimeline":1"#,
            "",
            "",
            Option::<String>::None,
            Option::<String>::None,
            Some("not-a-rfc3339-timestamp".to_string()),
            Option::<String>::None,
        ],
    )
    .unwrap();

    let accounts = store.get_all().unwrap();
    assert_eq!(accounts.len(), 1);
    let account = &accounts[0];
    assert_eq!(account.username, "legacy");
    assert!(account.locks.is_empty());
    assert!(account.stats.is_empty());
    assert!(account.headers.is_empty());
    assert!(account.cookies.is_empty());
    assert!(account.last_used.is_none());
}

#[test]
fn health_report_exposes_lock_queue_availability_and_account_state() {
    let (_dir, pool) = pool();
    add_account(&pool, "user1", "pass1");
    add_account(&pool, "user2", "pass2");
    pool.set_active("user1", true).unwrap();

    let _ = pool.get_for_queue("SearchTimeline").unwrap().unwrap();
    let report = pool.health_report().unwrap();

    assert_eq!(report.total, 2);
    assert_eq!(report.active, 1);
    assert_eq!(report.inactive, 1);
    assert_eq!(report.queues["SearchTimeline"].locked, 1);
    assert_eq!(report.queues["SearchTimeline"].available, 0);

    let user1 = report.accounts.iter().find(|account| account.username == "user1").unwrap();
    assert!(user1.active);
    assert!(user1.locked_queues.iter().any(|queue| queue.queue == "SearchTimeline"));

    let user2 = report.accounts.iter().find(|account| account.username == "user2").unwrap();
    assert!(!user2.active);
}

#[test]
fn unlock_account_clears_only_requested_account_locks() {
    let (_dir, pool) = pool();
    add_account(&pool, "user1", "pass1");
    add_account(&pool, "user2", "pass2");
    pool.set_active("user1", true).unwrap();
    pool.set_active("user2", true).unwrap();

    let first = pool.get_for_queue("SearchTimeline").unwrap().unwrap();
    let second = pool.get_for_queue("SearchTimeline").unwrap().unwrap();
    assert_ne!(first.username, second.username);

    let removed = pool.unlock_account(&first.username).unwrap();
    assert_eq!(removed, 1);

    let first = pool.get(&first.username).unwrap();
    let second = pool.get(&second.username).unwrap();
    assert!(first.locks.is_empty());
    assert!(second.locks.contains_key("SearchTimeline"));
}
