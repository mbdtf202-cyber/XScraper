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
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


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


def output_dir() -> Path:
    out_dir = ROOT / ".local" / "live-acceptance"
    out_dir.mkdir(parents=True, exist_ok=True)
    return out_dir


def write_report(report: dict[str, Any]) -> Path:
    out_path = output_dir() / "report.json"
    out_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return out_path


def evidence_dir() -> Path:
    out_dir = output_dir() / "evidence"
    out_dir.mkdir(parents=True, exist_ok=True)
    return out_dir


def redact_text(raw: str) -> str:
    raw = re.sub(r"auth_token=[^;\s]+", "auth_token=<redacted>", raw, flags=re.I)
    raw = re.sub(r"ct0=[^;\s]+", "ct0=<redacted>", raw, flags=re.I)
    raw = re.sub(r"authorization:\s*[^\n]+", "authorization: <redacted>", raw, flags=re.I)
    raw = re.sub(r"//[^/\s:@]+:[^@\s/]+@", "//<redacted>@", raw)
    return raw


def write_evidence(report: dict[str, Any], name: str, result: subprocess.CompletedProcess[str]) -> dict[str, Any]:
    body = {
        "name": name,
        "returncode": result.returncode,
        "stdout": redact_text(result.stdout),
        "stderr": redact_text(result.stderr),
    }
    path = evidence_dir() / f"{len(report.get('evidence', [])) + 1:02d}-{name}.json"
    path.write_text(json.dumps(body, indent=2, sort_keys=True) + "\n")
    ref = {"name": name, "path": str(path.relative_to(ROOT))}
    report.setdefault("evidence", []).append(ref)
    return ref


def redact_command(cmd: list[str]) -> str:
    if "add-cookie" not in cmd:
        return " ".join(cmd)
    redacted = list(cmd)
    idx = redacted.index("add-cookie")
    if len(redacted) > idx + 2:
        redacted[idx + 2] = "<redacted-cookies>"
    return " ".join(redacted)


def run_stage(
    report: dict[str, Any],
    name: str,
    cmd: list[str],
    env: dict[str, str],
    parser=None,
) -> tuple[bool, subprocess.CompletedProcess[str], Any]:
    result = run(cmd, env)
    stage: dict[str, Any] = {
        "name": name,
        "command": redact_command(cmd),
        "returncode": result.returncode,
        "ok": result.returncode == 0,
    }
    parsed: Any = None
    if result.stdout.strip():
        stage["stdoutPreview"] = result.stdout[:500]
    if result.stderr.strip():
        stage["stderrPreview"] = result.stderr[:500]
    if result.returncode != 0 or result.stderr.strip():
        stage["evidence"] = write_evidence(report, name, result)

    if result.returncode == 0 and parser is not None:
        try:
            parsed = parser(result.stdout)
            stage.update(parsed if isinstance(parsed, dict) else {"parsed": parsed})
        except Exception as exc:  # noqa: BLE001 - report parser failures as harness failures.
            stage["ok"] = False
            stage["parseError"] = str(exc)
            stage["evidence"] = write_evidence(report, f"{name}-parse-error", result)

    report["stages"].append(stage)
    return bool(stage["ok"]), result, parsed


def health_snapshot(base: list[str], env: dict[str, str]) -> dict[str, Any] | None:
    result = run(base + ["health"], env)
    if result.returncode != 0:
        return {"ok": False, "stderrPreview": result.stderr[:500]}
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        return {"ok": False, "parseError": str(exc), "stdoutPreview": result.stdout[:500]}
    return {"ok": True, "value": value}


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

    report: dict[str, Any] = {
        "db": str(db),
        "query": query,
        "ok": False,
        "stages": [],
        "evidence": [],
        "health": {},
    }

    try:
        if username and cookies:
            ok, result, _ = run_stage(report, "auth.add-cookie", base + ["add-cookie", username, cookies], env)
            if not ok:
                report["health"]["afterFailure"] = health_snapshot(base, env)
                path = write_report(report)
                print(result.stderr, file=sys.stderr)
                print(f"live acceptance: failed report={path}", file=sys.stderr)
                return result.returncode or 1

        report["health"]["beforeSearch"] = health_snapshot(base, env)
        ok, result, parsed = run_stage(
            report,
            "search",
            base + ["search", query, "--limit", os.environ.get("XSCRAPER_LIVE_LIMIT", "3")],
            env,
            lambda stdout: {"count": len(parse_ndjson(stdout)), "rows": parse_ndjson(stdout)[:3]},
        )
        if not ok:
            report["health"]["afterFailure"] = health_snapshot(base, env)
            path = write_report(report)
            print(result.stderr, file=sys.stderr)
            print(f"live acceptance: failed report={path}", file=sys.stderr)
            return result.returncode or 1
        tweets = parsed["rows"] if isinstance(parsed, dict) else []
        if not tweets:
            report["stages"].append({"name": "parser.nonempty-search", "ok": False, "count": 0})
            report["health"]["afterFailure"] = health_snapshot(base, env)
            path = write_report(report)
            print("live acceptance: search returned no tweets", file=sys.stderr)
            print(f"live acceptance: failed report={path}", file=sys.stderr)
            return 1
        report["stages"].append({"name": "parser.nonempty-search", "ok": True, "count": len(tweets)})

        login = os.environ.get("XSCRAPER_LIVE_USER_LOGIN")
        if login:
            ok, result, _ = run_stage(
                report,
                "user-by-login",
                base + ["user-by-login", login],
                env,
                lambda stdout: {"hasOutput": bool(stdout.strip())},
            )
            if not ok:
                report["health"]["afterFailure"] = health_snapshot(base, env)
                path = write_report(report)
                print(result.stderr, file=sys.stderr)
                print(f"live acceptance: failed report={path}", file=sys.stderr)
                return result.returncode or 1

        tweet_id = os.environ.get("XSCRAPER_LIVE_TWEET_ID")
        if tweet_id:
            ok, result, _ = run_stage(
                report,
                "tweet-details",
                base + ["tweet-details", tweet_id],
                env,
                lambda stdout: {"hasOutput": bool(stdout.strip())},
            )
            if not ok:
                report["health"]["afterFailure"] = health_snapshot(base, env)
                path = write_report(report)
                print(result.stderr, file=sys.stderr)
                print(f"live acceptance: failed report={path}", file=sys.stderr)
                return result.returncode or 1

        report["health"]["afterSuccess"] = health_snapshot(base, env)
        report["ok"] = all(stage.get("ok") for stage in report["stages"])
        path = write_report(report)
        print(f"live acceptance: ok report={path}")
        return 0
    finally:
        if temp_dir is not None:
            temp_dir.cleanup()


if __name__ == "__main__":
    raise SystemExit(main())
