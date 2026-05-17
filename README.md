# XScraper

XScraper is an async Rust X/Twitter GraphQL scraper with SQLite-backed account sessions, account-level queue locks, raw GraphQL output, typed JSON models, diagnostics, and release automation.

Cookie import is the recommended setup path for real scraping. Password/email login, manual email challenge, IMAP email-code retrieval, and TOTP MFA are also available for account recovery workflows.

## Features

| Area | XScraper |
| --- | --- |
| Runtime | Rust, Tokio, Reqwest |
| Account storage | SQLite with bundled rusqlite |
| Account import | account file plus direct `add-cookie` |
| Cookie sessions | yes |
| Password/email login | yes |
| Rate-limit smoothing | per-queue account locks |
| Raw GraphQL pages | yes with `--raw` |
| Tweet/User/Trend models | typed serde structs |
| CLI | `xscraper` |
| Performance harness | Criterion parser benchmark |

## Install

```bash
cargo install --path .
```

For development:

```bash
cargo build
cargo test
```

## Configuration

The CLI works with flags or environment variables:

```bash
XSCRAPER_DB=accounts.db
XSCRAPER_PROXY=http://127.0.0.1:7890
XSCRAPER_BASE_URL=https://x.com
XSCRAPER_RAISE_WHEN_NO_ACCOUNT=1
```

A JSON example is provided at [`config/xscraper.example.json`](config/xscraper.example.json). The current CLI reads flags/env directly; the JSON file is included as a stable deployment template for wrappers and services.

## Account Setup

Cookie-first setup is the recommended path:

```bash
xscraper add-cookie my_account 'ct0=CSRF_VALUE; auth_token=AUTH_TOKEN'
xscraper accounts
```

Bulk import from account files:

```bash
xscraper add-accounts ./accounts.txt username:password:email:email_password:_:cookies
```

Accounts with a `ct0` cookie are marked active immediately. Password-only accounts are stored but remain inactive until cookies are imported.

Password/login flow:

```bash
xscraper login_accounts
xscraper login_accounts --manual
xscraper relogin user1 user2 --manual
xscraper relogin_failed
```

For IMAP email-code retrieval, XScraper maps common domains such as iCloud, Outlook/Hotmail, and Yahoo, and falls back to `imap.<domain>`. Use `--manual` for providers that block IMAP or require app-specific setup.

Diagnostics:

```bash
xscraper doctor security
xscraper doctor imap user@icloud.com
xscraper doctor xclid --offline
xscraper doctor xclid
```

## CLI Usage

```bash
xscraper search "rust lang:en" --limit 20
xscraper search-user "x developers" --limit 10
xscraper tweet-details 1649191520250245121
xscraper tweet-replies 1649191520250245121 --limit 20
xscraper retweeters 1649191520250245121 --limit 20
xscraper user-by-login xdevelopers
xscraper user-by-id 2244994945
xscraper following 2244994945 --limit 20
xscraper followers 2244994945 --limit 20
xscraper verified-followers 2244994945 --limit 20
xscraper subscriptions 2244994945 --limit 20
xscraper user-tweets 2244994945 --limit 20
xscraper user-tweets-and-replies 2244994945 --limit 20
xscraper user-media 2244994945 --limit 20
xscraper list-timeline 123456789 --limit 20
xscraper trends news --limit 20
xscraper bookmarks --limit 20
```

Raw GraphQL pages:

```bash
xscraper search "rust lang:en" --limit 20 --raw
xscraper user-by-login xdevelopers --raw
```

Offline parser verification against a JSON payload:

```bash
xscraper parse-fixture ./payload.json tweets --limit 20
xscraper parse-fixture ./payload.json users
xscraper parse-fixture ./payload.json trends --limit 20
```

Output is newline-delimited JSON by default, so it can be redirected or piped:

```bash
xscraper search "from:xdevelopers" --limit 50 > tweets.ndjson
```

## Library Usage

```rust
use xscraper::{AccountsPool, Api, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = AccountsPool::new("accounts.db");
    let api = Api::new(pool);
    let tweets = api.search("rust lang:en", 20, None).await?;

    for tweet in tweets {
        println!("{} {}", tweet.id, tweet.raw_content);
    }

    Ok(())
}
```

## Rate Limit Behavior

Each GraphQL operation is treated as a queue, such as `SearchTimeline`, `UserTweets`, or `TweetDetail`.

1. XScraper locks an active account for the queue before a request.
2. Successful requests unlock the account and increment per-queue stats.
3. Rate-limit and authentication failures move work to another account.
4. Expired or blocked sessions are marked inactive.
5. If no active account is available, the CLI waits unless `XSCRAPER_RAISE_WHEN_NO_ACCOUNT=1` is set.

Inspect state:

```bash
xscraper accounts
xscraper stats
xscraper reset-locks
xscraper delete-inactive
```

## Testing

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
python3 scripts/security_check.py
python3 scripts/xclid_drift_check.py
python3 scripts/live_acceptance.py
```

The test suite uses synthetic payloads under `tests/support/` so it can run without network credentials or external packages.

The live acceptance harness is intentionally environment-gated. It skips without credentials and only performs real X requests when `XSCRAPER_LIVE_QUERY` plus either `XSCRAPER_LIVE_COOKIE_USERNAME` and `XSCRAPER_LIVE_COOKIES`, or `XSCRAPER_LIVE_DB`, are set:

```bash
XSCRAPER_LIVE_QUERY='rust lang:en' \
XSCRAPER_LIVE_COOKIE_USERNAME=my_account \
XSCRAPER_LIVE_COOKIES='ct0=...; auth_token=...' \
python3 scripts/live_acceptance.py
```

Optional live checks are enabled by `XSCRAPER_LIVE_USER_LOGIN`, `XSCRAPER_LIVE_TWEET_ID`, and `XSCRAPER_LIVE_LIMIT`.

## Performance

Parser benchmark:

```bash
cargo bench --bench parser_bench
```

Live scraping is normally dominated by account rate limits, proxy quality, and X response latency, so parser-only benchmarks are the stable local performance signal.

## Project Layout

```text
src/account.rs       account model and X request headers
src/api.rs           public async API and GraphQL operation mapping
src/cli.rs           command-line interface
src/gql.rs           operation ids, feature flags, trend ids
src/models.rs        typed serde models
src/parser.rs        GraphQL response parser
src/pool.rs          account pool facade
src/queue_client.rs  rate-limit-aware HTTP client
src/storage.rs       SQLite migrations and persistence
tests/support/       synthetic parser payloads
benches/             Criterion performance benchmarks
scripts/             production, security, and live acceptance checks
.github/workflows/   CI and multi-platform release packaging
```

## Security and Compliance

XScraper does not bypass authentication. It sends requests using cookies you provide and stores those cookies in the SQLite database you choose. Treat `accounts.db` as sensitive secret material and do not commit it.

Use X/Twitter accounts, proxies, and collected data responsibly. Platform terms, rate limits, local law, and user privacy obligations still apply.

The repository ships `.gitignore` guards for `accounts.db*`, `.env*`, `.local/`, and logs. Run `python3 scripts/security_check.py` or `xscraper doctor security` before publishing a release.
