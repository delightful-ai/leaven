"""Private Rust-owned checkpoint materialization for SDK prompt runs."""

import json
import subprocess
from pathlib import Path

from .._seam_optimize import PlannedOptimizeCase, SeamOptimizeReport
from ..artifacts.prompt import PromptArtifact
from ..result import Optimized, RunSummary
from ..run_status import project_cost_usage
from .rust_export import resolve_leaven_binary
from .rust_open import open_rust_optimized

SDK_PROMPT_RUN_RECORD_SCHEMA = "leaven.sdk_prompt_run_record.v1"


def persist_rust_prompt_checkpoint(
    *,
    seed: PromptArtifact,
    cases: list[PlannedOptimizeCase],
    report: SeamOptimizeReport,
    run_id: str,
    root: str | Path = ".leaven/runs",
) -> Optimized[object]:
    """Materialize an SDK prompt mechanics report into a Rust-owned checkpoint."""
    run_dir = Path(root) / _run_dir_name(run_id)
    run_dir.mkdir(parents=True, exist_ok=True)
    input_path = run_dir / "sdk_prompt_run_record.json"
    _write_record(input_path, seed=seed, cases=cases, report=report, run_id=run_id)
    process = subprocess.run(
        [
            str(resolve_leaven_binary()),
            "run",
            "checkpoint-sdk-prompt",
            "--input",
            str(input_path),
            "--run-dir",
            str(run_dir),
        ],
        text=True,
        capture_output=True,
        timeout=60,
        check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(
            "leaven run checkpoint-sdk-prompt failed\n"
            f"status: {process.returncode}\nstdout:\n{process.stdout}\nstderr:\n{process.stderr}"
        )
    result = open_rust_optimized(run_dir)
    if result is None:
        raise RuntimeError("Rust prompt checkpoint materialization produced no readback")
    return _with_report_status(result, report)


def _write_record(
    path: Path,
    *,
    seed: PromptArtifact,
    cases: list[PlannedOptimizeCase],
    report: SeamOptimizeReport,
    run_id: str,
) -> None:
    record = {
        "schema_version": SDK_PROMPT_RUN_RECORD_SCHEMA,
        "run_id": run_id,
        "seed": {
            "template": seed.template,
            "candidate_id": "cand_seed",
        },
        "cases": [
            {
                "case_id": case.case_id,
                "input": case.input,
                "target": case.target,
                "split": case.split,
            }
            for case in cases
        ],
        "assessments": [
            {
                "case_id": assessment.case_id,
                "target": assessment.case_target,
                "output": assessment.output,
                "score": assessment.score.value,
                "feedback": assessment.score.feedback,
                "rewards": [
                    {
                        "id": reward.id,
                        "value": reward.value,
                        "weight": reward.weight,
                        "feedback": reward.feedback,
                    }
                    for reward in assessment.rewards
                ],
                "effect_receipts": [
                    receipt.receipt_id for receipt in assessment.effect_receipts
                ],
            }
            for assessment in report.assessments
        ],
        "total_lm_tokens": report.total_lm_tokens,
    }
    tmp = path.with_name(f".{path.name}.tmp")
    tmp.write_text(json.dumps(record, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    tmp.replace(path)


def _run_dir_name(run_id: str) -> str:
    cleaned = "".join(ch if ch.isalnum() or ch in "._-" else "_" for ch in run_id)
    return cleaned or "leaven_run"


def _with_report_status(result: Optimized[object], report: SeamOptimizeReport) -> Optimized[object]:
    status = project_cost_usage(
        default_cost_usd=report.total_cost_usd,
        default_lm_tokens=report.total_lm_tokens,
        unsupported=report.unsupported,
    )
    summary = result.summary.model_copy(
        update={
            "total_cost_usd": status.total_cost_usd,
            "cost_status": status.cost_status,
            "total_lm_tokens": status.total_lm_tokens,
            "usage_status": status.usage_status,
            "unsupported": report.unsupported,
        }
    )
    typed_summary = RunSummary.model_validate(summary)
    return result.model_copy(
        update={
            "summary": typed_summary,
            "proposal_receipts": report.proposal_receipts,
            "effect_receipts": report.effect_receipts,
        }
    )


__all__ = ["persist_rust_prompt_checkpoint"]
