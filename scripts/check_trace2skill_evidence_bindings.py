#!/usr/bin/env python3
"""Check Trace2Skill ARA claim, experiment, and evidence-index bindings."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


EXPECTED_EVIDENCE_FILES = {
    "tables/table_main_spreadsheetbench.md",
    "tables/table_parallel_vs_sequential.md",
    "tables/table_reasoningbank.md",
    "tables/table_agentic_ablation.md",
    "tables/table_math.md",
    "tables/table_vqa.md",
    "figures/figure_trace2skill_framework.md",
    "leaven_mechanics_tests.md",
    "prompt_templates.md",
    "stage2_rendered_prompts.md",
}


@dataclass(frozen=True)
class EvidenceRow:
    evidence_file: str
    source: str
    claims: list[str]


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "docs/ara/trace2skill_spreadsheetbench").is_dir():
            return candidate
    return Path.cwd()


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def markdown_ids(pattern: str, text: str) -> set[str]:
    return set(re.findall(pattern, text, re.M))


def parse_evidence_rows(evidence_readme: Path) -> list[EvidenceRow]:
    rows: list[EvidenceRow] = []
    for line in read(evidence_readme).splitlines():
        if not line.startswith("|") or line.startswith("|-"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) < 4 or cells[0] == "Evidence file":
            continue
        link_match = re.search(r"\(([^)]+)\)", cells[0])
        if not link_match:
            continue
        source_match = re.search(r"`([^`]+)`", cells[1])
        rows.append(
            EvidenceRow(
                evidence_file=link_match.group(1),
                source=source_match.group(1) if source_match else cells[1],
                claims=re.findall(r"C\d+", cells[2]),
            )
        )
    return rows


def source_exists(repo_root: Path, source: str) -> bool:
    return (repo_root / source).exists()


def check_evidence_bindings(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    evidence_dir = ara_root / "evidence"
    claims_text = read(ara_root / "logic/claims.md")
    experiments_text = read(ara_root / "logic/experiments.md")
    rows = parse_evidence_rows(evidence_dir / "README.md")

    claim_ids = markdown_ids(r"^## (C\d+):", claims_text)
    experiment_ids = markdown_ids(r"^## (E\d+):", experiments_text)
    indexed_files = {row.evidence_file for row in rows}

    actual_evidence_files = {
        path.relative_to(evidence_dir).as_posix()
        for path in evidence_dir.rglob("*.md")
        if path.name != "README.md"
    }
    if indexed_files != actual_evidence_files:
        missing = sorted(actual_evidence_files - indexed_files)
        extra = sorted(indexed_files - actual_evidence_files)
        if missing:
            errors.append(f"evidence/README.md missing files: {', '.join(missing)}")
        if extra:
            errors.append(f"evidence/README.md indexes nonexistent files: {', '.join(extra)}")

    if EXPECTED_EVIDENCE_FILES - actual_evidence_files:
        errors.append(
            "ARA evidence tree missing expected files: "
            + ", ".join(sorted(EXPECTED_EVIDENCE_FILES - actual_evidence_files))
        )
    if actual_evidence_files - EXPECTED_EVIDENCE_FILES:
        errors.append(
            "ARA evidence tree has unclassified extra files: "
            + ", ".join(sorted(actual_evidence_files - EXPECTED_EVIDENCE_FILES))
        )

    claims_with_evidence: set[str] = set()
    for row in rows:
        evidence_path = evidence_dir / row.evidence_file
        if not evidence_path.is_file():
            errors.append(f"indexed evidence file missing: {row.evidence_file}")
            continue
        text = read(evidence_path)
        if "**Source**" not in text:
            errors.append(f"{row.evidence_file} missing **Source** field")
        if not row.claims:
            errors.append(f"{row.evidence_file} has no indexed claims")
        for claim in row.claims:
            if claim not in claim_ids:
                errors.append(f"{row.evidence_file} indexes unknown claim {claim}")
            claims_with_evidence.add(claim)
        if not source_exists(repo_root, row.source):
            errors.append(f"{row.evidence_file} source path is not inspectable: {row.source}")

    for claim in sorted(claim_ids):
        if claim not in claims_with_evidence:
            errors.append(f"{claim} has no evidence/README.md row")

        block_match = re.search(rf"^## {claim}:.*?(?=^## C\d+:|\Z)", claims_text, re.M | re.S)
        block = block_match.group(0) if block_match else ""
        proof_match = re.search(r"\*\*Proof\*\*:\s*\[([^\]]+)\]", block)
        if not proof_match:
            errors.append(f"{claim} missing proof experiment list")
            continue
        for experiment_id in re.findall(r"E\d+", proof_match.group(1)):
            if experiment_id not in experiment_ids:
                errors.append(f"{claim} references unknown experiment {experiment_id}")

    for experiment_id in sorted(experiment_ids):
        block_match = re.search(rf"^## {experiment_id}:.*?(?=^## E\d+:|\Z)", experiments_text, re.M | re.S)
        block = block_match.group(0) if block_match else ""
        verifies_match = re.search(r"\*\*Verifies\*\*:\s*([^\n]+)", block)
        if not verifies_match:
            errors.append(f"{experiment_id} missing Verifies line")
            continue
        for claim in re.findall(r"C\d+", verifies_match.group(1)):
            if claim not in claim_ids:
                errors.append(f"{experiment_id} verifies unknown claim {claim}")

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
    errors = check_evidence_bindings(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} evidence bindings")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
