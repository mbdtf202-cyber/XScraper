use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;
use xscraper::pool::{AccountsPool, AddAccount};

mod support {
    pub mod sample_payloads;
}

fn write_sample_user_payload() -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), support::sample_payloads::user_payload().to_string()).unwrap();
    file
}

fn write_current_search_payload() -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), support::sample_payloads::current_search_payload().to_string())
        .unwrap();
    file
}

#[test]
fn account_pool_adds_locks_and_unlocks_accounts() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("accounts.db");
    let pool = AccountsPool::new(&db);

    assert!(
        pool.add_account(AddAccount {
            username: "user1".into(),
            password: "pass1".into(),
            email: "email1".into(),
            email_password: "email_pass1".into(),
            ..AddAccount::default()
        })
        .unwrap()
    );

    assert!(!pool.get("user1").unwrap().active);
    pool.set_active("user1", true).unwrap();

    let account = pool.get_for_queue("SearchTimeline").unwrap().unwrap();
    assert!(account.locks.contains_key("SearchTimeline"));
    assert!(pool.get_for_queue("SearchTimeline").unwrap().is_none());

    pool.unlock("user1", "SearchTimeline", 3).unwrap();
    let account = pool.get_for_queue("SearchTimeline").unwrap().unwrap();
    assert_eq!(account.stats["SearchTimeline"], 3);
}

#[test]
fn cli_can_parse_fixture_without_accounts() {
    let payload = write_sample_user_payload();
    let mut cmd = Command::cargo_bin("xscraper").unwrap();
    cmd.args(["parse-fixture", payload.path().to_str().unwrap(), "users"])
        .assert()
        .success()
        .stdout(contains("xscraper_dev"));
}

#[test]
fn cli_parse_fixture_handles_current_search_tweets() {
    let payload = write_current_search_payload();
    let mut cmd = Command::cargo_bin("xscraper").unwrap();
    cmd.args(["parse-fixture", payload.path().to_str().unwrap(), "tweets"])
        .assert()
        .success()
        .stdout(contains("current_user"))
        .stdout(contains("Current X payload"));
}

#[test]
fn cli_accepts_underscore_command_aliases() {
    let payload = write_sample_user_payload();
    let mut cmd = Command::cargo_bin("xscraper").unwrap();
    cmd.args(["parse_fixture", payload.path().to_str().unwrap(), "users"])
        .assert()
        .success()
        .stdout(contains("xscraper_dev"));
}

#[test]
fn cli_add_cookie_marks_account_active() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("accounts.db");

    let mut cmd = Command::cargo_bin("xscraper").unwrap();
    cmd.args([
        "--db",
        db.to_str().unwrap(),
        "add-cookie",
        "cookie_user",
        "ct0=csrf; auth_token=token",
    ])
    .assert()
    .success()
    .stdout(contains("account added"));

    let pool = AccountsPool::new(db);
    let account = pool.get("cookie_user").unwrap();
    assert!(account.active);
    assert_eq!(account.cookies["ct0"], "csrf");
}
