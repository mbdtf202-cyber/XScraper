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
| Tweet/User/Trend/List models | typed serde structs |
| X Lists | details, latest/ranked timeline, members, subscribers, user list relations, and list analysis |
| Drift defense | live bundle drift, browser XHR drift probe, and xclid asset diagnostics |
| Account diagnostics | JSON health scores, rate-limit/auth event counts, proxy scoring, and targeted account unlock |
| Evidence | redacted raw-response artifacts for live failures and fixture candidates |
| Batch collection | SQLite checkpoint store for dedupe, cursor resume, and long job recovery |
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

Global CLI flags:

```bash
xscraper --db ./accounts.db --proxy http://127.0.0.1:7890 --debug search "rust lang:en"
```

Set `XSCRAPER_RAISE_WHEN_NO_ACCOUNT=1` in automation so empty or fully locked
pools fail fast instead of waiting.

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
xscraper doctor xclid --offline --json
xscraper doctor xclid
xscraper doctor drift --live
xscraper doctor browser-drift --events ./cdp-events.json --operation search --target "rust"
xscraper --db .local/live-acceptance/live-debug.db doctor browser-drift --live --account x_live_account --operation search --target "rust"
xscraper doctor report --json
```

`doctor drift --live` fetches the current X frontend bundle and compares the
repo's GraphQL operation ids against the live bundle. Run it before claiming a
live scraping release is current.

`doctor browser-drift` is the browser drift probe entrypoint. Feed it captured
Chrome/CDP `Network.requestWillBeSent` events or run it with `--live` to launch
or connect to Chrome/CDP. It compares the real
`/i/api/graphql/{queryId}/{operationName}` XHR variables, features, and field
toggles against the local `src/operations.rs` request spec.

Live browser drift needs a Chrome context that can actually load X search
results. The simplest path is `--account <username>`, which injects that
account's SQLite cookies into the temporary CDP browser. You can also use
`--cdp-url http://127.0.0.1:9222` for a Chrome instance you started with remote
debugging and a logged-in profile, or pass `--user-data-dir` for a dedicated
logged-in profile. A fresh temporary Chrome profile without injected cookies may
hit the login wall and produce a diagnostic `missingLocal` report because no
GraphQL XHR was observed.

`doctor report --json` emits one machine-readable health document covering the
account pool, queue locks, account health scores, proxy scores, offline xclid
diagnostics, and optional browser drift evidence via `--browser-events`.

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
xscraper list-details 123456789
xscraper list-timeline 123456789 --limit 20
xscraper list-ranked-timeline 123456789 --limit 20
xscraper list-members 123456789 --limit 20
xscraper list-subscribers 123456789 --limit 20
xscraper list-ownerships 2244994945 --limit 20
xscraper list-memberships 2244994945 --limit 20
xscraper combined-lists 2244994945 --limit 20
xscraper trends news --limit 20
xscraper bookmarks --limit 20
xscraper analyze-account xdevelopers --days 7 --limit 100
xscraper analyze-list 123456789 --days 7 --limit 100
xscraper compare-accounts xdevelopers rustlang --days 7 --limit 100
```

List commands accept the numeric list id and X/Twitter list URLs such as
`https://x.com/i/lists/123456789`. `list-details` also accepts slug URLs such
as `https://x.com/xdevelopers/lists/rust-team`; timeline, member, and
subscriber commands resolve slug targets through `list-details` before fetching
the timeline that requires a numeric `listId`.

Raw GraphQL pages:

```bash
xscraper search "rust lang:en" --limit 20 --raw
xscraper search-user "x developers" --limit 10 --raw
xscraper search-trend "rust" --limit 20 --raw
xscraper user-by-login xdevelopers --raw
xscraper list-details https://x.com/i/lists/123456789 --raw
xscraper list-members https://x.com/i/lists/123456789 --limit 20 --raw
```

Offline parser verification against a JSON payload:

