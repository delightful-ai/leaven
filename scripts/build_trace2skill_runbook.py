#!/usr/bin/env python3
"""Build the Trace2Skill paper-denominator runbook from ARA state."""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path
from typing import Any


def import_approval_checker(repo_root: Path) -> Any:
    path = repo_root / "scripts/check_trace2skill_approval_packet.py"
    spec = importlib.util.spec_from_file_location("check_trace2skill_approval_packet", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def shell_block(lines: list[str]) -> str:
    return "```bash\n" + "\n".join(lines) + "\n```"


def stage(
    stage_id: str,
    title: str,
    denominator: str,
    runnable_now: bool,
    approval_required: bool,
    commands: list[str],
    expected_artifacts: list[str],
    allowed_label: str,
    forbidden_label: str,
    expected_dataset_slice: dict[str, Any] | None = None,
    expected_seed_policy: dict[str, Any] | None = None,
    expected_runtime_policy: dict[str, Any] | None = None,
    expected_command_policy: dict[str, Any] | None = None,
    expected_aggregate_policy: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "id": stage_id,
        "title": title,
        "denominator": denominator,
        "runnable_now": runnable_now,
        "approval_required": approval_required,
        "commands": commands,
        "expected_artifacts": expected_artifacts,
        "allowed_label": allowed_label,
        "forbidden_label": forbidden_label,
        "expected_dataset_slice": expected_dataset_slice,
        "expected_seed_policy": expected_seed_policy,
        "expected_runtime_policy": expected_runtime_policy,
        "expected_command_policy": expected_command_policy,
        "expected_aggregate_policy": expected_aggregate_policy,
    }


def build_runbook(repo_root: Path, ara_dir: Path) -> dict[str, Any]:
    approval = import_approval_checker(repo_root)
    packet = approval.approval_packet((ara_dir / "results/full_run_plan.md").read_text(encoding="utf-8"))
    approval_errors = approval.packet_errors(packet, ara_dir.resolve())
    manifest = load_json(ara_dir / "results/dataset_manifest.json")
    splits = {item["name"]: item for item in manifest["splits"]}
    evolving = splits["evolving"]
    held_out = splits["held_out"]
    data_path = packet["dataset"]["path"]
    seeds = " ".join(str(seed) for seed in packet["protocol"]["seeds"])
    workers = packet["protocol"]["stage2_workers"]
    merge_batch_size = packet["protocol"]["merge_batch_size"]
    turn_budget = packet["protocol"]["react_turn_budget"]

    common_env = [
        "cd tmp/repros/trace2skill-upstream",
        f"DATA_PATH={data_path}",
        "MODEL=${MODEL:?set approved served model id}",
        f"WORKERS={workers}",
        f"MERGE_BATCH_SIZE={merge_batch_size}",
        f"MAX_TURNS={turn_budget}",
        f"SEEDS=({seeds})",
        "GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_instruct_reasoning.json",
        "THINK_GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_thinking_reasoning.json",
        "RUN_ROOT=${RUN_ROOT:?set approved artifact root}",
    ]

    stages = [
        stage(
            "G0",
            "No-spend guardrails",
            "ara-and-approval-preflight",
            True,
            False,
            [
                "uv run python scripts/build_trace2skill_dataset_manifest.py",
                "uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench --expect-blocked",
                "uv run --with pyyaml python scripts/audit_trace2skill_closeout.py docs/ara/trace2skill_spreadsheetbench",
                "uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench",
            ],
            [
                "docs/ara/trace2skill_spreadsheetbench/results/dataset_manifest.json",
                "docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.json",
                "docs/ara/trace2skill_spreadsheetbench/validation.md",
            ],
            "guardrail-ready",
            "paper reproduction",
            None,
            None,
            None,
            None,
        ),
        stage(
            "G1",
            "Deterministic one-case Leaven seam proof",
            "one-case-13-1-deterministic",
            True,
            False,
            [
                "cargo run -p trace2skill_spreadsheetbench -- --prepare-one-case-run --run-dir tmp/trace2skill-one-case-live",
                "cargo run -p trace2skill_spreadsheetbench -- --run-one-case-acp-worker --run-dir tmp/trace2skill-one-case-live --model-id local-openpyxl-trace2skill-agent",
                "cargo run -p trace2skill_spreadsheetbench -- --prepare-one-case-analyst-fanout --run-dir tmp/trace2skill-one-case-live",
            ],
            [
                "tmp/trace2skill-one-case-live/manifest.json",
                "tmp/trace2skill-one-case-live/13-1_output.xlsx",
                "tmp/trace2skill-one-case-live/acp_result.json",
                "tmp/trace2skill-one-case-live/agent_transcript.md",
                "tmp/trace2skill-one-case-live/score_report.json",
                "tmp/trace2skill-one-case-live/trajectory.json",
                "tmp/trace2skill-one-case-live/stage2_analyst_prompt.md",
                "tmp/trace2skill-one-case-live/stage2_fanout.json",
            ],
            "deterministic-one-case",
            "paper reproduction",
            {
                "kind": "one-case",
                "split": "one-case",
                "case_id": "13-1",
                "case_range": "0..1",
                "case_count": 1,
                "denominator_contains": "one-case-13-1",
            },
            None,
            None,
            None,
        ),
        stage(
            "G1M",
            "Model-backed one-case upstream gate",
            "one-case-13-1-model-backed",
            False,
            True,
            common_env
            + [
                "SEED=41",
                "ONE_CASE_DIR=\"$RUN_ROOT/model_one_case_seed_${SEED}\"",
                "python run_spreadsheetbench.py --data_path \"$DATA_PATH\" --model \"$MODEL\" --agent cli_skill_preloaded --log_dir \"$ONE_CASE_DIR/logs\" --log_format markdown --working_dir \"$ONE_CASE_DIR/work\" --output_dir \"$ONE_CASE_DIR/outputs\" --max_turns \"$MAX_TURNS\" --workers 1 --skills_dir spreadsheet_agent/skills --seeds \"$SEED\" --generation_config \"$GENERATION_CONFIG\" --start_idx 0 --end_idx 1",
                "python evaluate_with_official.py --data_path \"$DATA_PATH\" --output_dir \"$ONE_CASE_DIR/outputs\" --verbose --start_idx 0 --end_idx 1",
            ],
            [
                "model_one_case_seed_41/logs",
                "model_one_case_seed_41/work",
                "model_one_case_seed_41/rendered_prompts/{case_id}/agent_prompt.md",
                "model_one_case_seed_41/prompt_render_manifest.json",
                "model_one_case_seed_41/outputs/eval_official_results.json",
                "model_one_case_seed_41/leaven_results.jsonl",
            ],
            "model-one-case",
            "held-out split reproduced",
            {
                "kind": "one-case",
                "split": "one-case",
                "case_id": "13-1",
                "case_range": "0..1",
                "case_count": 1,
                "denominator_contains": "one-case-13-1",
            },
            {
                "kind": "exact",
                "seed": 41,
            },
            {
                "kind": "upstream-run",
                "workers": 1,
                "max_turns": turn_budget,
            },
            {
                "kind": "upstream-eval",
                "required_source_command_fragments": [
                    "run_spreadsheetbench.py",
                    "evaluate_with_official.py",
                ],
            },
        ),
        stage(
            "G2",
            "Small-N held-out subset gate",
            "paper-subset",
            False,
            True,
            common_env
            + [
                "SEED=41",
                "SUBSET_START=200",
                "SUBSET_END=${SUBSET_END:?set small approved end <= 400}",
                "SUBSET_DIR=\"$RUN_ROOT/subset_${SUBSET_START}_${SUBSET_END}_seed_${SEED}\"",
                "python run_spreadsheetbench.py --data_path \"$DATA_PATH\" --model \"$MODEL\" --agent cli_skill_preloaded --log_dir \"$SUBSET_DIR/logs\" --log_format markdown --working_dir \"$SUBSET_DIR/work\" --output_dir \"$SUBSET_DIR/outputs\" --max_turns \"$MAX_TURNS\" --workers \"$WORKERS\" --skills_dir spreadsheet_agent/skills --seeds \"$SEED\" --generation_config \"$GENERATION_CONFIG\" --start_idx \"$SUBSET_START\" --end_idx \"$SUBSET_END\"",
                "python evaluate_with_official.py --data_path \"$DATA_PATH\" --output_dir \"$SUBSET_DIR/outputs\" --verbose --start_idx \"$SUBSET_START\" --end_idx \"$SUBSET_END\"",
            ],
            [
                "subset_<start>_<end>_seed_41/logs",
                "subset_<start>_<end>_seed_41/work",
                "subset_<start>_<end>_seed_41/rendered_prompts/{case_id}/agent_prompt.md",
                "subset_<start>_<end>_seed_41/prompt_render_manifest.json",
                "subset_<start>_<end>_seed_41/outputs/eval_official_results.json",
                "subset_<start>_<end>_seed_41/leaven_results.jsonl",
            ],
            "paper-subset",
            "held-out split reproduced",
            {
                "kind": "held-out-subset",
                "split": held_out["name"],
                "range_start_min": 200,
                "range_end_max": 400,
                "case_count_min": 1,
                "case_count_max_exclusive": held_out["case_count"],
                "denominator_contains": "subset",
                "forbidden_denominator_fragments": ["paper-denominator", "full-paper"],
            },
            {
                "kind": "exact",
                "seed": 41,
            },
            {
                "kind": "upstream-run",
                "workers": workers,
                "max_turns": turn_budget,
            },
            {
                "kind": "upstream-eval",
                "required_source_command_fragments": [
                    "run_spreadsheetbench.py",
                    "evaluate_with_official.py",
                ],
            },
        ),
        stage(
            "G3",
            "Evolving split trajectory and skill evolution",
            "evolving-split-0..200",
            False,
            True,
            common_env
            + [
                "for SEED in \"${SEEDS[@]}\"; do",
                "  BASELINE_DIR=\"$RUN_ROOT/baseline_seed_${SEED}\"",
                "  python run_spreadsheetbench.py --data_path \"$DATA_PATH\" --model \"$MODEL\" --agent cli_skill_preloaded --log_dir \"$BASELINE_DIR/logs\" --log_format markdown --working_dir \"$BASELINE_DIR/work\" --output_dir \"$BASELINE_DIR/outputs\" --max_turns \"$MAX_TURNS\" --workers \"$WORKERS\" --skills_dir spreadsheet_agent/skills --seeds \"$SEED\" --generation_config \"$GENERATION_CONFIG\" --start_idx 0 --end_idx 200",
                "  python evaluate_with_official.py --data_path \"$DATA_PATH\" --output_dir \"$BASELINE_DIR/outputs\" --verbose --start_idx 0 --end_idx 200",
                "  python analyze_results.py --eval_results \"$BASELINE_DIR/outputs/eval_official_results.json\" --log_dir \"$BASELINE_DIR/logs\"",
                "  python analysis/run_error_analysis.py --data_path \"$DATA_PATH\" --work_dir \"$BASELINE_DIR/work\" --logs_dir \"$BASELINE_DIR/logs\" --output_dir \"$BASELINE_DIR/error_analysis\" --model \"$MODEL\" --workers \"$WORKERS\" --generation_config \"$GENERATION_CONFIG\" --max_turns \"$MAX_TURNS\"",
                "  python analysis/run_success_analysis_llm.py --logs_dir \"$BASELINE_DIR/logs\" --output_dir \"$BASELINE_DIR/success_analysis\" --model \"$MODEL\" --max_workers \"$WORKERS\" --generation_config \"$THINK_GENERATION_CONFIG\"",
                "  python analysis/parse_error_analysis_outputs.py --input_dir \"$BASELINE_DIR/error_analysis\" --output \"$BASELINE_DIR/error_analysis_parsed.json\"",
                "  python analysis/parse_success_analysis_outputs.py --input_dir \"$BASELINE_DIR/success_analysis\" --output \"$BASELINE_DIR/success_analysis_parsed.json\"",
                "  EVOLUTION_DIR=\"$RUN_ROOT/skill_evolution_seed_${SEED}/error_driven_skill_evolution\"",
                "  EVOLVED_SKILLS=\"$EVOLUTION_DIR/skills\"",
                "  mkdir -p \"$EVOLVED_SKILLS\" && cp -r spreadsheet_agent/skills/. \"$EVOLVED_SKILLS\"",
                "  python -m skill_evolver.run_parallel_skill_evolution --input-json \"$BASELINE_DIR/error_analysis_parsed.json\" --skill-dir \"$EVOLVED_SKILLS/xlsx\" --model \"$MODEL\" --verbose --batch-size 1 --merge-batch-size \"$MERGE_BATCH_SIZE\" --changelog \"$EVOLUTION_DIR/change.log\" --save-intermediates --intermediates-dir \"$EVOLUTION_DIR/intermediates\" --max-workers \"$WORKERS\" --prompt generic --generation-config \"$THINK_GENERATION_CONFIG\" --parse-failure-dir \"$EVOLUTION_DIR/parse_failures\" --patch-pipeline json --seed \"$SEED\"",
                "done",
            ],
            [
                "baseline_seed_{seed}/logs",
                "baseline_seed_{seed}/work",
                "baseline_seed_{seed}/rendered_prompts/{case_id}/agent_prompt.md",
                "baseline_seed_{seed}/prompt_render_manifest.json",
                "baseline_seed_{seed}/outputs/eval_official_results.json",
                "baseline_seed_{seed}/error_analysis_parsed.json",
                "baseline_seed_{seed}/stage2_analyst_prompts/{case_id}/error_prompt.md",
                "baseline_seed_{seed}/stage2_analyst_prompts/{case_id}/success_prompt.md",
                "baseline_seed_{seed}/stage2_fanout.jsonl",
                "baseline_seed_{seed}/success_analysis_parsed.json",
                "skill_evolution_seed_{seed}/error_driven_skill_evolution/change.log",
                "skill_evolution_seed_{seed}/error_driven_skill_evolution/intermediates",
                "skill_evolution_seed_{seed}/error_driven_skill_evolution/stage3_merge_prompts/{batch_id}.md",
                "skill_evolution_seed_{seed}/error_driven_skill_evolution/stage3_merge_manifest.json",
                "skill_evolution_seed_{seed}/error_driven_skill_evolution/skills",
            ],
            "evolving-split-run",
            "held-out result",
            {
                "kind": "exact-range",
                "split": evolving["name"],
                "case_range": evolving["range"],
                "case_count": evolving["case_count"],
                "denominator": "evolving-split-0..200",
            },
            {
                "kind": "one-of",
                "seeds": packet["protocol"]["seeds"],
            },
            {
                "kind": "skill-evolution",
                "workers": workers,
                "max_turns": turn_budget,
                "merge_batch_size": merge_batch_size,
            },
            {
                "kind": "skill-evolution",
                "required_source_command_fragments": [
                    "run_spreadsheetbench.py",
                    "evaluate_with_official.py",
                    "analyze_results.py",
                    "analysis/run_error_analysis.py",
                    "analysis/run_success_analysis_llm.py",
                    "skill_evolver.run_parallel_skill_evolution",
                ],
            },
        ),
        stage(
            "G3V",
            "Training-set validation and best-seed selection",
            "training-validation-0..200",
            False,
            True,
            common_env
            + [
                "for SEED in \"${SEEDS[@]}\"; do",
                "  EVOLVED_SKILLS=\"$RUN_ROOT/skill_evolution_seed_${SEED}/error_driven_skill_evolution/skills\"",
                "  VALIDATION_DIR=\"$RUN_ROOT/validation_train_seed_${SEED}\"",
                "  python run_spreadsheetbench.py --data_path \"$DATA_PATH\" --model \"$MODEL\" --log_dir \"$VALIDATION_DIR/baseline_logs\" --working_dir \"$VALIDATION_DIR/baseline_work\" --output_dir \"$VALIDATION_DIR/baseline_outputs\" --max_turns \"$MAX_TURNS\" --workers \"$WORKERS\" --skills_dir spreadsheet_agent/skills --seeds \"$SEED\" --generation_config \"$GENERATION_CONFIG\" --start_idx 0 --end_idx 200",
                "  python evaluate_with_official.py --data_path \"$DATA_PATH\" --output_dir \"$VALIDATION_DIR/baseline_outputs\" --start_idx 0 --end_idx 200",
                "  python run_spreadsheetbench.py --data_path \"$DATA_PATH\" --model \"$MODEL\" --log_dir \"$VALIDATION_DIR/evolved_logs\" --working_dir \"$VALIDATION_DIR/evolved_work\" --output_dir \"$VALIDATION_DIR/evolved_outputs\" --max_turns \"$MAX_TURNS\" --workers \"$WORKERS\" --skills_dir \"$EVOLVED_SKILLS\" --seeds \"$SEED\" --generation_config \"$GENERATION_CONFIG\" --start_idx 0 --end_idx 200",
                "  python evaluate_with_official.py --data_path \"$DATA_PATH\" --output_dir \"$VALIDATION_DIR/evolved_outputs\" --start_idx 0 --end_idx 200",
                "done",
                "# Select BEST_SEED from training-set validation only; do not inspect held-out outputs.",
            ],
            [
                "validation_train_seed_{seed}/baseline_rendered_prompts/{case_id}/agent_prompt.md",
                "validation_train_seed_{seed}/baseline_prompt_render_manifest.json",
                "validation_train_seed_{seed}/baseline_outputs/eval_official_results.json",
                "validation_train_seed_{seed}/evolved_rendered_prompts/{case_id}/agent_prompt.md",
                "validation_train_seed_{seed}/evolved_prompt_render_manifest.json",
                "validation_train_seed_{seed}/evolved_outputs/eval_official_results.json",
                "best_seed_selection_note.md",
            ],
            "training-validation-candidate",
            "held-out result",
            {
                "kind": "exact-range",
                "split": evolving["name"],
                "case_range": evolving["range"],
                "case_count": evolving["case_count"],
                "denominator": "training-validation-0..200",
            },
            {
                "kind": "one-of",
                "seeds": packet["protocol"]["seeds"],
            },
            {
                "kind": "upstream-run",
                "workers": workers,
                "max_turns": turn_budget,
            },
            {
                "kind": "upstream-eval",
                "required_source_command_fragments": [
                    "run_spreadsheetbench.py",
                    "evaluate_with_official.py",
                ],
            },
        ),
        stage(
            "G4",
            "Held-out split evaluation",
            "held-out-200..400",
            False,
            True,
            common_env
            + [
                "BEST_SEED=${BEST_SEED:?select using training-set validation only}",
                "EVOLVED_SKILLS=\"$RUN_ROOT/skill_evolution_seed_${BEST_SEED}/error_driven_skill_evolution/skills\"",
                "EVOLVED_RUN_DIR=\"$RUN_ROOT/heldout_seed_${BEST_SEED}\"",
                "python run_spreadsheetbench.py --data_path \"$DATA_PATH\" --model \"$MODEL\" --log_dir \"$EVOLVED_RUN_DIR/logs\" --log_format markdown --working_dir \"$EVOLVED_RUN_DIR/work\" --output_dir \"$EVOLVED_RUN_DIR/outputs\" --max_turns \"$MAX_TURNS\" --workers \"$WORKERS\" --skills_dir \"$EVOLVED_SKILLS\" --seeds \"$BEST_SEED\" --generation_config \"$GENERATION_CONFIG\" --start_idx 200 --end_idx 400",
                "python evaluate_with_official.py --data_path \"$DATA_PATH\" --output_dir \"$EVOLVED_RUN_DIR/outputs\" --start_idx 200 --end_idx 400",
            ],
            [
                "heldout_seed_<best>/logs",
                "heldout_seed_<best>/work",
                "heldout_seed_<best>/rendered_prompts/{case_id}/agent_prompt.md",
                "heldout_seed_<best>/prompt_render_manifest.json",
                "heldout_seed_<best>/outputs/eval_official_results.json",
                "heldout_seed_<best>/leaven_results.jsonl",
            ],
            "held-out-single-seed-candidate",
            "paper aggregate",
            {
                "kind": "exact-range",
                "split": held_out["name"],
                "case_range": held_out["range"],
                "case_count": held_out["case_count"],
                "denominator": "held-out-200..400",
            },
            {
                "kind": "one-of",
                "seeds": packet["protocol"]["seeds"],
            },
            {
                "kind": "upstream-run",
                "workers": workers,
                "max_turns": turn_budget,
            },
            {
                "kind": "upstream-eval",
                "required_source_command_fragments": [
                    "run_spreadsheetbench.py",
                    "evaluate_with_official.py",
                ],
            },
        ),
        stage(
            "G5",
            "Seed aggregate and result rows",
            "seed-aggregate-41-42-43",
            False,
            True,
            [
                "uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench",
                "# After all three approved seed runs finish, write denominator-labeled rows to:",
                "# docs/ara/trace2skill_spreadsheetbench/results/<approved-run-id>.jsonl",
                "uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench",
                "uv run --with pyyaml python scripts/audit_trace2skill_closeout.py docs/ara/trace2skill_spreadsheetbench",
            ],
            [
                "docs/ara/trace2skill_spreadsheetbench/results/<approved-run-id>.jsonl",
                "docs/ara/trace2skill_spreadsheetbench/results/<approved-run-id>.prompt_render_manifest.json",
                "docs/ara/trace2skill_spreadsheetbench/plots/trace2skill_targets.png",
                "docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.json",
            ],
            "seed-aggregate-candidate",
            "cross-model paper reproduction",
            {
                "kind": "aggregate",
                "split": held_out["name"],
                "case_range": held_out["range"],
                "case_count": held_out["case_count"],
                "denominator": "seed-aggregate-41-42-43",
                "seeds": packet["protocol"]["seeds"],
            },
            {
                "kind": "all-of",
                "seeds": packet["protocol"]["seeds"],
            },
            None,
            None,
            {
                "kind": "seed-aggregate",
                "source_runbook_stage_id": "G4",
                "source_proof_classification": "held-out-single-seed-candidate",
                "required_seeds": packet["protocol"]["seeds"],
                "source_result_paths_min": len(packet["protocol"]["seeds"]),
            },
        ),
        stage(
            "G6",
            "Cross-model paper rows",
            "full-paper-denominator",
            False,
            True,
            [
                "# Repeat G3-G5 for each approved paper model/condition row being claimed.",
                "# Do not mark paper-denominator-reproduction until closeout_audit.json can prove every claimed row.",
                "uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench",
                "uv run --with pyyaml python scripts/audit_trace2skill_closeout.py docs/ara/trace2skill_spreadsheetbench",
            ],
            [
                "complete denominator-labeled result JSONL rows",
                "complete rendered prompt manifests for every claimed model call",
                "updated closeout_audit.json with overall_complete true only after objective-wide proof",
            ],
            "paper-denominator-reproduction",
            "anything stronger than completed rows",
            {
                "kind": "full-paper",
                "split": "all",
                "case_range": f"0..{manifest['case_count']}",
                "denominator": "full-paper-denominator",
                "required_split_ranges": [evolving["range"], held_out["range"]],
                "case_count": manifest["case_count"],
                "seeds": packet["protocol"]["seeds"],
            },
            {
                "kind": "all-of",
                "seeds": packet["protocol"]["seeds"],
            },
            None,
            None,
            {
                "kind": "full-paper",
                "source_proof_classifications": [
                    "training-validation-candidate",
                    "seed-aggregate-candidate",
                ],
                "source_result_paths_min": 1,
            },
        ),
    ]

    return {
        "schema_version": "leaven.trace2skill.runbook.v1",
        "approval_state": {
            "normal_preflight_passes": not approval_errors,
            "blocked_reasons": approval_errors,
        },
        "dataset": {
            "case_count": manifest["case_count"],
            "case_order_sha256": manifest["case_order"]["sha256"],
            "splits": manifest["splits"],
        },
        "paper_protocol": {
            "seeds": packet["protocol"]["seeds"],
            "workers": workers,
            "merge_batch_size": merge_batch_size,
            "react_turn_budget": turn_budget,
            "dataset_path": data_path,
        },
        "stages": stages,
    }


def write_markdown(runbook: dict[str, Any], output: Path) -> None:
    lines = [
        "# Trace2Skill Full-Denominator Runbook",
        "",
        "This runbook is generated from the ARA approval packet and dataset manifest.",
        "It is not permission to launch Qwen/vLLM work.",
        "",
        f"Normal approval preflight passes: `{str(runbook['approval_state']['normal_preflight_passes']).lower()}`",
        "",
        "## Paper Protocol",
        "",
        f"- Dataset path: `{runbook['paper_protocol']['dataset_path']}`",
        f"- Seeds: `{runbook['paper_protocol']['seeds']}`",
        f"- Workers: `{runbook['paper_protocol']['workers']}`",
        f"- Merge batch size: `{runbook['paper_protocol']['merge_batch_size']}`",
        f"- ReAct turn budget: `{runbook['paper_protocol']['react_turn_budget']}`",
        f"- Case order SHA-256: `{runbook['dataset']['case_order_sha256']}`",
        "",
        "## Approval Blockers",
        "",
    ]
    if runbook["approval_state"]["blocked_reasons"]:
        lines.extend(f"- {reason}" for reason in runbook["approval_state"]["blocked_reasons"])
    else:
        lines.append("- none")
    lines.extend(["", "## Stages", ""])

    for stage_item in runbook["stages"]:
        lines.extend(
            [
                f"### {stage_item['id']}: {stage_item['title']}",
                "",
                f"- Denominator: `{stage_item['denominator']}`",
                f"- Runnable now: `{str(stage_item['runnable_now']).lower()}`",
                f"- Approval required: `{str(stage_item['approval_required']).lower()}`",
                f"- Allowed label: `{stage_item['allowed_label']}`",
                f"- Forbidden label: `{stage_item['forbidden_label']}`",
                f"- Expected dataset slice: `{json.dumps(stage_item['expected_dataset_slice'], sort_keys=True)}`",
                f"- Expected seed policy: `{json.dumps(stage_item['expected_seed_policy'], sort_keys=True)}`",
                f"- Expected runtime policy: `{json.dumps(stage_item['expected_runtime_policy'], sort_keys=True)}`",
                f"- Expected command policy: `{json.dumps(stage_item['expected_command_policy'], sort_keys=True)}`",
                f"- Expected aggregate policy: `{json.dumps(stage_item['expected_aggregate_policy'], sort_keys=True)}`",
                "",
                "Commands:",
                "",
                shell_block(stage_item["commands"]),
                "",
                "Expected artifacts:",
            ]
        )
        lines.extend(f"- `{artifact}`" for artifact in stage_item["expected_artifacts"])
        lines.append("")

    output.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "ara_dir",
        type=Path,
        nargs="?",
        default=Path("docs/ara/trace2skill_spreadsheetbench"),
    )
    parser.add_argument(
        "--output-json",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.json"),
    )
    parser.add_argument(
        "--output-md",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.md"),
    )
    args = parser.parse_args()

    repo_root = Path.cwd().resolve()
    ara_dir = args.ara_dir.resolve()
    runbook = build_runbook(repo_root, ara_dir)
    args.output_json.write_text(json.dumps(runbook, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    write_markdown(runbook, args.output_md)
    print(args.output_json)
    print(args.output_md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
