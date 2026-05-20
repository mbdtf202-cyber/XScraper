#!/usr/bin/env python3
"""Run the local release gate for XScraper."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_step(name: str, cmd: list[str]) -> bool:
    print(f"release gate: {name}")
    result = subprocess.run(cmd, cwd=ROOT)
    if result.returncode != 0:
        print(f"release gate: failed {name}", file=sys.stderr)
        return False
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description="Run XScraper release validation gates.")
    parser.add_argument(
        "--live-drift",
        action="store_true",
        help="also fetch the current X frontend and compare GraphQL operation ids",
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

    for name, cmd in steps:
        if not run_step(name, cmd):
            return 1

    print("release gate: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
