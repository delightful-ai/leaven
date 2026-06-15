#!/usr/bin/env python3
"""Check the Trace2Skill official-eval importer against a no-spend fixture."""

from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


EXPECTED_METRICS = {
    "official_instance_accuracy": 50.0,
    "official_test_case_accuracy": 75.0,
    "official_avg_soft_score": 75.0,
    "official_avg_hard_score": 50.0,
}
APPROVAL_ARTIFACT = "docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md"
ARA_DIR = "docs/ara/trace2skill_spreadsheetbench"
FIXTURE = "scripts/fixtures/trace2skill_eval_official_results_sample.json"


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / FIXTURE).is_file():
            return candidate
    return Path.cwd()


def import_result_intake_checker(repo_root: Path) -> Any:
    path = repo_root / "scripts/check_trace2skill_result_intake.py"
    spec = importlib.util.spec_from_file_location("check_trace2skill_result_intake", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def touch(repo_root: Path, path: Path, text: str) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path.relative_to(repo_root).as_posix()


def prompt_artifact_paths(repo_root: Path, tmp_path: Path) -> list[str]:
    prompt = tmp_path / "subset_200_202_seed_41/rendered_prompts/52807/agent_prompt.md"
    manifest = tmp_path / "subset_200_202_seed_41/prompt_render_manifest.json"
    return [
        touch(repo_root, prompt, "fixture rendered agent prompt\n"),
        touch(repo_root, manifest, '{"schema_version":"fixture.prompt_manifest.v1"}\n'),
    ]


def skill_artifact_path(repo_root: Path, tmp_path: Path) -> str:
    skill = tmp_path / "subset_200_202_seed_41/skill/SKILL.md"
    return touch(repo_root, skill, "# Fixture Trace2Skill Skill\n\nNo-spend fixture skill.\n")


def eval_results_artifact_path(repo_root: Path, tmp_path: Path) -> str:
    eval_results = tmp_path / "subset_200_202_seed_41/outputs/eval_official_results.json"
    return touch(repo_root, eval_results, (repo_root / FIXTURE).read_text(encoding="utf-8"))


def mutated_eval_results_artifact_path(repo_root: Path, tmp_path: Path, mutate: Any) -> str:
    eval_results = tmp_path / "subset_200_202_seed_41/outputs/eval_official_results.json"
    payload = json.loads((repo_root / FIXTURE).read_text(encoding="utf-8"))
    mutate(payload)
    return touch(repo_root, eval_results, json.dumps(payload, indent=2) + "\n")


def importer_base_args(
    output: Path,
    artifact_paths: list[str] | None = None,
    eval_results: str | None = None,
    skill_path: str | None = None,
) -> list[str]:
    artifact_args: list[str] = []
    for artifact_path in artifact_paths or []:
        artifact_args.extend(["--artifact-path", artifact_path])
    skill_args = ["--skill-path", skill_path] if skill_path is not None else []
    return [
        "scripts/import_trace2skill_eval_results.py",
        "--eval-results",
        eval_results or FIXTURE,
        "--output",
        output.as_posix(),
        "--ara-dir",
        ARA_DIR,
        "--run-id",
        "trace2skill-import-fixture",
        "--created-at",
        "2026-06-14T00:00:02Z",
        "--proof-classification",
        "paper-subset",
        "--runbook-stage-id",
        "G2",
        "--split",
        "held_out",
        "--case-range",
        "200..202",
        "--case-count",
        "2",
        "--denominator",
        "fixture-held-out-subset-not-paper",
        "--model-id",
        "Qwen3.5-122B-A10B",
        "--serving-backend",
        "vLLM",
        "--seed",
        "41",
        "--workers",
        "128",
        "--max-turns",
        "100",
        "--skill-kind",
        "fixture-skill",
        *skill_args,
        *artifact_args,
        "--approval-artifact-path",
        APPROVAL_ARTIFACT,
        "--command-policy",
        "upstream-eval",
        "--source-command",
        "python run_spreadsheetbench.py --model Qwen3.5-122B-A10B --workers 128 "
        "--max_turns 100 --seeds 41 --start_idx 200 --end_idx 202 "
        "&& python evaluate_with_official.py --start_idx 200 --end_idx 202",
    ]


def run_importer(repo_root: Path, args: list[str]) -> subprocess.CompletedProcess[str]:
    command = [sys.executable, *args]
    print("$ " + " ".join(command))
    return subprocess.run(
        command,
        cwd=repo_root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            loaded = json.loads(line)
            if not isinstance(loaded, dict):
                raise ValueError(f"{path} contains a non-object JSONL row")
            rows.append(loaded)
    return rows


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")


def expect_failure(
    repo_root: Path,
    args: list[str],
    expected_stderr: str,
    errors: list[str],
    label: str,
) -> None:
    result = run_importer(repo_root, args)
    if result.returncode == 0:
        errors.append(f"{label}: importer unexpectedly succeeded")
        return
    if expected_stderr not in result.stderr:
        errors.append(
            f"{label}: stderr did not contain {expected_stderr!r}; stderr was {result.stderr!r}"
        )


def check_positive_import(repo_root: Path, output: Path, errors: list[str]) -> None:
    artifact_paths = prompt_artifact_paths(repo_root, output.parent)
    eval_results = eval_results_artifact_path(repo_root, output.parent)
    skill_path = skill_artifact_path(repo_root, output.parent)
    result = run_importer(repo_root, importer_base_args(output, artifact_paths, eval_results, skill_path))
    if result.returncode != 0:
        errors.append(f"positive import failed: {result.stderr.strip()}")
        return

    try:
        rows = load_jsonl(output)
    except (json.JSONDecodeError, ValueError) as exc:
        errors.append(str(exc))
        return

    if len(rows) != 4:
        errors.append(f"positive import wrote {len(rows)} rows, expected 4")
        return

    observed = {row.get("metric_name"): row for row in rows}
    if set(observed) != set(EXPECTED_METRICS):
        errors.append(f"positive import metric names were {sorted(observed)!r}")
        return

    for metric_name, expected_value in EXPECTED_METRICS.items():
        row = observed[metric_name]
        prefix = f"positive import {metric_name}"
        if row.get("proof_classification") != "paper-subset":
            errors.append(f"{prefix}: proof_classification is {row.get('proof_classification')!r}")
        if row.get("metric_value") != expected_value:
            errors.append(f"{prefix}: metric_value is {row.get('metric_value')!r}")
        if row.get("metric_unit") != "percent":
            errors.append(f"{prefix}: metric_unit is {row.get('metric_unit')!r}")
        if row.get("plot_binding") is not None:
            errors.append(f"{prefix}: plot_binding must stay null for fixture rows")

        artifact_paths = row.get("artifact_paths")
        if not isinstance(artifact_paths, list):
            errors.append(f"{prefix}: artifact_paths is not a list")
            continue
        for expected_path in (
            eval_results,
            skill_path,
            APPROVAL_ARTIFACT,
            *prompt_artifact_paths(repo_root, output.parent),
        ):
            if expected_path not in artifact_paths:
                errors.append(f"{prefix}: artifact_paths missing {expected_path}")

        skill_source = row.get("skill_source")
        if not isinstance(skill_source, dict):
            errors.append(f"{prefix}: skill_source is not an object")
        elif skill_source.get("path") != skill_path:
            errors.append(f"{prefix}: skill_source.path is {skill_source.get('path')!r}")

        extra = row.get("extra")
        if not isinstance(extra, dict):
            errors.append(f"{prefix}: extra is not an object")
            continue
        if extra.get("runbook_stage_id") != "G2":
            errors.append(f"{prefix}: extra.runbook_stage_id is {extra.get('runbook_stage_id')!r}")
        if extra.get("approval_artifact_paths") != [APPROVAL_ARTIFACT]:
            errors.append(f"{prefix}: approval_artifact_paths is {extra.get('approval_artifact_paths')!r}")
        if not isinstance(extra.get("source_metric"), str):
            errors.append(f"{prefix}: extra.source_metric missing")


def runbook_stages(ara_root: Path) -> dict[str, dict[str, Any]]:
    runbook = json.loads((ara_root / "results/full_denominator_runbook.json").read_text(encoding="utf-8"))
    return {
        stage.get("id"): stage
        for stage in runbook.get("stages", [])
        if isinstance(stage, dict)
    }


def check_runbook_stage(repo_root: Path, ara_root: Path, errors: list[str]) -> None:
    stages = runbook_stages(ara_root)
    stage = stages.get("G2")
    if stage is None:
        errors.append("runbook stage G2 is missing")
    elif stage.get("allowed_label") != "paper-subset":
        errors.append(f"runbook stage G2 allowed_label is {stage.get('allowed_label')!r}")
    if not (repo_root / APPROVAL_ARTIFACT).is_file():
        errors.append(f"approval artifact is not inspectable: {APPROVAL_ARTIFACT}")


def check_result_intake_for_rows(
    repo_root: Path,
    ara_root: Path,
    output: Path,
    expected_error: str | None,
    errors: list[str],
    label: str,
) -> None:
    checker = import_result_intake_checker(repo_root)
    stages = runbook_stages(ara_root)
    intake_errors: list[str] = []
    try:
        rows = load_jsonl(output)
    except (json.JSONDecodeError, ValueError) as exc:
        errors.append(f"{label}: {exc}")
        return
    for line_number, row in enumerate(rows, start=1):
        checker.check_record(
            repo_root,
            output,
            line_number,
            row,
            stages,
            intake_errors,
            source_approval_blockers=checker.approval_packet_errors(ara_root),
        )
    if expected_error is None:
        if intake_errors:
            errors.append(f"{label}: result-intake errors: {intake_errors!r}")
    elif not any(expected_error in error for error in intake_errors):
        errors.append(f"{label}: expected intake error {expected_error!r}, got {intake_errors!r}")


def check_real_results_require_runnable_approval(
    repo_root: Path,
    ara_root: Path,
    tmp_path: Path,
    errors: list[str],
) -> None:
    output = ara_root / "results/fixture_blocked_approval_import.jsonl"
    if output.exists():
        output.unlink()
    artifact_paths = prompt_artifact_paths(repo_root, tmp_path / "blocked_approval")
    eval_results = eval_results_artifact_path(repo_root, tmp_path / "blocked_approval")
    result = run_importer(
        repo_root,
        importer_base_args(output, artifact_paths, eval_results),
    )
    if result.returncode == 0:
        errors.append("blocked real-results import: importer unexpectedly succeeded")
    elif "require a runnable approval packet" not in result.stderr:
        errors.append(
            "blocked real-results import: stderr did not contain runnable approval refusal; "
            f"stderr was {result.stderr!r}"
        )
    if output.exists():
        errors.append(f"blocked real-results import wrote unexpected file: {output}")
        output.unlink()


def check_mutated_result_intake(
    repo_root: Path,
    ara_root: Path,
    source_output: Path,
    mutated_output: Path,
    mutate: Any,
    expected_error: str,
    errors: list[str],
    label: str,
) -> None:
    rows = load_jsonl(source_output)
    mutated = [dict(row) for row in rows]
    mutate(mutated[0])
    mutated_output.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in mutated),
        encoding="utf-8",
    )
    check_result_intake_for_rows(
        repo_root,
        ara_root,
        mutated_output,
        expected_error,
        errors,
        label,
    )


