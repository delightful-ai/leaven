"""Workspace package groups shared by repository automation scripts."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

MILESTONE_PACKAGES = [
    "p0_graph_skeleton",
    "p1_keep_best",
    "p2_pairwise_tournament",
    "p3_gepa_parity",
    "p4_meta_harness_lite",
    "p5_evoskill_iteration",
    "p5_skill_paper_reproductions",
    "p6_optimizer_policy_self_opt",
    "p7_self_optimization_kernel",
    "p8_aime_gepa",
    "trace2skill_spreadsheetbench",
]


def package_exclude_args(packages: list[str]) -> list[str]:
    args: list[str] = []
    for package in packages:
        args.extend(["--exclude", package])
    return args


def cargo_metadata(workspace_root: Path | None = None) -> dict:
    metadata = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=workspace_root,
        check=False,
        stdout=subprocess.PIPE,
        text=True,
    )
    if metadata.returncode != 0:
        raise SystemExit(metadata.returncode)
    return json.loads(metadata.stdout)
