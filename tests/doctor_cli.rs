use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn doctor_security_reports_clean_repository() {
    Command::cargo_bin("xscraper")
        .unwrap()
        .arg("doctor")
        .arg("security")
        .assert()
        .success()
        .stdout(contains("security: ok"));
}

#[test]
fn doctor_security_rejects_world_readable_runtime_database() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("accounts.db");
    std::fs::write(&db, "").unwrap();
    std::fs::write(dir.path().join(".gitignore"), "/accounts.db*\n/.env\n/.env.*\n/.local/\n")
        .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    Command::cargo_bin("xscraper")
        .unwrap()
        .current_dir(dir.path())
        .arg("doctor")
        .arg("security")
        .assert()
        .failure()
        .stderr(contains("accounts.db is readable by group/others"));
}

#[test]
fn doctor_imap_prints_resolved_domain_without_password() {
    Command::cargo_bin("xscraper")
        .unwrap()
        .args(["doctor", "imap", "user@icloud.com"])
        .assert()
        .success()
        .stdout(contains("imap.mail.me.com"));
}

#[test]
fn doctor_xclid_can_run_in_offline_mode() {
    Command::cargo_bin("xscraper")
        .unwrap()
        .args(["doctor", "xclid", "--offline"])
        .assert()
        .success()
        .stdout(contains("xclid: ok"));
}