def model_one_case_artifact_paths(repo_root: Path, tmp_path: Path) -> list[str]:
    base = tmp_path / "model_one_case_seed_41"
    return [
        touch(
            repo_root,
            base / "rendered_prompts/13-1/agent_prompt.md",
            "fixture model one-case prompt\n",
        ),
        touch(
            repo_root,
            base / "prompt_render_manifest.json",
            '{"schema_version":"fixture.prompt_manifest.v1"}\n',
        ),
        touch(
            repo_root,
            base / "outputs/eval_official_results.json",
            '{"schema_version":"fixture.eval.v1"}\n',
        ),
    ]


def model_one_case_row(repo_root: Path, tmp_path: Path) -> dict[str, Any]:
    artifact_paths = [APPROVAL_ARTIFACT, *model_one_case_artifact_paths(repo_root, tmp_path)]
    return {
        "schema_version": "leaven.trace2skill.result.v1",
        "run_id": "trace2skill-model-one-case-fixture",
        "created_at": "2026-06-14T00:00:03Z",
        "proof_classification": "model-one-case",
        "dataset_slice": {
            "name": "SpreadsheetBench-Verified",
            "split": "one-case",
            "case_range": "0..1",
            "case_count": 1,
            "denominator": "one-case-13-1-model-backed",
        },
        "model_id": "Qwen3.5-122B-A10B",
        "serving_backend": "vLLM",
        "seed": 41,
        "skill_source": {"kind": "fixture-model-one-case"},
        "metric_name": "official_instance_accuracy",
        "metric_value": 100.0,
        "metric_unit": "percent",
        "plot_binding": None,
        "cost": {
            "usd": None,
            "prompt_tokens": None,
            "completion_tokens": None,
        },
        "runtime": {
            "seconds": None,
            "workers": 1,
            "max_turns": 100,
        },
        "source_command": (
            "python run_spreadsheetbench.py --model Qwen3.5-122B-A10B --workers 1 "
            "--max_turns 100 --seeds 41 --start_idx 0 --end_idx 1 "
            "&& python evaluate_with_official.py --start_idx 0 --end_idx 1"
        ),
        "artifact_paths": artifact_paths,
        "extra": {
            "runbook_stage_id": "G1M",
            "approval_artifact_paths": [APPROVAL_ARTIFACT],
            "command_policy": "upstream-eval",
            "case_id": "13-1",
        },
        "notes": "",
    }


def check_model_one_case_result_intake(
    repo_root: Path,
    ara_root: Path,
    tmp_path: Path,
    errors: list[str],
) -> None:
    output = tmp_path / "model-one-case.jsonl"
    write_jsonl(output, [model_one_case_row(repo_root, tmp_path)])
    check_result_intake_for_rows(repo_root, ara_root, output, None, errors, "model one-case intake")

    def mutate_case_id(row: dict[str, Any]) -> None:
        row["extra"] = dict(row["extra"])
        row["extra"]["case_id"] = "not-13-1"

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "model-one-case-wrong-case-id.jsonl",
        mutate_case_id,
        "G1M rows must carry extra.case_id '13-1'",
        errors,
        "model one-case case-id drift",
    )


