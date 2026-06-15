#!/usr/bin/env python3
"""Check rendered Trace2Skill one-case Stage 2 prompt artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


RUN_DIR = Path("tmp/trace2skill-one-case-live")
PROMPT_PATH = RUN_DIR / "stage2_analyst_prompt.md"
FANOUT_PATH = RUN_DIR / "stage2_fanout.json"
EVIDENCE_DOC = Path("docs/ara/trace2skill_spreadsheetbench/evidence/stage2_rendered_prompts.md")
UPSTREAM_PROMPT_DIR = Path("tmp/repros/trace2skill-upstream/skill_evolver/prompts")
EXPECTED_PROMPT_BYTES = 13031
EXPECTED_PROMPT_SHA256 = "94893fef2c3459bbe76bb63854dd2e9aab813625877c584867d34eadba700ba4"
EXPECTED_FANOUT_BYTES = 654
EXPECTED_FANOUT_SHA256 = "71856dffdfbb4db1ebcfa43f32845a44ef1c37021f6965215523a9fbd33dd8c8"
EXPECTED_SOURCE_PROMPTS = [
    "skill_evolving_agent/system_prompt_base.txt",
    "parallel_evolving_agent/map_output_format.txt",
    "success_evolving_agent/success_record_section.txt",
    "success_evolving_agent/success_modification_strategies_section.txt",
    "success_evolving_agent/success_intro_replacement.txt",
    "success_evolving_agent/success_input_replacement.txt",
    "success_evolving_agent/success_goal_replacement.txt",
    "success_evolving_agent/success_first_constraint_replacement.txt",
    "success_evolving_agent/success_traceability_constraint.txt",
    "success_evolving_agent/success_output_reasoning_replacement.txt",
    "success_evolving_agent/success_analysis_records_header.txt",
    "success_evolving_agent/current_skill_folder_header.txt",
    "success_evolving_agent/skill_folder_size_status_header.txt",
    "success_evolving_agent/skill_md_status_line.txt",
    "success_evolving_agent/reference_files_status_line.txt",
    "success_evolving_agent/size_warning.txt",
]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return loaded


def fail_unless(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def check_source_prompt_blocks(repo_root: Path, prompt: str, errors: list[str]) -> None:
    source_dir = repo_root / UPSTREAM_PROMPT_DIR
    for relative in EXPECTED_SOURCE_PROMPTS:
        source_path = source_dir / relative
        if not source_path.is_file():
            errors.append(f"missing upstream prompt source: {UPSTREAM_PROMPT_DIR / relative}")
            continue
        contents = source_path.read_text(encoding="utf-8")
        fail_unless(errors, f"### {relative}" in prompt, f"rendered prompt missing heading for {relative}")
        fail_unless(errors, contents in prompt, f"rendered prompt missing exact source text for {relative}")

    heading_count = sum(1 for relative in EXPECTED_SOURCE_PROMPTS if f"### {relative}" in prompt)
    fail_unless(
        errors,
        heading_count == len(EXPECTED_SOURCE_PROMPTS),
        f"rendered prompt has {heading_count} expected source headings, expected {len(EXPECTED_SOURCE_PROMPTS)}",
    )


def check_fanout(fanout: dict[str, Any], errors: list[str]) -> None:
    fail_unless(errors, fanout.get("expected_call_ids") == ["success-13-1-1"], "fanout expected_call_ids changed")
    calls = fanout.get("calls")
    if not isinstance(calls, list) or len(calls) != 1 or not isinstance(calls[0], dict):
        errors.append("fanout must contain exactly one call object")
        return
    call = calls[0]
    fail_unless(errors, call.get("call_id") == "success-13-1-1", "fanout call_id changed")
    fail_unless(errors, call.get("role") == "Success", "fanout role must be Success")
    fail_unless(errors, call.get("source_task_ids") == ["13-1"], "fanout source_task_ids changed")
    fail_unless(errors, call.get("status") == "Pending", "fanout status must remain Pending")
    fail_unless(errors, call.get("response") is None, "fanout response must stay null before model execution")
    fail_unless(errors, call.get("retry_count") == 0, "fanout retry_count must be 0")
    fail_unless(errors, call.get("support_count") == 1, "fanout support_count must be 1")

    prompt_key = (
        call.get("prompt", {})
        .get("BlobRef", {})
        .get("reference", {})
        .get("key")
    )
    fail_unless(
        errors,
        prompt_key == PROMPT_PATH.as_posix(),
        f"fanout prompt key must point at {PROMPT_PATH.as_posix()}",
    )


def check_evidence_doc(repo_root: Path, errors: list[str]) -> None:
    doc_path = repo_root / EVIDENCE_DOC
    if not doc_path.is_file():
        errors.append(f"missing evidence doc: {EVIDENCE_DOC}")
        return
    doc = doc_path.read_text(encoding="utf-8")
    for needle in (
        EXPECTED_PROMPT_SHA256,
        EXPECTED_FANOUT_SHA256,
        PROMPT_PATH.as_posix(),
        FANOUT_PATH.as_posix(),
        "not executed an analyst model call",
    ):
        if needle not in doc:
            errors.append(f"stage2_rendered_prompts.md missing {needle!r}")


def check_stage2_prompt_artifacts(repo_root: Path, ara_root: Path) -> list[str]:
    del ara_root
    errors: list[str] = []
    prompt_path = repo_root / PROMPT_PATH
    fanout_path = repo_root / FANOUT_PATH
    for label, path in (("rendered prompt", prompt_path), ("fanout", fanout_path)):
        if not path.is_file():
            errors.append(f"missing {label}: {path.relative_to(repo_root)}")
    if errors:
        return errors

    prompt = prompt_path.read_text(encoding="utf-8")
    fanout = load_json(fanout_path)
    prompt_sha = sha256_file(prompt_path)
    fanout_sha = sha256_file(fanout_path)

    fail_unless(errors, prompt_path.stat().st_size == EXPECTED_PROMPT_BYTES, "rendered prompt byte size changed")
    fail_unless(errors, prompt_sha == EXPECTED_PROMPT_SHA256, "rendered prompt SHA-256 changed")
    fail_unless(errors, fanout_path.stat().st_size == EXPECTED_FANOUT_BYTES, "fanout byte size changed")
    fail_unless(errors, fanout_sha == EXPECTED_FANOUT_SHA256, "fanout SHA-256 changed")

    for needle in (
        "# Trace2Skill Stage 2 MAP Analyst Prompt Source",
        "This pending fan-out has not executed an analyst model call.",
        "call_id: success-13-1-1",
        "task_id: 13-1",
        "role: Success",
        "SuccessParallelSkillEvolver._build_map_system_prompt",
        "build_success_user_message",
        "trajectory_file: tmp/trace2skill-one-case-live/trajectory.json",
        "score_report_file: tmp/trace2skill-one-case-live/score_report.json",
        "score: 1 (120/120)",
        "artifact deliberately stops before model execution, parsing, or merge",
    ):
        if needle not in prompt:
            errors.append(f"rendered prompt missing {needle!r}")

    check_source_prompt_blocks(repo_root, prompt, errors)
    check_fanout(fanout, errors)
    check_evidence_doc(repo_root, errors)
    return errors


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "examples/trace2skill_spreadsheetbench").is_dir():
            return candidate
    return Path.cwd()


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
    repo_root = repo_root_for(ara_root).resolve()
    errors = check_stage2_prompt_artifacts(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} Stage 2 rendered prompt artifacts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