```bash
xscraper parse-fixture ./payload.json tweets --limit 20
xscraper parse-fixture ./payload.json users
xscraper parse-fixture ./payload.json lists
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

Each GraphQL operation is treated as a queue, such as `SearchTimeline`, `UserTweets`, `TweetDetail`, or `ListMembers`.

1. XScraper locks an active account for the queue before a request.
2. Successful requests unlock the account and increment per-queue stats.
3. Rate-limit and authentication failures move work to another account.
4. Expired or blocked sessions are marked inactive.
5. If no active account is available, the CLI waits unless `XSCRAPER_RAISE_WHEN_NO_ACCOUNT=1` is set.

Inspect state:

```bash
xscraper accounts
xscraper health
xscraper stats
xscraper reset-locks
xscraper unlock-account my_account
xscraper delete-inactive
```

`health` emits a JSON account-pool report with active/inactive counts, queue
availability, per-account locked queues, request totals, health scores,
event-count distributions, and proxy health. Use `unlock-account` for a single
account when a failed run leaves one account locked but the rest of the pool
should remain untouched.

Example health shape:

```json
{
  "total": 2,
  "active": 1,
  "inactive": 1,
  "queues": {
    "SearchTimeline": {
      "active": 1,
      "locked": 1,
      "available": 0
    }
  },
  "accounts": [
    {
      "username": "my_account",
      "logged_in": true,
      "active": true,
      "healthScore": 70,
      "lastUsed": "2026-05-20T00:00:00Z",
      "totalReq": 12,
      "eventCounts": {
        "rate_limited": 1,
        "success": 12
      },
      "reasons": [
        "rate limit events: 1"
      ],
      "lockedQueues": [
        {
          "queue": "SearchTimeline",
          "unlockAt": "2026-05-20T00:15:00Z",
          "secondsRemaining": 900
        }
      ],
      "errorMsg": null
    }
  ],
  "proxies": [
    {
      "proxy": "http://proxy-a:8080",
      "score": 100,
      "successes": 12,
      "failures": 0,
      "eventCounts": {
        "success": 12
      },
      "reasons": []
    }
  ]
}
```

## Evidence and Fixtures

Live failures should leave replayable evidence, not just terminal text.
`scripts/live_acceptance.py` writes `.local/live-acceptance/report.json` on
success and failure, and stores redacted stage evidence under
`.local/live-acceptance/evidence/` when a command fails or emits stderr.

The Rust evidence layer in `src/evidence.rs` can persist redacted JSON/text
payloads with a manifest. Use it for raw GraphQL responses, headers, operation
metadata, account identity, X error codes, and parser fixture candidates. The
redactor removes cookies, auth headers, proxy credentials, and common token
fields before anything is written.

## Batch Jobs and Checkpointing

`src/jobs.rs` provides the long-run collection primitive:

- one SQLite job database per batch run;
- deterministic fingerprints to dedupe identical operation/target/cursor items;
- per-operation checkpoints with cursor and last seen id;
- pending-item resume for keyword, account, or list batches.

This is intentionally scoped to X GraphQL operation work. It is not a generic
web crawler framework.

## GraphQL Operation Maintenance

GraphQL request definitions live in [`src/operations.rs`](src/operations.rs).
That file is the single source for operation ids, variables, feature overrides,
field toggles, cursor type, and item-versus-timeline mode. Public API methods
and raw CLI paths call through that spec so `search`, `search-user`,
`search-trend`, list methods, timeline methods, and diagnostics do not drift
apart.

When X changes its frontend contract:

1. Run `xscraper doctor drift --live`.
2. Capture real browser/CDP XHRs and run `xscraper doctor browser-drift --events ./cdp-events.json`.
3. Update operation ids or request metadata in `src/operations.rs` or
   `src/gql.rs`.
4. Run `python3 scripts/release_gate.py --json --live-drift`.
5. If live credentials are available, run `scripts/live_acceptance.py` with a
   real cookie session and inspect `.local/live-acceptance/report.json`.

## Testing

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
python3 scripts/security_check.py
python3 scripts/xclid_drift_check.py
python3 scripts/live_acceptance.py
python3 scripts/release_gate.py
python3 scripts/release_gate.py --json
python3 scripts/release_gate.py --live-drift
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

`scripts/live_acceptance.py` writes `.local/live-acceptance/report.json` on
success and failure. The report is structured by stage (`auth`, `search`,
optional user/tweet probes, parser checks, and account-pool health snapshots),
so failed live runs leave a diagnostic artifact instead of only stderr output.

CI runs `python3 scripts/release_gate.py`, which covers formatting, clippy,
all targets and features, the repository security guard, offline xclid drift
check, and live acceptance harness skip mode. `--live-drift` is intentionally
manual because it depends on the current external X frontend.

`python3 scripts/release_gate.py --json` also writes
`.local/release-gate/report.json` with step names, commands, return codes, and
the final gate result.

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
src/browser_probe.rs CDP/XHR GraphQL drift parser
src/cli.rs           command-line interface
src/diagnostics.rs   unified machine-readable doctor reports
src/evidence.rs      redacted raw-response evidence manifests
src/fetch_profile.rs scoped timeout/proxy/header/request-id profile
src/gql.rs           operation ids, feature flags, trend ids
src/jobs.rs          batch job dedupe and checkpoint storage
src/lists.rs         X List target parsing for ids and URLs
src/operations.rs    canonical GraphQL request specs
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
