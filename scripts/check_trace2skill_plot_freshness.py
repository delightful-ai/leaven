#!/usr/bin/env python3
"""Check that the committed Trace2Skill target plot is freshly renderable."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from pathlib import Path


PLOT_FILE = "plots/trace2skill_targets.png"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def render_plot(repo_root: Path, ara_root: Path, output: Path) -> subprocess.CompletedProcess[str]:
    command = [
        "uv",
        "run",
        "--with",
        "matplotlib",
        "--with",
        "pandas",
        "python",
        str(repo_root / "scripts/plot_trace2skill_ara.py"),
        str(ara_root),
        "--output",
        str(output),
    ]
    return subprocess.run(command, cwd=repo_root, text=True, capture_output=True, check=False)


def check_plot_freshness(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    committed_plot = ara_root / PLOT_FILE
    if not committed_plot.is_file():
        return [f"missing committed target plot: {committed_plot}"]

    with tempfile.TemporaryDirectory(prefix="trace2skill-plot-freshness-") as temp:
        rendered_plot = Path(temp) / "trace2skill_targets.png"
        result = render_plot(repo_root, ara_root, rendered_plot)
        if result.returncode != 0:
            detail = "\n".join(part for part in [result.stdout.strip(), result.stderr.strip()] if part)
            return [f"plot freshness render failed with exit {result.returncode}: {detail}"]
        if not rendered_plot.is_file():
            return [f"plotter did not create expected temp plot: {rendered_plot}"]
        if not rendered_plot.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"):
            return [f"temp plot is not a PNG: {rendered_plot}"]
        committed_hash = sha256_file(committed_plot)
        rendered_hash = sha256_file(rendered_plot)
        if committed_hash != rendered_hash:
            errors.append(
                f"{committed_plot} is stale; regenerate with "
                "uv run --with matplotlib --with pandas python "
                "scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench "
                f"(committed sha256={committed_hash}, rendered sha256={rendered_hash})"
            )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = Path(__file__).resolve().parents[1]
    errors = check_plot_freshness(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(f"PASS: {args.ara_dir} plot freshness")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
