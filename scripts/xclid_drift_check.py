#!/usr/bin/env python3
"""Check that x-client-transaction-id generation is callable."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--live", action="store_true", help="fetch current x.com xclid assets")
    args = parser.parse_args()

    cmd = xscraper_cmd() + ["doctor", "xclid"]
    if not args.live:
        cmd.append("--offline")
    result = subprocess.run(cmd, cwd=ROOT, text=True)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
