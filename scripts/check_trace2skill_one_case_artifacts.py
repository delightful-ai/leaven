#!/usr/bin/env python3
"""Check deterministic Trace2Skill one-case artifacts and result row consistency."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


RUN_DIR = Path("tmp/trace2skill-one-case-live")
RESULT_JSONL = Path("docs/ara/trace2skill_spreadsheetbench/results/deterministic_one_case.jsonl")
EXPECTED_OUTPUT_SHA256 = "131cf073e40f73b5f152d3a4d718532ee6c980e467e48e1a136e1275cd31bf40"
EXPECTED_OUTPUT_BYTES = 8423
EXPECTED_TRANSCRIPT_SHA256 = "b55441e92bb38613f4f6932e703a1c0f188fd4ce6e07aa5b55ba6cb9f5a6dde7"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def jsonl_records(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def fail_unless(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def check_required_files(repo_root: Path, run_dir: Path, errors: list[str]) -> dict[str, Path]:
    files = {
        "manifest": run_dir / "manifest.json",
        "prompt": run_dir / "agent_prompt.md",
        "init_workbook": run_dir / "1_13-1_init.xlsx",
        "golden_workbook": run_dir / "1_13-1_golden.xlsx",
        "acp": run_dir / "acp_result.json",
        "transcript": run_dir / "agent_transcript.md",
        "score": run_dir / "score_report.json",
        "trajectory": run_dir / "trajectory.json",
        "output_workbook": run_dir / "13-1_output.xlsx",
        "worker": run_dir / "trace2skill_acp_worker.py",
    }
    for label, rel_path in files.items():
        if not (repo_root / rel_path).is_file():
            errors.append(f"missing {label}: {rel_path}")
    return files


def check_one_case_artifacts(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    files = check_required_files(repo_root, RUN_DIR, errors)
    if errors:
        return errors

    score = load_json(repo_root / files["score"])
    manifest = load_json(repo_root / files["manifest"])
    acp = load_json(repo_root / files["acp"])
    trajectory = load_json(repo_root / files["trajectory"])
    records = jsonl_records(repo_root / RESULT_JSONL)
    output_workbook = repo_root / files["output_workbook"]
    transcript = repo_root / files["transcript"]
    one_case_doc = ara_root / "results/one_case_live.md"

    output_sha = sha256_file(output_workbook)
    transcript_sha = sha256_file(transcript)

    fail_unless(errors, score.get("case_id") == "13-1", "score_report.json case_id must be 13-1")
    fail_unless(errors, score.get("passed") is True, "score_report.json passed must be true")
    fail_unless(errors, score.get("score") == 1.0, "score_report.json score must be 1.0")
    fail_unless(errors, score.get("matched_cells") == 120, "score_report.json matched_cells must be 120")
    fail_unless(errors, score.get("total_cells") == 120, "score_report.json total_cells must be 120")
    fail_unless(errors, score.get("mismatches") == [], "score_report.json mismatches must be empty")
    fail_unless(
        errors,
        score.get("candidate_workbook", {}).get("path") == files["output_workbook"].as_posix(),
        "score_report.json candidate_workbook.path must point at 13-1_output.xlsx",
    )
    fail_unless(
        errors,
        score.get("candidate_workbook", {}).get("bytes") == EXPECTED_OUTPUT_BYTES,
        f"score_report.json candidate_workbook.bytes must be {EXPECTED_OUTPUT_BYTES}",
    )

    fail_unless(errors, output_workbook.stat().st_size == EXPECTED_OUTPUT_BYTES, "output workbook byte size changed")
    fail_unless(errors, output_sha == EXPECTED_OUTPUT_SHA256, "output workbook SHA-256 changed")
    fail_unless(errors, transcript_sha == EXPECTED_TRANSCRIPT_SHA256, "agent transcript SHA-256 changed")

    fail_unless(errors, manifest.get("case_id") == "13-1", "manifest.json case_id must be 13-1")
    fail_unless(
        errors,
        manifest.get("status") == "scored_candidate_workbook",
        "manifest.json status must be scored_candidate_workbook",
    )
    fail_unless(
        errors,
        manifest.get("prompt_file") == files["prompt"].as_posix(),
        "manifest.json prompt_file must point at agent_prompt.md",
    )
    fail_unless(
        errors,
        manifest.get("init_workbook") == files["init_workbook"].as_posix(),
        "manifest.json init_workbook must point at 1_13-1_init.xlsx",
    )
    fail_unless(
        errors,
        manifest.get("golden_workbook") == files["golden_workbook"].as_posix(),
        "manifest.json golden_workbook must point at 1_13-1_golden.xlsx",
    )
    fail_unless(
        errors,
        manifest.get("output_workbook") == files["output_workbook"].as_posix(),
        "manifest.json output_workbook must point at 13-1_output.xlsx",
    )

    command = acp.get("primary", {}).get("commands", [{}])[0]
    acp_file = command.get("files", {}).get("13-1_output.xlsx", {})
    fail_unless(errors, acp.get("method") == "leaven/agent.run", "acp_result.json method must be leaven/agent.run")
    fail_unless(errors, acp.get("primary", {}).get("status") == "completed", "ACP primary status must be completed")
    fail_unless(errors, command.get("status") == "completed", "ACP command status must be completed")
    fail_unless(errors, acp_file.get("bytes") == EXPECTED_OUTPUT_BYTES, "ACP output workbook bytes changed")
    fail_unless(errors, acp_file.get("sha256") == EXPECTED_OUTPUT_SHA256, "ACP output workbook SHA-256 changed")
    fail_unless(
        errors,
        acp.get("primary", {}).get("transcript_ref", {}).get("sha256") == EXPECTED_TRANSCRIPT_SHA256,
        "ACP transcript SHA-256 changed",
    )
    fail_unless(errors, acp.get("primary", {}).get("cost", {}).get("agent_calls") == 1, "ACP agent_calls must be 1")
    fail_unless(errors, acp.get("primary", {}).get("cost", {}).get("usd_micro") == 0, "ACP usd_micro must be 0")
    fail_unless(errors, acp.get("receipts", [{}])[0].get("status") == "succeeded", "ACP receipt must succeed")

    fail_unless(errors, trajectory.get("task_id") == "13-1", "trajectory.json task_id must be 13-1")
    fail_unless(errors, trajectory.get("outcome") == "Success", "trajectory.json outcome must be Success")
    fail_unless(
        errors,
        trajectory.get("model_id") == "local-openpyxl-trace2skill-agent",
        "trajectory.json model_id must be local-openpyxl-trace2skill-agent",
    )
    analysis_sources = [entry.get("source_file") for entry in trajectory.get("analysis_records", [])]
    fail_unless(errors, "score_report.json" in analysis_sources, "trajectory.json must reference score_report.json")

    if len(records) != 1:
        errors.append(f"deterministic_one_case.jsonl must contain one record, found {len(records)}")
        return errors
    record = records[0]
    fail_unless(errors, record.get("proof_classification") == "deterministic-one-case", "result proof_classification changed")
    fail_unless(errors, record.get("plot_binding") is None, "deterministic one-case result must keep plot_binding null")
    fail_unless(errors, record.get("metric_name") == "workbook_score", "result metric_name must be workbook_score")
    fail_unless(errors, record.get("metric_value") == 1.0, "result metric_value must be 1.0")
    fail_unless(
        errors,
        record.get("dataset_slice", {}).get("denominator") == "one-case-13-1-only",
        "result denominator must be one-case-13-1-only",
    )
    fail_unless(
        errors,
        record.get("model_id") == "local-openpyxl-trace2skill-agent",
        "result model_id must be local-openpyxl-trace2skill-agent",
    )
    for rel_path in record.get("artifact_paths", []):
        if not (repo_root / rel_path).is_file():
            errors.append(f"result artifact path is not inspectable: {rel_path}")
    extra = record.get("extra", {})
    fail_unless(errors, extra.get("case_id") == "13-1", "result extra.case_id must be 13-1")
    fail_unless(errors, extra.get("matched_cells") == 120, "result extra.matched_cells must be 120")
    fail_unless(errors, extra.get("total_cells") == 120, "result extra.total_cells must be 120")
    for label in ("prompt", "init_workbook", "golden_workbook"):
        rel_path = files[label].as_posix()
        if rel_path not in record.get("artifact_paths", []):
            errors.append(f"result artifact_paths must include prepared {label}: {rel_path}")

    if one_case_doc.is_file():
        doc = one_case_doc.read_text(encoding="utf-8")
        doc_needles = [
            EXPECTED_OUTPUT_SHA256,
            "`plot_binding` is intentionally `null`",
            "Not a paper reproduction",
            "| Score | `1.0` |",
            "| Matched cells | `120` |",
            "| Total cells | `120` |",
            "| Passed | `true` |",
            f"| Output workbook bytes | `{EXPECTED_OUTPUT_BYTES}` |",
            f"`{files['prompt'].as_posix()}`",
            f"`{files['init_workbook'].as_posix()}`",
            f"`{files['golden_workbook'].as_posix()}`",
            f"`{files['output_workbook'].as_posix()}`",
        ]
        for needle in doc_needles:
            if needle not in doc:
                errors.append(f"one_case_live.md missing {needle!r}")
    else:
        errors.append(f"missing {one_case_doc}")
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
    repo_root = repo_root_for(ara_root)
    errors = check_one_case_artifacts(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} deterministic one-case artifacts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
