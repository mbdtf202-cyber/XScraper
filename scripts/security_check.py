#!/usr/bin/env python3
"""Repository-local secret and runtime-file guard for XScraper."""

from __future__ import annotations

import os
import re
import stat
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SENSITIVE_NAMES = {".env", "accounts.db"}
SENSITIVE_SUFFIXES = (".sqlite", ".sqlite3")
SECRET_PATTERNS = [
    re.compile(r"auth_token=[A-Za-z0-9_%.-]{24,}"),
    re.compile(r"ct0=[A-Za-z0-9_%.-]{24,}"),
]


def git(*args: str) -> list[str]:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return [line.strip() for line in result.stdout.splitlines() if line.strip()]


def is_sensitive_path(path: str) -> bool:
    name = Path(path).name
    return (
        name in SENSITIVE_NAMES
        or name.startswith(".env.")
        or name.startswith("accounts.db-")
        or name.endswith(SENSITIVE_SUFFIXES)
    )


def is_text_file(path: Path) -> bool:
    try:
        with path.open("rb") as handle:
            chunk = handle.read(4096)
    except OSError:
        return False
    return b"\0" not in chunk


def check_gitignore() -> list[str]:
    required = {"/accounts.db*", "/.env", "/.env.*", "/.local/"}
    gitignore = ROOT / ".gitignore"
    lines = set(gitignore.read_text().splitlines()) if gitignore.exists() else set()
    return sorted(required - lines)


def check_tracked_files() -> list[str]:
    failures: list[str] = []
    for path in git("ls-files"):
        if is_sensitive_path(path):
            failures.append(f"tracked sensitive runtime file: {path}")
            continue

        full_path = ROOT / path
        if not full_path.exists() or not is_text_file(full_path):
            continue
        text = full_path.read_text(errors="ignore")
        for pattern in SECRET_PATTERNS:
            if pattern.search(text):
                failures.append(f"possible committed session secret in {path}")
                break
    return failures


def check_runtime_files() -> list[str]:
    warnings: list[str] = []
    for candidate in ROOT.glob("accounts.db*"):
        try:
            mode = stat.S_IMODE(candidate.stat().st_mode)
        except OSError:
            continue
        if mode & (stat.S_IRWXG | stat.S_IRWXO):
            warnings.append(f"{candidate.name} is readable by group/others; prefer chmod 600")
    return warnings


def main() -> int:
    failures = [f".gitignore missing {item}" for item in check_gitignore()]
    failures.extend(check_tracked_files())
    warnings = check_runtime_files()

    for warning in warnings:
        print(f"security: warn {warning}")
    if failures:
        for failure in failures:
            print(f"security: fail {failure}", file=sys.stderr)
        return 1

    print("security: ok sensitive runtime files are ignored and untracked")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
