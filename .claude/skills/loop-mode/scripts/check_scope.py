#!/usr/bin/env python3
"""Verify that all worktree changes stay inside task allowlist patterns."""

from __future__ import annotations

import argparse
import fnmatch
import json
import subprocess
import sys
from pathlib import Path


def run_git(repo: Path, *args: str) -> bytes:
    try:
        return subprocess.check_output(
            ["git", "-C", str(repo), *args], stderr=subprocess.STDOUT
        )
    except subprocess.CalledProcessError as exc:
        message = exc.output.decode("utf-8", errors="replace").strip()
        raise RuntimeError(message or f"git {' '.join(args)} failed") from exc


def nul_paths(raw: bytes) -> set[str]:
    return {
        item.decode("utf-8", errors="surrogateescape")
        for item in raw.split(b"\0")
        if item
    }


def normalize_pattern(pattern: str) -> str:
    normalized = pattern.removeprefix("./")
    return f"{normalized}**" if normalized.endswith("/") else normalized


def is_allowed(path: str, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check committed, staged, unstaged, and untracked paths against an allowlist."
    )
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--base", required=True, help="Base ref or commit")
    parser.add_argument(
        "--allow", action="append", required=True, help="Allowed POSIX-style glob; repeatable"
    )
    parser.add_argument("--json", action="store_true", help="Emit machine-readable output")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo = Path(
        run_git(args.repo, "rev-parse", "--show-toplevel")
        .decode("utf-8")
        .strip()
    )
    run_git(repo, "rev-parse", "--verify", f"{args.base}^{{commit}}")

    changed = set()
    changed |= nul_paths(run_git(repo, "diff", "--name-only", "-z", f"{args.base}...HEAD"))
    changed |= nul_paths(run_git(repo, "diff", "--name-only", "-z", "HEAD"))
    changed |= nul_paths(run_git(repo, "ls-files", "--others", "--exclude-standard", "-z"))

    patterns = [normalize_pattern(pattern) for pattern in args.allow]
    violations = sorted(path for path in changed if not is_allowed(path, patterns))
    result = {
        "repo": str(repo),
        "base": args.base,
        "allow": patterns,
        "changed_count": len(changed),
        "violation_count": len(violations),
        "violations": violations,
    }

    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    elif violations:
        print(f"scope check failed: {len(violations)} of {len(changed)} changed path(s) violate the allowlist")
        for path in violations[:50]:
            print(f"- {path}")
        if len(violations) > 50:
            print(f"- ... and {len(violations) - 50} more")
    else:
        print(f"scope check passed: {len(changed)} changed path(s)")

    return 1 if violations else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as exc:
        print(f"scope check error: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc

