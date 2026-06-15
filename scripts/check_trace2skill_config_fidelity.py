#!/usr/bin/env python3
"""Check Trace2Skill ARA protocol/config evidence against paper and upstream sources."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


EXPECTED_INSTRUCT_CONFIG = {
    "temperature": 1.0,
    "top_p": 1.0,
    "presence_penalty": 2.0,
    "timeout": 600,
    "extra_body": {
        "top_k": 40,
        "min_p": 0.0,
        "repetition_penalty": 1.0,
        "chat_template_kwargs": {"enable_thinking": False},
    },
}

EXPECTED_THINKING_CONFIG = {
    "temperature": 1.0,
    "top_p": 0.95,
    "presence_penalty": 1.5,
    "timeout": 1800,
    "extra_body": {
        "top_k": 20,
        "min_p": 0.0,
        "repetition_penalty": 1.0,
        "chat_template_kwargs": {"enable_thinking": True},
    },
}

EXPECTED_TRAINING_ROWS = {
    "Dataset size": "400 samples",
    "Evolving split": "rows `0..200`",
    "Held-out split": "rows `200..400`",
    "Seeds": "`41`, `42`, `43`",
    "Stage 1 trajectories": "1 trajectory per problem",
    "Stage 2 workers": "128 sub-agents",
    "Merge batch size": "32",
    "ReAct turn budget": "100",
}

EXPECTED_MODEL_ROWS = {
    "Skill author/user model": {"Qwen3.5-122B-A10B", "Qwen3.5-35B-A3B"},
    "Serving backend": {"vLLM"},
    "Multi-turn mode": {"instruct mode"},
    "Single-call mode": {"thinking mode"},
    "Generation config": {"`gen_config/qwen3.5_35B_122B_instruct_reasoning.json`"},
    "Thinking generation config": {"`gen_config/qwen3.5_35B_122B_thinking_reasoning.json`"},
}


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "tmp/repros/trace2skill-upstream").is_dir():
            return candidate
    return Path.cwd()


def markdown_rows(path: Path) -> dict[str, list[str]]:
    rows: dict[str, list[str]] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("|") or line.startswith("|-"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) < 2 or cells[0] == "Field":
            continue
        rows.setdefault(cells[0], []).append(cells[1])
    return rows


def require_contains(errors: list[str], path: Path, needles: list[str]) -> None:
    text = path.read_text(encoding="utf-8")
    for needle in needles:
        if needle not in text:
            errors.append(f"{path} missing source text: {needle}")


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def check_training_rows(errors: list[str], ara_root: Path) -> None:
    training_path = ara_root / "src/configs/training.md"
    if not training_path.is_file():
        errors.append(f"missing training config: {training_path}")
        return
    rows = markdown_rows(training_path)
    for field, expected in EXPECTED_TRAINING_ROWS.items():
        values = rows.get(field)
        if not values:
            errors.append(f"training.md missing row: {field}")
        elif expected not in values:
            errors.append(f"training.md row {field!r} values {values!r} do not include {expected!r}")

    caveat_values = rows.get("Upstream reproduction caveat", [])
    caveat_text = " ".join(caveat_values)
    for needle in ("--merge-batch-size", "default `5`", "paper value `32`"):
        if needle not in caveat_text:
            errors.append(f"training.md upstream caveat must mention {needle}")


def check_model_rows(errors: list[str], ara_root: Path) -> None:
    model_path = ara_root / "src/configs/model.md"
    if not model_path.is_file():
        errors.append(f"missing model config: {model_path}")
        return
    rows = markdown_rows(model_path)
    for field, expected_values in EXPECTED_MODEL_ROWS.items():
        values = set(rows.get(field, []))
        missing = expected_values - values
        if missing:
            errors.append(f"model.md row {field!r} missing values: {sorted(missing)!r}")


def check_source_text(errors: list[str], repo_root: Path) -> None:
    paper = repo_root / "tmp/skill_opt_sources/arx_2603.25158/full_source.md"
    readme = repo_root / "tmp/repros/trace2skill-upstream/README.md"
    evolution_script = repo_root / "tmp/repros/trace2skill-upstream/skill_evolver/run_parallel_skill_evolution.py"
    require_contains(
        errors,
        paper,
        [
            "splitting its 400 samples into 200 for the *evolving set* and 200 held-out for testing",
            "All results are averaged over three random seeds (41, 42, 43)",
            "We experiment with two Qwen3.5 MoE models: Qwen3.5-122B-A10B and Qwen3.5-35B-A3B",
            "Models are served with vLLM",
            "Stage\xa01 generates 1 trajectory per problem",
            "128 sub-agents run in parallel",
            "merge batch size of 32",
            "interaction turn budget to 100",
        ],
    )
    require_contains(
        errors,
        readme,
        [
            "DATA_PATH=data/spreadsheetbench_verified/spreadsheetbench_verified_400",
            "MODEL=Qwen3.5-122B-A10B",
            "WORKERS=128",
            "SEED=41",
            "GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_instruct_reasoning.json",
            "THINK_GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_thinking_reasoning.json",
            "--max_turns 100",
            "--start_idx 0",
            "--end_idx 200",
            "--start_idx 200",
            "--end_idx 400",
        ],
    )
    source = evolution_script.read_text(encoding="utf-8")
    if not re.search(r'"--merge-batch-size",\s*type=int,\s*default=5', source, re.S):
        errors.append("run_parallel_skill_evolution.py no longer shows --merge-batch-size default=5")


def check_dataset_manifest(errors: list[str], ara_root: Path) -> None:
    manifest = load_json(ara_root / "results/dataset_manifest.json")
    if manifest.get("case_count") != 400:
        errors.append("dataset_manifest.json case_count must be 400")
    if manifest.get("case_order", {}).get("first_id") != "13-1":
        errors.append("dataset_manifest.json first case must be 13-1")
    splits = {split.get("name"): split for split in manifest.get("splits", [])}
    expected_splits = {
        "evolving": ("0..200", 200),
        "held_out": ("200..400", 200),
    }
    for name, (range_label, case_count) in expected_splits.items():
        split = splits.get(name)
        if not split:
            errors.append(f"dataset_manifest.json missing split {name}")
            continue
        if split.get("range") != range_label or split.get("case_count") != case_count:
            errors.append(f"dataset_manifest.json split {name} must be {range_label} with {case_count} cases")


def check_generation_configs(errors: list[str], repo_root: Path) -> None:
    configs = {
        "gen_config/qwen3.5_35B_122B_instruct_reasoning.json": EXPECTED_INSTRUCT_CONFIG,
        "gen_config/qwen3.5_35B_122B_thinking_reasoning.json": EXPECTED_THINKING_CONFIG,
    }
    upstream = repo_root / "tmp/repros/trace2skill-upstream"
    for rel, expected in configs.items():
        actual = load_json(upstream / rel)
        if actual != expected:
            errors.append(f"{rel} does not match expected Qwen reproduction config")


def check_run_plan_and_runbook(errors: list[str], ara_root: Path) -> None:
    full_run_plan = (ara_root / "results/full_run_plan.md").read_text(encoding="utf-8")
    for needle in (
        "seeds: [41, 42, 43]",
        "stage2_workers: 128",
        "merge_batch_size: 32",
        "react_turn_budget: 100",
    ):
        if needle not in full_run_plan:
            errors.append(f"full_run_plan.md missing approval packet value {needle!r}")

    runbook = load_json(ara_root / "results/full_denominator_runbook.json")
    protocol = runbook.get("paper_protocol", {})
    expected_protocol = {
        "seeds": [41, 42, 43],
        "workers": 128,
        "merge_batch_size": 32,
        "react_turn_budget": 100,
    }
    for key, expected in expected_protocol.items():
        if protocol.get(key) != expected:
            errors.append(f"full_denominator_runbook.json paper_protocol.{key} must be {expected!r}")

    evolution_commands = [
        command
        for stage in runbook.get("stages", [])
        for command in stage.get("commands", [])
        if "run_parallel_skill_evolution" in command
    ]
    if not evolution_commands:
        errors.append("full_denominator_runbook.json has no skill-evolution command")
    for command in evolution_commands:
        if '--merge-batch-size "$MERGE_BATCH_SIZE"' not in command:
            errors.append("skill-evolution runbook command must pass --merge-batch-size \"$MERGE_BATCH_SIZE\"")


def check_config_fidelity(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    check_training_rows(errors, ara_root)
    check_model_rows(errors, ara_root)
    check_source_text(errors, repo_root)
    check_dataset_manifest(errors, ara_root)
    check_generation_configs(errors, repo_root)
    check_run_plan_and_runbook(errors, ara_root)
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "ara_dir",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench"),
        nargs="?",
    )
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = repo_root_for(ara_root)
    errors = check_config_fidelity(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} config fidelity")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