def source_heldout_row(
    repo_root: Path,
    tmp_path: Path,
    seed: int,
    model_id: str = "Qwen3.5-122B-A10B",
    serving_backend: str = "vLLM",
) -> dict[str, Any]:
    prompt = touch(
        repo_root,
        tmp_path / f"heldout_seed_{seed}/rendered_prompts/52807/agent_prompt.md",
        f"fixture held-out seed {seed} rendered agent prompt\n",
    )
    prompt_manifest = touch(
        repo_root,
        tmp_path / f"heldout_seed_{seed}/prompt_render_manifest.json",
        '{"schema_version":"fixture.heldout_prompt_manifest.v1"}\n',
    )
    eval_result = touch(
        repo_root,
        tmp_path / f"heldout_seed_{seed}/outputs/eval_official_results.json",
        '{"schema_version":"fixture.heldout_eval.v1"}\n',
    )
    return {
        "schema_version": "leaven.trace2skill.result.v1",
        "run_id": f"trace2skill-heldout-fixture-seed-{seed}",
        "created_at": "2026-06-14T00:00:03Z",
        "proof_classification": "held-out-single-seed-candidate",
        "dataset_slice": {
            "name": "SpreadsheetBench-Verified",
            "split": "held_out",
            "case_range": "200..400",
            "case_count": 200,
            "denominator": "held-out-200..400",
        },
        "model_id": model_id,
        "serving_backend": serving_backend,
        "seed": seed,
        "skill_source": {"kind": "fixture-heldout"},
        "metric_name": "official_instance_accuracy",
        "metric_value": 50.0,
        "metric_unit": "percent",
        "plot_binding": None,
        "cost": {
            "usd": None,
            "prompt_tokens": None,
            "completion_tokens": None,
        },
        "runtime": {
            "seconds": None,
            "workers": 128,
            "max_turns": 100,
        },
        "source_command": (
            f"python run_spreadsheetbench.py --model {model_id} --workers 128 "
            f"--max_turns 100 --seeds {seed} --start_idx 200 --end_idx 400 "
            "&& python evaluate_with_official.py --start_idx 200 --end_idx 400"
        ),
        "artifact_paths": [APPROVAL_ARTIFACT, prompt, prompt_manifest, eval_result],
        "extra": {
            "runbook_stage_id": "G4",
            "approval_artifact_paths": [APPROVAL_ARTIFACT],
            "command_policy": "upstream-eval",
        },
        "notes": "",
    }


def source_training_validation_row(
    repo_root: Path,
    tmp_path: Path,
    seed: int,
    model_id: str = "Qwen3.5-122B-A10B",
    serving_backend: str = "vLLM",
) -> dict[str, Any]:
    base = tmp_path / f"validation_train_seed_{seed}"
    baseline_prompt = touch(
        repo_root,
        base / "baseline_rendered_prompts/13-1/agent_prompt.md",
        f"fixture validation seed {seed} baseline prompt\n",
    )
    baseline_manifest = touch(
        repo_root,
        base / "baseline_prompt_render_manifest.json",
        '{"schema_version":"fixture.validation_baseline_prompt_manifest.v1"}\n',
    )
    baseline_eval = touch(
        repo_root,
        base / "baseline_outputs/eval_official_results.json",
        '{"schema_version":"fixture.validation_baseline_eval.v1"}\n',
    )
    evolved_prompt = touch(
        repo_root,
        base / "evolved_rendered_prompts/13-1/agent_prompt.md",
        f"fixture validation seed {seed} evolved prompt\n",
    )
    evolved_manifest = touch(
        repo_root,
        base / "evolved_prompt_render_manifest.json",
        '{"schema_version":"fixture.validation_evolved_prompt_manifest.v1"}\n',
    )
    evolved_eval = touch(
        repo_root,
        base / "evolved_outputs/eval_official_results.json",
        '{"schema_version":"fixture.validation_evolved_eval.v1"}\n',
    )
    best_seed_note = touch(
        repo_root,
        tmp_path / "best_seed_selection_note.md",
        f"fixture selected seed {seed} from training validation only\n",
    )
    return {
        "schema_version": "leaven.trace2skill.result.v1",
        "run_id": f"trace2skill-training-validation-fixture-seed-{seed}",
        "created_at": "2026-06-14T00:00:03Z",
        "proof_classification": "training-validation-candidate",
        "dataset_slice": {
            "name": "SpreadsheetBench-Verified",
            "split": "evolving",
            "case_range": "0..200",
            "case_count": 200,
            "denominator": "training-validation-0..200",
        },
        "model_id": model_id,
        "serving_backend": serving_backend,
        "seed": seed,
        "skill_source": {"kind": "fixture-training-validation"},
        "metric_name": "official_instance_accuracy",
        "metric_value": 50.0,
        "metric_unit": "percent",
        "plot_binding": None,
        "cost": {
            "usd": None,
            "prompt_tokens": None,
            "completion_tokens": None,
        },
        "runtime": {
            "seconds": None,
            "workers": 128,
            "max_turns": 100,
        },
        "source_command": (
            f"python run_spreadsheetbench.py --model {model_id} --workers 128 "
            f"--max_turns 100 --seeds {seed} --start_idx 0 --end_idx 200 "
            "&& python evaluate_with_official.py --start_idx 0 --end_idx 200"
        ),
        "artifact_paths": [
            APPROVAL_ARTIFACT,
            baseline_prompt,
            baseline_manifest,
            baseline_eval,
            evolved_prompt,
            evolved_manifest,
            evolved_eval,
            best_seed_note,
        ],
        "extra": {
            "runbook_stage_id": "G3V",
            "approval_artifact_paths": [APPROVAL_ARTIFACT],
            "command_policy": "upstream-eval",
        },
        "notes": "",
    }


def source_evolving_row(
    repo_root: Path,
    tmp_path: Path,
    seed: int,
    model_id: str = "Qwen3.5-122B-A10B",
    serving_backend: str = "vLLM",
) -> dict[str, Any]:
    base = tmp_path / f"evolving_seed_{seed}"
    prompt = touch(
        repo_root,
        base / f"baseline_seed_{seed}/rendered_prompts/52807/agent_prompt.md",
        f"fixture evolving seed {seed} rendered prompt\n",
    )
    prompt_manifest = touch(
        repo_root,
        base / f"baseline_seed_{seed}/prompt_render_manifest.json",
        '{"schema_version":"fixture.evolving_prompt_manifest.v1"}\n',
    )
    eval_result = touch(
        repo_root,
        base / f"baseline_seed_{seed}/outputs/eval_official_results.json",
        '{"schema_version":"fixture.evolving_eval.v1"}\n',
    )
    error_analysis = touch(
        repo_root,
        base / f"baseline_seed_{seed}/error_analysis_parsed.json",
        '{"schema_version":"fixture.error_analysis.v1"}\n',
    )
    error_prompt = touch(
        repo_root,
        base / f"baseline_seed_{seed}/stage2_analyst_prompts/52807/error_prompt.md",
        "fixture error analyst prompt\n",
    )
    success_prompt = touch(
        repo_root,
        base / f"baseline_seed_{seed}/stage2_analyst_prompts/52807/success_prompt.md",
        "fixture success analyst prompt\n",
    )
    fanout = touch(
        repo_root,
        base / f"baseline_seed_{seed}/stage2_fanout.jsonl",
        '{"schema_version":"fixture.stage2_fanout.v1"}\n',
    )
    success_analysis = touch(
        repo_root,
        base / f"baseline_seed_{seed}/success_analysis_parsed.json",
        '{"schema_version":"fixture.success_analysis.v1"}\n',
    )
    change_log = touch(
        repo_root,
        base / f"skill_evolution_seed_{seed}/error_driven_skill_evolution/change.log",
        "fixture skill evolution change log\n",
    )
    merge_prompt = touch(
        repo_root,
        base / f"skill_evolution_seed_{seed}/error_driven_skill_evolution/stage3_merge_prompts/batch-000.md",
        "fixture merge prompt\n",
    )
    merge_manifest = touch(
        repo_root,
        base / f"skill_evolution_seed_{seed}/error_driven_skill_evolution/stage3_merge_manifest.json",
        '{"schema_version":"fixture.stage3_merge_manifest.v1"}\n',
    )
    skill = touch(
        repo_root,
        base / f"skill_evolution_seed_{seed}/error_driven_skill_evolution/skills/xlsx/SKILL.md",
        "# Fixture evolved skill\n",
    )
    return {
        "schema_version": "leaven.trace2skill.result.v1",
        "run_id": f"trace2skill-evolving-fixture-seed-{seed}",
        "created_at": "2026-06-14T00:00:03Z",
        "proof_classification": "evolving-split-run",
        "dataset_slice": {
            "name": "SpreadsheetBench-Verified",
            "split": "evolving",
            "case_range": "0..200",
            "case_count": 200,
            "denominator": "evolving-split-0..200",
        },
        "model_id": model_id,
        "serving_backend": serving_backend,
        "seed": seed,
        "skill_source": {"kind": "fixture-evolved-skill", "path": skill},
        "metric_name": "official_instance_accuracy",
        "metric_value": 50.0,
        "metric_unit": "percent",
        "plot_binding": None,
        "cost": {
            "usd": None,
            "prompt_tokens": None,
            "completion_tokens": None,
        },
        "runtime": {
            "seconds": None,
            "workers": 128,
            "max_turns": 100,
        },
        "source_command": (
            f"python run_spreadsheetbench.py --model {model_id} --workers 128 "
            f"--max_turns 100 --seeds {seed} --start_idx 0 --end_idx 200 "
            "&& python evaluate_with_official.py --start_idx 0 --end_idx 200 "
            "&& python analyze_results.py "
            f"&& python analysis/run_error_analysis.py --model {model_id} --workers 128 --max_turns 100 "
            f"&& python analysis/run_success_analysis_llm.py --model {model_id} --max_workers 128 "
            f"&& python -m skill_evolver.run_parallel_skill_evolution --model {model_id} "
            f"--merge-batch-size 32 --max-workers 128 --seed {seed}"
        ),
        "artifact_paths": [
            APPROVAL_ARTIFACT,
            prompt,
            prompt_manifest,
            eval_result,
            error_analysis,
            error_prompt,
            success_prompt,
            fanout,
            success_analysis,
            change_log,
            merge_prompt,
            merge_manifest,
            skill,
        ],
        "extra": {
            "runbook_stage_id": "G3",
            "approval_artifact_paths": [APPROVAL_ARTIFACT],
            "command_policy": "skill-evolution",
            "source_metric": "instance_accuracy",
            "merge_batch_size": 32,
        },
        "notes": "",
    }


