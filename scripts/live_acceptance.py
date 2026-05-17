#!/usr/bin/env python3
"""Environment-gated live acceptance runner for XScraper.

The script skips without credentials. To exercise real X endpoints, provide a
cookie session or an existing DB with active accounts:

  XSCRAPER_LIVE_QUERY='rust lang:en' \
  XSCRAPER_LIVE_COOKIE_USERNAME=my_account \
  XSCRAPER_LIVE_COOKIES='ct0=...; auth_token=...' \
  python3 scripts/live_acceptance.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def xscraper_cmd() -> list[str]:
    override = os.environ.get("XSCRAPER_BIN")
    if override:
        return [override]
    binary = ROOT / "target" / "debug" / ("xscraper.exe" if os.name == "nt" else "xscraper")
    if binary.exists():
        return [str(binary)]
    return ["cargo", "run", "--quiet", "--"]


def run(cmd: list[str], env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def parse_ndjson(text: str) -> list[dict]:
    rows = []
    for line in text.splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return rows


def main() -> int:
    query = os.environ.get("XSCRAPER_LIVE_QUERY")
    username = os.environ.get("XSCRAPER_LIVE_COOKIE_USERNAME")
    cookies = os.environ.get("XSCRAPER_LIVE_COOKIES")
    existing_db = os.environ.get("XSCRAPER_LIVE_DB") or os.environ.get("XSCRAPER_DB")

    if not query or not ((username and cookies) or existing_db):
        print(
            "live acceptance: skipped; set XSCRAPER_LIVE_QUERY and either "
            "XSCRAPER_LIVE_COOKIE_USERNAME+XSCRAPER_LIVE_COOKIES or XSCRAPER_LIVE_DB"
        )
        return 0

    temp_dir: tempfile.TemporaryDirectory[str] | None = None
    if existing_db:
        db = Path(existing_db)
    else:
        temp_dir = tempfile.TemporaryDirectory()
        db = Path(temp_dir.name) / "accounts.db"

    env = os.environ.copy()
    env["XSCRAPER_DB"] = str(db)
    env.setdefault("XSCRAPER_RAISE_WHEN_NO_ACCOUNT", "1")
    base = xscraper_cmd()

    report: dict[str, object] = {"db": str(db), "query": query, "checks": []}

    try:
        if username and cookies:
            result = run(base + ["add-cookie", username, cookies], env)
            if result.returncode != 0:
                print(result.stderr, file=sys.stderr)
                return result.returncode
            report["checks"].append({"command": "add-cookie", "ok": True})

        result = run(base + ["search", query, "--limit", os.environ.get("XSCRAPER_LIVE_LIMIT", "3")], env)
        if result.returncode != 0:
            print(result.stderr, file=sys.stderr)
            return result.returncode
        tweets = parse_ndjson(result.stdout)
        if not tweets:
            print("live acceptance: search returned no tweets", file=sys.stderr)
            return 1
        report["checks"].append({"command": "search", "count": len(tweets)})

        login = os.environ.get("XSCRAPER_LIVE_USER_LOGIN")
        if login:
            result = run(base + ["user-by-login", login], env)
            if result.returncode != 0:
                print(result.stderr, file=sys.stderr)
                return result.returncode
            report["checks"].append({"command": "user-by-login", "ok": bool(result.stdout.strip())})

        tweet_id = os.environ.get("XSCRAPER_LIVE_TWEET_ID")
        if tweet_id:
            result = run(base + ["tweet-details", tweet_id], env)
            if result.returncode != 0:
                print(result.stderr, file=sys.stderr)
                return result.returncode
            report["checks"].append({"command": "tweet-details", "ok": bool(result.stdout.strip())})

        out_dir = ROOT / ".local" / "live-acceptance"
        out_dir.mkdir(parents=True, exist_ok=True)
        (out_dir / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"live acceptance: ok report={out_dir / 'report.json'}")
        return 0
    finally:
        if temp_dir is not None:
            temp_dir.cleanup()


if __name__ == "__main__":
    raise SystemExit(main())
