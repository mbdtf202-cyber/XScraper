use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn list_commands_are_registered_for_help() {
    for command in [
        "list-details",
        "list-timeline",
        "list-ranked-timeline",
        "list-members",
        "list-subscribers",
        "list-ownerships",
        "list-memberships",
        "combined-lists",
        "analyze-list",
    ] {
        Command::cargo_bin("xscraper")
            .unwrap()
            .args([command, "--help"])
            .assert()
            .success()
            .stdout(contains("Usage:"));
    }
}
