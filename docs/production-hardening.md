# Production Hardening

This document lists the checks that keep XScraper releasable.

## Offline Gates

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
python3 scripts/security_check.py
python3 scripts/xclid_drift_check.py
python3 scripts/live_acceptance.py
```

The same offline chain is available as one command:

```bash
python3 scripts/release_gate.py
```

`scripts/live_acceptance.py` skips without live credentials. A skip only proves the harness wiring, not live X access.

## Live Acceptance

```bash
XSCRAPER_LIVE_QUERY='rust lang:en' \
XSCRAPER_LIVE_COOKIE_USERNAME=my_account \
XSCRAPER_LIVE_COOKIES='ct0=...; auth_token=...' \
python3 scripts/live_acceptance.py
```

Optional checks:

```bash
XSCRAPER_LIVE_USER_LOGIN=xdevelopers
XSCRAPER_LIVE_TWEET_ID=1649191520250245121
XSCRAPER_LIVE_LIMIT=3
```

Reports are written under `.local/live-acceptance/` and ignored by git. The
report includes staged command results, parser checks, and account-pool health
snapshots so live failures can be debugged without rerunning blindly.

## X List Coverage

List support is covered in three layers:

1. Offline parser and contract tests cover `ListInfo`, list URL/slug parsing,
   operation request variables, CLI registration, and `analyze-list`.
2. `doctor drift --live` checks the current X frontend operation ids for
   `ListByRestId`, `ListBySlug`, `ListLatestTweetsTimeline`,
   `ListRankedTweetsTimeline`, `ListMembers`, `ListSubscribers`,
   `ListOwnerships`, `ListMemberships`, and `CombinedLists`.
3. Real list reads still require a valid account pool. Before calling a list
   workflow current, run a live command such as `xscraper list-details
   https://x.com/i/lists/<id> --raw` with a real cookie-backed account.

## Release

CI runs `python3 scripts/release_gate.py` on every push and pull request. Before
publishing from a machine with network access, also run:

```bash
python3 scripts/release_gate.py --live-drift
```

Tag pushes matching `v*` build Linux, Intel macOS, Apple Silicon macOS, and
Windows archives through `.github/workflows/release.yml`.