def check_evolving_result_intake(repo_root: Path, ara_root: Path, tmp_path: Path, errors: list[str]) -> None:
    output = tmp_path / "evolving.jsonl"
    write_jsonl(output, [source_evolving_row(repo_root, tmp_path, 41)])
    check_result_intake_for_rows(repo_root, ara_root, output, None, errors, "evolving intake")

    def mutate_evolving_to_wrong_source_merge_batch(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace("--merge-batch-size 32", "--merge-batch-size 5")

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-wrong-source-merge-batch.jsonl",
        mutate_evolving_to_wrong_source_merge_batch,
        "evolving-split-run source_command must include --merge-batch-size 32",
        errors,
        "evolving source-command merge-batch drift",
    )

    def mutate_evolving_to_missing_error_analysis(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace(
            "&& python analysis/run_error_analysis.py --model Qwen3.5-122B-A10B --workers 128 --max_turns 100 ",
            "",
        )

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-missing-error-analysis.jsonl",
        mutate_evolving_to_missing_error_analysis,
        "evolving-split-run source_command must include analysis/run_error_analysis.py",
        errors,
        "evolving missing error-analysis command",
    )

    def mutate_evolving_to_wrong_error_analysis_model(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace(
            "analysis/run_error_analysis.py --model Qwen3.5-122B-A10B",
            "analysis/run_error_analysis.py --model Qwen3.5-35B-A3B",
        )

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-wrong-error-analysis-model.jsonl",
        mutate_evolving_to_wrong_error_analysis_model,
        "evolving-split-run run_error_analysis command must include --model 'Qwen3.5-122B-A10B'",
        errors,
        "evolving error-analysis model drift",
    )

    def mutate_evolving_to_wrong_error_analysis_workers(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace(
            "analysis/run_error_analysis.py --model Qwen3.5-122B-A10B --workers 128",
            "analysis/run_error_analysis.py --model Qwen3.5-122B-A10B --workers 1",
        )

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-wrong-error-analysis-workers.jsonl",
        mutate_evolving_to_wrong_error_analysis_workers,
        "evolving-split-run run_error_analysis command must include --workers 128",
        errors,
        "evolving error-analysis worker drift",
    )

    def mutate_evolving_to_wrong_error_analysis_max_turns(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace(
            "analysis/run_error_analysis.py --model Qwen3.5-122B-A10B --workers 128 --max_turns 100",
            "analysis/run_error_analysis.py --model Qwen3.5-122B-A10B --workers 128 --max_turns 99",
        )

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-wrong-error-analysis-max-turns.jsonl",
        mutate_evolving_to_wrong_error_analysis_max_turns,
        "evolving-split-run run_error_analysis command must include --max_turns 100",
        errors,
        "evolving error-analysis max-turn drift",
    )

    def mutate_evolving_to_missing_success_analysis_model(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace(
            "analysis/run_success_analysis_llm.py --model Qwen3.5-122B-A10B --max_workers 128",
            "analysis/run_success_analysis_llm.py --max_workers 128",
        )

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-missing-success-analysis-model.jsonl",
        mutate_evolving_to_missing_success_analysis_model,
        "evolving-split-run run_success_analysis_llm command must include --model 'Qwen3.5-122B-A10B'",
        errors,
        "evolving success-analysis model drift",
    )

    def mutate_evolving_to_wrong_source_success_workers(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace("--max_workers 128", "--max_workers 1")

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-wrong-source-success-workers.jsonl",
        mutate_evolving_to_wrong_source_success_workers,
        "evolving-split-run source_command must include --max_workers 128",
        errors,
        "evolving source-command success-worker drift",
    )

    def mutate_evolving_to_wrong_source_max_workers(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace("--max-workers 128", "--max-workers 1")

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-wrong-source-max-workers.jsonl",
        mutate_evolving_to_wrong_source_max_workers,
        "evolving-split-run source_command must include --max-workers 128",
        errors,
        "evolving source-command max-workers drift",
    )

    def mutate_evolving_to_missing_skill_evolver_model(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace(
            "skill_evolver.run_parallel_skill_evolution --model Qwen3.5-122B-A10B ",
            "skill_evolver.run_parallel_skill_evolution ",
        )

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-missing-skill-evolver-model.jsonl",
        mutate_evolving_to_missing_skill_evolver_model,
        "evolving-split-run skill_evolver command must include --model 'Qwen3.5-122B-A10B'",
        errors,
        "evolving skill-evolver model drift",
    )

    def mutate_evolving_to_wrong_skill_evolver_seed(row: dict[str, Any]) -> None:
        row["source_command"] = row["source_command"].replace("--seed 41", "--seed 99")

    check_mutated_result_intake(
        repo_root,
        ara_root,
        output,
        tmp_path / "evolving-wrong-skill-evolver-seed.jsonl",
        mutate_evolving_to_wrong_skill_evolver_seed,
        "evolving-split-run skill_evolver command must include --seed 41",
        errors,
        "evolving skill-evolver seed drift",
    )


