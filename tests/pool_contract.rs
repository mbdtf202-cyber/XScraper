use chrono::Utc;
use tempfile::{TempDir, tempdir};
use xscraper::pool::{AccountsPool, AddAccount};

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
