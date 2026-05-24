#!/usr/bin/env python3
"""Run the local release gate for XScraper."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


REPORT_PATH = ROOT / ".local" / "release-gate" / "report.json"


def run_step(name: str, cmd: list[str]) -> dict[str, object]:
    print(f"release gate: {name}")
    result = subprocess.run(cmd, cwd=ROOT)
    step = {
        "name": name,
        "command": " ".join(cmd),
        "returncode": result.returncode,
        "ok": result.returncode == 0,
    }
    if result.returncode != 0:
        print(f"release gate: failed {name}", file=sys.stderr)
    return step


def write_report(report: dict[str, object], path: Path = REPORT_PATH) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return path


def main() -> int:
    parser = argparse.ArgumentParser(description="Run XScraper release validation gates.")
    parser.add_argument(
        "--live-drift",
        action="store_true",
        help="also fetch the current X frontend and compare GraphQL operation ids",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help=f"write a machine-readable report to {REPORT_PATH}",
    )
    args = parser.parse_args()

    steps = [
        ("format", ["cargo", "fmt", "--all", "--", "--check"]),
        ("clippy", ["cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings"]),
        ("tests", ["cargo", "test", "--all-targets", "--all-features"]),
        ("security", ["python3", "scripts/security_check.py"]),
        ("xclid-offline", ["python3", "scripts/xclid_drift_check.py"]),
        ("live-acceptance-harness", ["python3", "scripts/live_acceptance.py"]),
    ]
    if args.live_drift:
        steps.append(("live-drift", ["cargo", "run", "--quiet", "--", "doctor", "drift", "--live"]))

    report: dict[str, object] = {
        "createdAt": datetime.now(timezone.utc).isoformat(),
        "ok": False,
        "liveDrift": args.live_drift,
        "steps": [],
    }

    for name, cmd in steps:
        step = run_step(name, cmd)
        report["steps"].append(step)  # type: ignore[index]
        if not step["ok"]:
            if args.json:
                path = write_report(report)
                print(f"release gate: report={path}", file=sys.stderr)
            return 1

    report["ok"] = True
    if args.json:
        path = write_report(report)
        print(f"release gate: report={path}")
    print("release gate: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