def check_aggregate_result_intake(repo_root: Path, ara_root: Path, tmp_path: Path, errors: list[str]) -> None:
    source_paths: list[str] = []
    for seed in (41, 42, 43):
        source_path = tmp_path / f"heldout_seed_{seed}.jsonl"
        write_jsonl(source_path, [source_heldout_row(repo_root, tmp_path, seed)])
        source_paths.append(source_path.relative_to(repo_root).as_posix())

    prompt_manifest = repo_root / "docs/ara/trace2skill_spreadsheetbench/results/fixture_aggregate.prompt_render_manifest.json"
    prompt_manifest.write_text('{"schema_version":"fixture.aggregate_prompt_manifest.v1"}\n', encoding="utf-8")
    prompt_manifest_rel = prompt_manifest.relative_to(repo_root).as_posix()
    blocked_source_outputs: list[Path] = []
    aggregate_output = tmp_path / "aggregate.jsonl"
    aggregate_row = {
        "schema_version": "leaven.trace2skill.result.v1",
        "run_id": "trace2skill-aggregate-fixture",
        "created_at": "2026-06-14T00:00:03Z",
        "proof_classification": "seed-aggregate-candidate",
        "dataset_slice": {
            "name": "SpreadsheetBench-Verified",
            "split": "held_out",
            "case_range": "200..400",
            "case_count": 200,
            "denominator": "seed-aggregate-41-42-43",
        },
        "model_id": "Qwen3.5-122B-A10B",
        "serving_backend": "vLLM",
        "seed": None,
        "skill_source": {"kind": "fixture-aggregate"},
        "metric_name": "official_instance_accuracy",
        "metric_value": 50.0,
        "metric_unit": "percent",
        "plot_binding": None,
        "cost": {
            "usd": None,
            "prompt_tokens": None,
            "completion_tokens": None,
        },
        "runtime": {
            "seconds": None,
        },
        "source_command": "aggregate heldout_seed_41.jsonl heldout_seed_42.jsonl heldout_seed_43.jsonl",
        "artifact_paths": [APPROVAL_ARTIFACT, prompt_manifest_rel, *source_paths],
        "extra": {
            "runbook_stage_id": "G5",
            "approval_artifact_paths": [APPROVAL_ARTIFACT],
            "seeds": [41, 42, 43],
            "source_result_paths": source_paths,
        },
        "notes": "",
    }
    write_jsonl(aggregate_output, [aggregate_row])
    try:
        check_result_intake_for_rows(repo_root, ara_root, aggregate_output, None, errors, "aggregate intake")

        unsupported_source_path = tmp_path / "aggregate_training_validation_source.jsonl"
        write_jsonl(unsupported_source_path, [source_training_validation_row(repo_root, tmp_path, 41)])
        unsupported_source_rel = unsupported_source_path.relative_to(repo_root).as_posix()
        unsupported_source_aggregate_row = json.loads(json.dumps(aggregate_row))
        unsupported_source_aggregate_row["artifact_paths"] = [
            APPROVAL_ARTIFACT,
            prompt_manifest_rel,
            *source_paths,
            unsupported_source_rel,
        ]
        unsupported_source_aggregate_row["extra"]["source_result_paths"] = [
            *source_paths,
            unsupported_source_rel,
        ]
        unsupported_source_aggregate_output = tmp_path / "aggregate-unsupported-source-classification.jsonl"
        write_jsonl(unsupported_source_aggregate_output, [unsupported_source_aggregate_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            unsupported_source_aggregate_output,
            "must use proof_classification 'held-out-single-seed-candidate' from runbook stage 'G4'",
            errors,
            "aggregate unsupported source classification",
        )

        invalid_heldout_model_output = tmp_path / "heldout-invalid-model.jsonl"
        write_jsonl(
            invalid_heldout_model_output,
            [source_heldout_row(repo_root, tmp_path, 41, model_id="fixture-model")],
        )
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            invalid_heldout_model_output,
            "held-out-single-seed-candidate rows must use a paper model_id",
            errors,
            "held-out invalid model id",
        )

        def mutate_aggregate_to_missing_source_seed(row: dict[str, Any]) -> None:
            row["extra"] = dict(row["extra"])
            row["extra"]["source_result_paths"] = list(row["extra"]["source_result_paths"][:-1])

        check_mutated_result_intake(
            repo_root,
            ara_root,
            aggregate_output,
            tmp_path / "aggregate-missing-source-seed.jsonl",
            mutate_aggregate_to_missing_source_seed,
            "G5 aggregate rows must cite at least 3 source result path(s)",
            errors,
            "aggregate source-result drift",
        )

        def mutate_aggregate_to_wrong_case_count(row: dict[str, Any]) -> None:
            row["dataset_slice"] = dict(row["dataset_slice"])
            row["dataset_slice"]["case_count"] = 199

        check_mutated_result_intake(
            repo_root,
            ara_root,
            aggregate_output,
            tmp_path / "aggregate-wrong-case-count.jsonl",
            mutate_aggregate_to_wrong_case_count,
            "G5 rows must use dataset_slice.case_count 200",
            errors,
            "aggregate case-count drift",
        )

        def mutate_aggregate_to_wrong_metric_value(row: dict[str, Any]) -> None:
            row["metric_value"] = 51.0

        check_mutated_result_intake(
            repo_root,
            ara_root,
            aggregate_output,
            tmp_path / "aggregate-wrong-metric-value.jsonl",
            mutate_aggregate_to_wrong_metric_value,
            "G5 aggregate metric_value must equal mean source metric_value 50.0",
            errors,
            "aggregate metric-value drift",
        )

        def mutate_aggregate_to_wrong_serving_backend(row: dict[str, Any]) -> None:
            row["serving_backend"] = "fixture-backend"

        check_mutated_result_intake(
            repo_root,
            ara_root,
            aggregate_output,
            tmp_path / "aggregate-wrong-serving.jsonl",
            mutate_aggregate_to_wrong_serving_backend,
            "seed-aggregate-candidate rows must use vLLM",
            errors,
            "aggregate serving drift",
        )

        invalid_source_paths = list(source_paths)
        invalid_source_path = tmp_path / "heldout_seed_41_invalid.jsonl"
        invalid_source_row = source_heldout_row(repo_root, tmp_path, 41)
        invalid_source_row["runtime"] = dict(invalid_source_row["runtime"])
        invalid_source_row["runtime"]["workers"] = 1
        write_jsonl(invalid_source_path, [invalid_source_row])
        invalid_source_paths[0] = invalid_source_path.relative_to(repo_root).as_posix()
        invalid_aggregate_row = json.loads(json.dumps(aggregate_row))
        invalid_aggregate_row["artifact_paths"] = [APPROVAL_ARTIFACT, prompt_manifest_rel, *invalid_source_paths]
        invalid_aggregate_row["extra"]["source_result_paths"] = invalid_source_paths
        invalid_aggregate_output = tmp_path / "aggregate-invalid-source-row.jsonl"
        write_jsonl(invalid_aggregate_output, [invalid_aggregate_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            invalid_aggregate_output,
            "does not pass result intake",
            errors,
            "aggregate invalid source row",
        )

        mismatched_model_paths = list(source_paths)
        mismatched_model_source_path = tmp_path / "heldout_seed_41_wrong_model.jsonl"
        write_jsonl(
            mismatched_model_source_path,
            [source_heldout_row(repo_root, tmp_path, 41, model_id="fixture-other-model")],
        )
        mismatched_model_paths[0] = mismatched_model_source_path.relative_to(repo_root).as_posix()
        mismatched_model_aggregate_row = json.loads(json.dumps(aggregate_row))
        mismatched_model_aggregate_row["artifact_paths"] = [
            APPROVAL_ARTIFACT,
            prompt_manifest_rel,
            *mismatched_model_paths,
        ]
        mismatched_model_aggregate_row["extra"]["source_result_paths"] = mismatched_model_paths
        mismatched_model_aggregate_output = tmp_path / "aggregate-mismatched-source-model.jsonl"
        write_jsonl(mismatched_model_aggregate_output, [mismatched_model_aggregate_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            mismatched_model_aggregate_output,
            "G5 source row",
            errors,
            "aggregate source model drift",
        )

        blocked_source_paths: list[str] = []
        for seed in (41, 42, 43):
            blocked_source_output = ara_root / "results" / f"fixture_blocked_source_seed_{seed}.jsonl"
            write_jsonl(blocked_source_output, [source_heldout_row(repo_root, tmp_path, seed)])
            blocked_source_outputs.append(blocked_source_output)
            blocked_source_paths.append(blocked_source_output.relative_to(repo_root).as_posix())
        blocked_source_aggregate_row = json.loads(json.dumps(aggregate_row))
        blocked_source_aggregate_row["artifact_paths"] = [
            APPROVAL_ARTIFACT,
            prompt_manifest_rel,
            *blocked_source_paths,
        ]
        blocked_source_aggregate_row["extra"]["source_result_paths"] = blocked_source_paths
        blocked_source_aggregate_output = tmp_path / "aggregate-blocked-top-level-source.jsonl"
        write_jsonl(blocked_source_aggregate_output, [blocked_source_aggregate_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            blocked_source_aggregate_output,
            "require a runnable approval packet",
            errors,
            "aggregate blocked top-level source row",
        )

        full_paper_output = tmp_path / "full-paper.jsonl"
        aggregate_output_rel = aggregate_output.relative_to(repo_root).as_posix()
        training_validation_output = tmp_path / "training-validation-seed-41.jsonl"
        write_jsonl(training_validation_output, [source_training_validation_row(repo_root, tmp_path, 41)])
        training_validation_output_rel = training_validation_output.relative_to(repo_root).as_posix()
        full_paper_row = {
            "schema_version": "leaven.trace2skill.result.v1",
            "run_id": "trace2skill-full-paper-fixture",
            "created_at": "2026-06-14T00:00:04Z",
            "proof_classification": "paper-denominator-reproduction",
            "dataset_slice": {
                "name": "SpreadsheetBench-Verified",
                "split": "all",
                "case_range": "0..400",
                "case_count": 400,
                "denominator": "full-paper-denominator",
            },
            "model_id": "Qwen3.5-122B-A10B",
            "serving_backend": "vLLM",
            "seed": None,
            "skill_source": {"kind": "fixture-full-paper"},
            "metric_name": "official_instance_accuracy",
            "metric_value": 50.0,
            "metric_unit": "percent",
            "plot_binding": None,
            "cost": {
                "usd": None,
                "prompt_tokens": None,
                "completion_tokens": None,
            },
            "runtime": {
                "seconds": None,
            },
            "source_command": "aggregate seed-aggregate-41-42-43 into full-paper fixture",
            "artifact_paths": [
                APPROVAL_ARTIFACT,
                prompt_manifest_rel,
                training_validation_output_rel,
                aggregate_output_rel,
            ],
            "extra": {
                "runbook_stage_id": "G6",
                "approval_artifact_paths": [APPROVAL_ARTIFACT],
                "seeds": [41, 42, 43],
                "source_result_paths": [training_validation_output_rel, aggregate_output_rel],
            },
            "notes": "",
        }
        write_jsonl(full_paper_output, [full_paper_row])
        check_result_intake_for_rows(repo_root, ara_root, full_paper_output, None, errors, "full-paper intake")

        unsupported_source_output = tmp_path / "full-paper-unsupported-source-classification.jsonl"
        unsupported_source_row = json.loads(json.dumps(full_paper_row))
        unsupported_source_row["artifact_paths"] = [
            APPROVAL_ARTIFACT,
            prompt_manifest_rel,
            training_validation_output_rel,
            aggregate_output_rel,
            source_paths[0],
        ]
        unsupported_source_row["extra"]["source_result_paths"] = [
            training_validation_output_rel,
            aggregate_output_rel,
            source_paths[0],
        ]
        write_jsonl(unsupported_source_output, [unsupported_source_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            unsupported_source_output,
            "proof_classification must be one of ['training-validation-candidate', 'seed-aggregate-candidate']",
            errors,
            "full-paper unsupported source classification",
        )

        missing_training_output = tmp_path / "full-paper-missing-training-source.jsonl"
        missing_training_row = json.loads(json.dumps(full_paper_row))
        missing_training_row["artifact_paths"] = [APPROVAL_ARTIFACT, prompt_manifest_rel, aggregate_output_rel]
        missing_training_row["extra"]["source_result_paths"] = [aggregate_output_rel]
        write_jsonl(missing_training_output, [missing_training_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            missing_training_output,
            "G6 full-paper rows must cite source result rows covering split ranges ['0..200']",
            errors,
            "full-paper missing training source range",
        )

        mismatched_training_output = tmp_path / "training-validation-seed-41-wrong-model.jsonl"
        write_jsonl(
            mismatched_training_output,
            [source_training_validation_row(repo_root, tmp_path, 41, model_id="fixture-other-model")],
        )
        mismatched_training_output_rel = mismatched_training_output.relative_to(repo_root).as_posix()
        mismatched_training_full_paper_row = json.loads(json.dumps(full_paper_row))
        mismatched_training_full_paper_row["artifact_paths"] = [
            APPROVAL_ARTIFACT,
            prompt_manifest_rel,
            mismatched_training_output_rel,
            aggregate_output_rel,
        ]
        mismatched_training_full_paper_row["extra"]["source_result_paths"] = [
            mismatched_training_output_rel,
            aggregate_output_rel,
        ]
        mismatched_training_full_paper_output = tmp_path / "full-paper-mismatched-training-model.jsonl"
        write_jsonl(mismatched_training_full_paper_output, [mismatched_training_full_paper_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            mismatched_training_full_paper_output,
            "G6 source row",
            errors,
            "full-paper source model drift",
        )

        invalid_case_count_output = tmp_path / "full-paper-invalid-case-count.jsonl"
        invalid_case_count_row = json.loads(json.dumps(full_paper_row))
        invalid_case_count_row["dataset_slice"]["case_count"] = 200
        write_jsonl(invalid_case_count_output, [invalid_case_count_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            invalid_case_count_output,
            "G6 rows must use dataset_slice.case_count 400",
            errors,
            "full-paper invalid case count",
        )

        invalid_metric_output = tmp_path / "full-paper-invalid-metric-value.jsonl"
        invalid_metric_row = json.loads(json.dumps(full_paper_row))
        invalid_metric_row["metric_value"] = 51.0
        write_jsonl(invalid_metric_output, [invalid_metric_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            invalid_metric_output,
            "G6 full-paper metric_value must equal weighted mean source metric_value 50.0",
            errors,
            "full-paper metric-value drift",
        )

        invalid_serving_output = tmp_path / "full-paper-invalid-serving.jsonl"
        invalid_serving_row = json.loads(json.dumps(full_paper_row))
        invalid_serving_row["serving_backend"] = "fixture-backend"
        write_jsonl(invalid_serving_output, [invalid_serving_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            invalid_serving_output,
            "paper-denominator-reproduction rows must use vLLM",
            errors,
            "full-paper invalid serving backend",
        )

        invalid_model_output = tmp_path / "full-paper-invalid-model.jsonl"
        invalid_model_row = json.loads(json.dumps(full_paper_row))
        invalid_model_row["model_id"] = "fixture-model"
        write_jsonl(invalid_model_output, [invalid_model_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            invalid_model_output,
            "paper-denominator-reproduction rows must use a paper model_id",
            errors,
            "full-paper invalid model id",
        )

        invalid_aggregate_output_rel = invalid_aggregate_output.relative_to(repo_root).as_posix()
        invalid_full_paper_row = json.loads(json.dumps(full_paper_row))
        invalid_full_paper_row["artifact_paths"] = [APPROVAL_ARTIFACT, prompt_manifest_rel, invalid_aggregate_output_rel]
        invalid_full_paper_row["extra"]["source_result_paths"] = [invalid_aggregate_output_rel]
        invalid_full_paper_output = tmp_path / "full-paper-invalid-aggregate-source.jsonl"
        write_jsonl(invalid_full_paper_output, [invalid_full_paper_row])
        check_result_intake_for_rows(
            repo_root,
            ara_root,
            invalid_full_paper_output,
            "does not pass result intake",
            errors,
            "full-paper invalid aggregate source row",
        )
    finally:
        prompt_manifest.unlink(missing_ok=True)
        for blocked_source_output in blocked_source_outputs:
            blocked_source_output.unlink(missing_ok=True)


def check_importer_fixture(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    target_dir = repo_root / "target"
    target_dir.mkdir(exist_ok=True)
    check_runbook_stage(repo_root, ara_root, errors)

    with tempfile.TemporaryDirectory(prefix="trace2skill-importer-fixture-", dir=target_dir) as tmp:
        tmp_path = Path(tmp)
        output = tmp_path / "imported.jsonl"
        check_positive_import(repo_root, output, errors)
        check_result_intake_for_rows(repo_root, ara_root, output, None, errors, "positive import intake")
        check_model_one_case_result_intake(repo_root, ara_root, tmp_path, errors)
        check_real_results_require_runnable_approval(repo_root, ara_root, tmp_path, errors)
        check_evolving_result_intake(repo_root, ara_root, tmp_path, errors)
        check_aggregate_result_intake(repo_root, ara_root, tmp_path, errors)
        eval_results = eval_results_artifact_path(repo_root, tmp_path)

        def mutate_missing_schema_version(row: dict[str, Any]) -> None:
            row.pop("schema_version", None)

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "missing-schema-version.jsonl",
            mutate_missing_schema_version,
            "schema_version must be 'leaven.trace2skill.result.v1'",
            errors,
            "missing schema version",
        )

        def mutate_metric_value_to_string(row: dict[str, Any]) -> None:
            row["metric_value"] = str(row["metric_value"])

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "string-metric-value.jsonl",
            mutate_metric_value_to_string,
            "metric_value must be numeric",
            errors,
            "string metric value",
        )

        def mutate_official_source_metric_to_wrong_name(row: dict[str, Any]) -> None:
            row["extra"] = dict(row["extra"])
            row["extra"]["source_metric"] = "avg_soft_score"

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "wrong-source-metric-name.jsonl",
            mutate_official_source_metric_to_wrong_name,
            "non-overlay official source_metric 'avg_soft_score' must use metric_name 'official_avg_soft_score'",
            errors,
            "official source-metric name drift",
        )

        def mutate_official_source_metric_to_derived_overlay(row: dict[str, Any]) -> None:
            row["metric_unit"] = "delta_points"
            row["plot_binding"] = {
                "panel": "avg_improvement",
                "x_label": "122B\nDeep +Combined",
                "series": "Fixture",
                "axis": "left",
            }

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "source-metric-derived-overlay.jsonl",
            mutate_official_source_metric_to_derived_overlay,
            "official source_metric 'instance_accuracy' cannot bind to plot panel 'avg_improvement'/axis 'left'",
            errors,
            "official source-metric derived overlay drift",
        )

        def mutate_overlay_to_unknown_target_label(row: dict[str, Any]) -> None:
            row["plot_binding"] = {
                "panel": "same_model_deepening_vrf",
                "x_label": "Not A Paper Target",
                "series": "Fixture",
                "axis": "left",
            }

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "unknown-target-label-overlay.jsonl",
            mutate_overlay_to_unknown_target_label,
            "plot_binding.x_label 'Not A Paper Target' does not match target labels for panel 'same_model_deepening_vrf'",
            errors,
            "overlay target-label drift",
        )

        def mutate_overlay_to_wrong_model_family(row: dict[str, Any]) -> None:
            row["model_id"] = "Qwen3.5-35B-A3B"
            row["plot_binding"] = {
                "panel": "same_model_deepening_vrf",
                "x_label": "+Combined\n122B",
                "series": "Fixture",
                "axis": "left",
            }

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "wrong-model-family-overlay.jsonl",
            mutate_overlay_to_wrong_model_family,
            "plot_binding.x_label '+Combined\\n122B' requires 122B model family, got model_id 'Qwen3.5-35B-A3B'",
            errors,
            "overlay model-family drift",
        )

        def mutate_parallel_overlay_to_wrong_model_series(row: dict[str, Any]) -> None:
            row["model_id"] = "Qwen3.5-35B-A3B"
            row["plot_binding"] = {
                "panel": "parallel_vs_sequential",
                "x_label": "Parallel (ours)",
                "series": "Leaven 122B Vrf",
                "axis": "left",
            }

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "wrong-parallel-series-family-overlay.jsonl",
            mutate_parallel_overlay_to_wrong_model_series,
            "plot_binding.series 'Leaven 122B Vrf' requires 122B model family, got model_id 'Qwen3.5-35B-A3B'",
            errors,
            "parallel overlay series-family drift",
        )

        def mutate_missing_eval_artifact(row: dict[str, Any]) -> None:
            row["artifact_paths"] = [
                path
                for path in row["artifact_paths"]
                if "outputs/eval_official_results.json" not in path
            ]

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "missing-eval-artifact.jsonl",
            mutate_missing_eval_artifact,
            "missing artifact matching runbook expectation",
            errors,
            "missing official eval artifact",
        )

        def mutate_missing_skill_artifact(row: dict[str, Any]) -> None:
            skill_path = row["skill_source"]["path"]
            row["artifact_paths"] = [
                path
                for path in row["artifact_paths"]
                if path != skill_path
            ]

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "missing-skill-artifact.jsonl",
            mutate_missing_skill_artifact,
            "skill_source.path",
            errors,
            "missing skill artifact in audit",
        )

        def mutate_subset_to_paper_sized(row: dict[str, Any]) -> None:
            row["dataset_slice"] = dict(row["dataset_slice"])
            row["dataset_slice"]["case_count"] = 200
            row["dataset_slice"]["denominator"] = "full-paper-denominator"

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-drift.jsonl",
            mutate_subset_to_paper_sized,
            "G2 paper-subset rows must stay below the 200-case paper denominator",
            errors,
            "subset denominator drift",
        )

        def mutate_subset_to_wrong_split(row: dict[str, Any]) -> None:
            row["dataset_slice"] = dict(row["dataset_slice"])
            row["dataset_slice"]["split"] = "evolving"

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-split.jsonl",
            mutate_subset_to_wrong_split,
            "G2 rows must use dataset_slice.split 'held_out'",
            errors,
            "subset split drift",
        )

        def mutate_subset_to_wrong_model(row: dict[str, Any]) -> None:
            row["model_id"] = "fixture-model"

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-model.jsonl",
            mutate_subset_to_wrong_model,
            "paper-subset rows must use a paper model_id",
            errors,
            "subset model identity drift",
        )

        def mutate_subset_to_wrong_backend(row: dict[str, Any]) -> None:
            row["serving_backend"] = "fixture-backend"

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-backend.jsonl",
            mutate_subset_to_wrong_backend,
            "paper-subset rows must use vLLM",
            errors,
            "subset serving identity drift",
        )

        def mutate_heldout_to_training_range(row: dict[str, Any]) -> None:
            row["proof_classification"] = "held-out-single-seed-candidate"
            row["dataset_slice"] = dict(row["dataset_slice"])
            row["dataset_slice"]["case_range"] = "0..200"
            row["dataset_slice"]["case_count"] = 200
            row["dataset_slice"]["denominator"] = "held-out-200..400"
            row["extra"] = dict(row["extra"])
            row["extra"]["runbook_stage_id"] = "G4"

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "heldout-range-drift.jsonl",
            mutate_heldout_to_training_range,
            "G4 rows must use dataset_slice.case_range '200..400'",
            errors,
            "held-out range drift",
        )

        def mutate_subset_to_wrong_seed(row: dict[str, Any]) -> None:
            row["seed"] = 99

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-seed.jsonl",
            mutate_subset_to_wrong_seed,
            "G2 rows must use seed 41",
            errors,
            "subset seed drift",
        )

        def mutate_subset_to_wrong_workers(row: dict[str, Any]) -> None:
            row["runtime"] = dict(row["runtime"])
            row["runtime"]["workers"] = 1

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-workers.jsonl",
            mutate_subset_to_wrong_workers,
            "G2 rows must use runtime.workers 128",
            errors,
            "subset worker drift",
        )

        def mutate_subset_to_wrong_command_policy(row: dict[str, Any]) -> None:
            row["extra"] = dict(row["extra"])
            row["extra"]["command_policy"] = "fixture-smoke"

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-command-policy.jsonl",
            mutate_subset_to_wrong_command_policy,
            "G2 rows must use extra.command_policy 'upstream-eval'",
            errors,
            "subset command-policy drift",
        )

        def mutate_subset_to_wrong_source_range(row: dict[str, Any]) -> None:
            row["source_command"] = (
                "python run_spreadsheetbench.py --start_idx 0 --end_idx 2 "
                "&& python evaluate_with_official.py --start_idx 0 --end_idx 2"
            )

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-source-range.jsonl",
            mutate_subset_to_wrong_source_range,
            "G2 source_command must include dataset range fragment '--start_idx 200'",
            errors,
            "subset source-command range drift",
        )

        def mutate_subset_to_wrong_source_model(row: dict[str, Any]) -> None:
            row["source_command"] = row["source_command"].replace(
                "--model Qwen3.5-122B-A10B",
                "--model Qwen3.5-35B-A3B # claimed model_id Qwen3.5-122B-A10B",
            )

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-source-model.jsonl",
            mutate_subset_to_wrong_source_model,
            "paper-subset source_command must include --model 'Qwen3.5-122B-A10B'",
            errors,
            "subset source-command model drift",
        )

        def mutate_subset_to_wrong_source_seed(row: dict[str, Any]) -> None:
            row["source_command"] = row["source_command"].replace("--seeds 41", "--seeds 42")

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-source-seed.jsonl",
            mutate_subset_to_wrong_source_seed,
            "paper-subset source_command must include --seeds 41",
            errors,
            "subset source-command seed drift",
        )

        def mutate_subset_to_wrong_source_workers(row: dict[str, Any]) -> None:
            row["source_command"] = row["source_command"].replace("--workers 128", "--workers 1")

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-source-workers.jsonl",
            mutate_subset_to_wrong_source_workers,
            "paper-subset source_command must include --workers 128",
            errors,
            "subset source-command worker drift",
        )

        def mutate_subset_to_wrong_source_max_turns(row: dict[str, Any]) -> None:
            row["source_command"] = row["source_command"].replace("--max_turns 100", "--max_turns 99")

        check_mutated_result_intake(
            repo_root,
            ara_root,
            output,
            tmp_path / "subset-wrong-source-max-turns.jsonl",
            mutate_subset_to_wrong_source_max_turns,
            "paper-subset source_command must include --max_turns 100",
            errors,
            "subset source-command max-turns drift",
        )

        wrong_range_args = importer_base_args(
            tmp_path / "wrong-case-range.jsonl",
            prompt_artifact_paths(repo_root, tmp_path),
            eval_results,
        )
        case_range_index = wrong_range_args.index("--case-range") + 1
        wrong_range_args[case_range_index] = "201..203"
        expect_failure(
            repo_root,
            wrong_range_args,
            "eval result ids do not match declared dataset case range",
            errors,
            "wrong eval case range",
        )

        def remove_first_result_id(payload: dict[str, Any]) -> None:
            payload["results"][0].pop("id", None)

        missing_id_eval_results = mutated_eval_results_artifact_path(
            repo_root,
            tmp_path / "missing_result_id",
            remove_first_result_id,
        )
        expect_failure(
            repo_root,
            importer_base_args(
                tmp_path / "missing-result-id.jsonl",
                prompt_artifact_paths(repo_root, tmp_path / "missing_result_id"),
                missing_id_eval_results,
            ),
            "results[1].id must be a non-empty string",
            errors,
            "missing eval result id",
        )

        expect_failure(
            repo_root,
            importer_base_args(tmp_path / "missing-prompt.jsonl", eval_results=eval_results),
            "missing prompt artifact matching runbook expectation",
            errors,
            "missing prompt artifact",
        )

        wrong_stage_args = importer_base_args(
            tmp_path / "wrong-stage.jsonl",
            prompt_artifact_paths(repo_root, tmp_path),
            eval_results,
        )
        stage_id_index = wrong_stage_args.index("--runbook-stage-id") + 1
        wrong_stage_args[stage_id_index] = "G1"
        expect_failure(
            repo_root,
            wrong_stage_args,
            "does not match runbook stage G1 allowed_label 'deterministic-one-case'",
            errors,
            "wrong runbook stage label",
        )

        missing_stage_args = importer_base_args(tmp_path / "missing-stage.jsonl", eval_results=eval_results)
        stage_flag_index = missing_stage_args.index("--runbook-stage-id")
        del missing_stage_args[stage_flag_index : stage_flag_index + 2]
        expect_failure(
            repo_root,
            missing_stage_args,
            "--runbook-stage-id",
            errors,
            "missing runbook stage",
        )

        missing_approval_args = importer_base_args(tmp_path / "missing-approval.jsonl", eval_results=eval_results)
        approval_flag_index = missing_approval_args.index("--approval-artifact-path")
        del missing_approval_args[approval_flag_index : approval_flag_index + 2]
        expect_failure(
            repo_root,
            missing_approval_args,
            "paper-subset requires at least one --approval-artifact-path",
            errors,
            "missing approval artifact",
        )

        missing_skill_args = importer_base_args(
            tmp_path / "missing-skill.jsonl",
            prompt_artifact_paths(repo_root, tmp_path),
            eval_results,
            "target/trace2skill-importer-fixture-missing/SKILL.md",
        )
        expect_failure(
            repo_root,
            missing_skill_args,
            "--skill-path is not inspectable",
            errors,
            "missing skill artifact",
        )

        paper_denominator_args = importer_base_args(tmp_path / "paper-denominator.jsonl", eval_results=eval_results)
        proof_index = paper_denominator_args.index("--proof-classification") + 1
        paper_denominator_args[proof_index] = "paper-denominator-reproduction"
        stage_id_index = paper_denominator_args.index("--runbook-stage-id") + 1
        paper_denominator_args[stage_id_index] = "G9"
        denominator_index = paper_denominator_args.index("--denominator") + 1
        paper_denominator_args[denominator_index] = "SpreadsheetBench-Verified-200-case-paper-denominator"
        case_count_index = paper_denominator_args.index("--case-count") + 1
        paper_denominator_args[case_count_index] = "200"
        expect_failure(
            repo_root,
            paper_denominator_args,
            "refusing paper-denominator-reproduction without --allow-paper-denominator-reproduction",
            errors,
            "missing paper-denominator allow flag",
        )

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
    errors = check_importer_fixture(repo_root.resolve(), ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} official-eval importer fixture")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
