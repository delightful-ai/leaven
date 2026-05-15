#!/usr/bin/env python3
"""Enforce production Rust source file size limits."""

from __future__ import annotations

import argparse
import os
from pathlib import Path


WARN_LIMIT = 800
FAIL_LIMIT = 1000
SKIP_DIRS = {
    ".git",
    ".jj",
    "target",
    # External dependency clones (e.g. the vendored Codex tree) live behind a
    # symlinked `vendor/` directory and are not Leaven production source.
    "vendor",
}


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Warn when non-test Rust files exceed 800 effective lines and fail "
            "when they exceed 1000 effective lines."
        )
    )
    parser.add_argument("--root", default=".", help="repository root to scan")
    parser.add_argument("--warn", type=int, default=WARN_LIMIT, help="warning threshold")
    parser.add_argument("--fail", type=int, default=FAIL_LIMIT, help="failure threshold")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    warnings: list[tuple[int, Path]] = []
    failures: list[tuple[int, Path]] = []

    for path in rust_files(root):
        effective_lines = count_production_lines(path)
        relative = path.relative_to(root)
        if effective_lines > args.fail:
            failures.append((effective_lines, relative))
        elif effective_lines > args.warn:
            warnings.append((effective_lines, relative))

    for count, path in sorted(warnings, reverse=True):
        print(f"warning: {path} has {count} production lines (warn>{args.warn})")
    for count, path in sorted(failures, reverse=True):
        print(f"error: {path} has {count} production lines (fail>{args.fail})")

    return 1 if failures else 0


def rust_files(root: Path):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [
            dirname
            for dirname in dirnames
            if dirname not in SKIP_DIRS and not is_test_path(Path(dirpath, dirname), root)
        ]
        current = Path(dirpath)
        for filename in filenames:
            if not filename.endswith(".rs"):
                continue
            path = current / filename
            if is_test_path(path, root):
                continue
            yield path


def is_test_path(path: Path, root: Path) -> bool:
    relative = path.resolve().relative_to(root)
    parts = relative.parts
    return (
        "tests" in parts
        or "benches" in parts
        or "examples" in parts
        or relative.name == "tests.rs"
        or relative.name.endswith("_test.rs")
    )


def count_production_lines(path: Path) -> int:
    lines = path.read_text(encoding="utf-8").splitlines()
    ignored = ignored_cfg_test_lines(lines)
    return sum(1 for index, line in enumerate(lines) if index not in ignored and line.strip())


def ignored_cfg_test_lines(lines: list[str]) -> set[int]:
    ignored: set[int] = set()
    pending_cfg_test: list[int] = []
    index = 0
    while index < len(lines):
        stripped = lines[index].strip()
        if stripped.startswith("#[cfg(test)]"):
            pending_cfg_test.append(index)
            index += 1
            continue
        if pending_cfg_test and stripped.startswith("mod ") and "{" in stripped:
            start = pending_cfg_test[0]
            end = matching_block_end(lines, index)
            ignored.update(range(start, end + 1))
            pending_cfg_test.clear()
            index = end + 1
            continue
        if stripped and not stripped.startswith("#["):
            pending_cfg_test.clear()
        index += 1
    return ignored


def matching_block_end(lines: list[str], start: int) -> int:
    depth = 0
    seen_open = False
    for index in range(start, len(lines)):
        code = strip_line_comment(lines[index])
        for char in code:
            if char == "{":
                depth += 1
                seen_open = True
            elif char == "}":
                depth -= 1
                if seen_open and depth == 0:
                    return index
    return len(lines) - 1


def strip_line_comment(line: str) -> str:
    in_string = False
    escaped = False
    for index, char in enumerate(line):
        if escaped:
            escaped = False
            continue
        if char == "\\":
            escaped = True
            continue
        if char == '"':
            in_string = not in_string
            continue
        if not in_string and line[index : index + 2] == "//":
            return line[:index]
    return line


if __name__ == "__main__":
    raise SystemExit(main())
