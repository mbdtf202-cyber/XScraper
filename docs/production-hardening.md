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

Reports are written under `.local/live-acceptance/` and ignored by git.

## Release

CI runs formatting, clippy, tests, security guard, xclid offline drift check, and live harness skip mode on every push and pull request. Tag pushes matching `v*` build Linux, Intel macOS, Apple Silicon macOS, and Windows archives through `.github/workflows/release.yml`.
