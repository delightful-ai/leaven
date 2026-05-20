#!/usr/bin/env python3
"""Build one-sample paper-exact lane reports from materialized samples."""

from __future__ import annotations

import hashlib
import json
import shutil
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
SAMPLE_ROOT = REPO_ROOT / "tmp" / "paper_exact_samples"
LANE_ROOT = REPO_ROOT / "tmp" / "paper_exact_lanes"


def sha256_path(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def write_text(path: Path, data: str) -> dict[str, Any]:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(data)
    return {
        "path": str(path.relative_to(REPO_ROOT)),
        "bytes": path.stat().st_size,
        "sha256": sha256_path(path),
    }


def copy_source(src: str, dst_root: Path) -> dict[str, Any]:
    src_path = REPO_ROOT / src
    dst = dst_root / "sources" / src
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src_path, dst)
    return {
        "source": src,
        "lane_copy": str(dst.relative_to(REPO_ROOT)),
        "bytes": dst.stat().st_size,
        "sha256": sha256_path(dst),
    }


def collect_samples(manifest: dict[str, Any], paper: str) -> list[dict[str, Any]]:
    return [e for e in manifest["entries"] if e.get("paper") == paper]


def make_lane(
    manifest: dict[str, Any],
    key: str,
    paper: str,
    source_files: list[str],
    status: str,
    exactness: dict[str, Any],
    next_action: str,
) -> dict[str, Any]:
    lane_dir = LANE_ROOT / key
    copied = [copy_source(path, lane_dir) for path in source_files if (REPO_ROOT / path).exists()]
    missing = [path for path in source_files if not (REPO_ROOT / path).exists()]
    lane = {
        "paper": paper,
        "lane": key,
        "status": status,
        "sample_entries": collect_samples(manifest, paper),
        "prompt_and_setup_sources": copied,
        "missing_sources": missing,
        "exactness": exactness,
        "next_action": next_action,
    }
    write_json(lane_dir / "lane.json", lane)
    return {
        "paper": paper,
        "lane": key,
        "status": status,
        "lane_report": str((lane_dir / "lane.json").relative_to(REPO_ROOT)),
        "sample_count": len(lane["sample_entries"]),
        "copied_source_count": len(copied),
        "missing_source_count": len(missing),
    }


def render_preflights() -> dict[str, Any]:
    rendered: dict[str, Any] = {}

    officeqa = json.loads(
        (SAMPLE_ROOT / "evoskill" / "officeqa" / "officeqa_pro_first_case.json").read_text()
    )
    sealqa_prompt = (
        REPO_ROOT / "tmp" / "repros" / "evoskill" / "src" / "agent_profiles" / "sealqa_agent" / "prompt.txt"
    ).read_text()
    rendered["evoskill"] = write_text(
        LANE_ROOT / "evoskill" / "rendered_officeqa_prompt.md",
        "\n".join(
            [
                "# EvoSkill OfficeQA One-Sample Prompt Preflight",
                "",
                "This is a prompt/setup render only; the live EvoSkill loop still needs an",
                "adapter from the synthetic P5 fixture to this real OfficeQA case.",
                "",
                "## SealQA Agent Prompt Source",
                "",
                sealqa_prompt,
                "",
                "## OfficeQA Case",
                "",
                f"uid: {officeqa['uid']}",
                f"question: {officeqa['question']}",
                f"answer: {officeqa['answer']}",
                f"source_docs: {officeqa['source_docs']}",
                f"source_files: {officeqa['source_files']}",
                "",
                "## Materialized Source Text",
                "",
                "tmp/paper_exact_samples/evoskill/officeqa/treasury_bulletin_1941_01.txt",
                "",
            ]
        ),
    )

    trace_case = json.loads(
        (SAMPLE_ROOT / "trace2skill" / "spreadsheetbench_verified" / "dataset_first_case.json").read_text()
    )
    trace_system = (
        REPO_ROOT
        / "tmp"
        / "repros"
        / "trace2skill-upstream"
        / "spreadsheet_agent"
        / "system_prompt"
        / "cli_skill_preloaded_full_system_v1.txt"
    ).read_text()
    trace_task_prompt = (
        SAMPLE_ROOT / "trace2skill" / "spreadsheetbench_verified" / "13-1" / "prompt.txt"
    ).read_text()
    rendered["trace2skill"] = write_text(
        LANE_ROOT / "trace2skill" / "rendered_case_prompt.md",
        "\n".join(
            [
                "# Trace2Skill SpreadsheetBench Case 13-1 Prompt Preflight",
                "",
                "## System Prompt",
                "",
                trace_system,
                "",
                "## Dataset Instruction",
                "",
                trace_case["instruction"],
                "",
                "## Upstream prompt.txt",
                "",
                trace_task_prompt,
                "",
                "## Files",
                "",
                "- init: tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1/1_13-1_init.xlsx",
                "- golden: tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1/1_13-1_golden.xlsx",
                f"- answer_sheet: {trace_case['answer_sheet']}",
                f"- answer_position: {trace_case['answer_position']}",
                "",
            ]
        ),
    )

    gaia_blocked = json.loads(
        (SAMPLE_ROOT / "memento-skills" / "gaia" / "access_blocked.json").read_text()
    )
    hle_blocked = json.loads(
        (SAMPLE_ROOT / "memento-skills" / "hle" / "access_blocked.json").read_text()
    )
    rendered["memento-skills"] = write_text(
        LANE_ROOT / "memento-skills" / "gated_access_preflight.md",
        "\n".join(
            [
                "# Memento-Skills Gated Access Preflight",
                "",
                "GAIA and HLE are the benchmark surfaces for the exact lane, but this",
                "machine currently cannot download either dataset through Hugging Face.",
                "",
                f"- GAIA: {gaia_blocked['reason']}",
                f"- HLE: {hle_blocked['reason']}",
                "",
                "No surrogate sample is rendered here because that would make the lane",
                "non-exact unless explicitly approved.",
                "",
            ]
        ),
    )

    alfworld = json.loads((SAMPLE_ROOT / "d2skill" / "alfworld" / "first_traj_data.json").read_text())
    anns = alfworld["turk_annotations"]["anns"][0]
    actions = [
        step["discrete_action"]["action"] + "(" + ", ".join(step["discrete_action"].get("args", [])) + ")"
        for step in alfworld["plan"]["high_pddl"]
    ]
    d2_template = (
        REPO_ROOT
        / "tmp"
        / "repros"
        / "d2skill-agenticrl"
        / "agent_system"
        / "environments"
        / "prompts"
        / "alfworld.py"
    ).read_text()
    rendered["d2skill"] = write_text(
        LANE_ROOT / "d2skill" / "rendered_alfworld_prompt.md",
        "\n".join(
            [
                "# D2Skill ALFWorld One-Trajectory Prompt Preflight",
                "",
                "This render uses a real ALFWorld train trajectory but does not start the",
                "full TextWorld/THOR runtime or RL trainer.",
                "",
                "## D2Skill Prompt Source File",
                "",
                "tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/alfworld.py",
                "",
                "## Task",
                "",
                anns["task_desc"],
                "",
                "## High-Level Reference Actions",
                "",
                "\n".join(f"- {a}" for a in actions),
                "",
                "## Prompt Source Excerpt",
                "",
                "```python",
                d2_template.split('ALFWORLD_TEMPLATE = """', 1)[1].split('"""', 1)[0].strip(),
                "```",
                "",
            ]
        ),
    )

    skill = (
        SAMPLE_ROOT / "skillreducer" / "skillsbench" / "skills" / "skill-creator" / "SKILL.md"
    ).read_text()
    task_instruction = (
        SAMPLE_ROOT / "skillreducer" / "skillsbench" / "task" / "instruction.md"
    ).read_text()
    rendered["skillreducer"] = write_text(
        LANE_ROOT / "skillreducer" / "rendered_reduction_prompt.md",
        "\n".join(
            [
                "# SkillReducer One-Skill Prompt Preflight",
                "",
                "The exact SkillReducer prompt text is not exposed in local paper source;",
                "this render pins the real skill/task inputs for the next live reduction",
                "attempt and must be labeled prompt-non-exact until upstream prompt text is found.",
                "",
                "## Task Instruction",
                "",
                task_instruction,
                "",
                "## Skill Under Reduction",
                "",
                skill[:5000],
                "",
            ]
        ),
    )

    return rendered


def main() -> int:
    manifest_path = SAMPLE_ROOT / "manifest.json"
    if not manifest_path.exists():
        raise SystemExit(f"missing sample manifest: {manifest_path}")
    manifest = json.loads(manifest_path.read_text())

    lanes = [
        make_lane(
            manifest,
            "evoskill",
            "EvoSkill",
            [
                "tmp/repros/evoskill/examples/officeqa/.evoskill/config.toml",
                "tmp/repros/evoskill/examples/officeqa/data/officeqa_sample.csv",
                "tmp/repros/evoskill/src/agent_profiles/proposer/prompt.py",
                "tmp/repros/evoskill/src/agent_profiles/skill_proposer/prompt.py",
                "tmp/repros/evoskill/src/agent_profiles/skill_generator/prompt.py",
                "tmp/repros/evoskill/src/agent_profiles/sealqa_agent/prompt.txt",
                "tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/proposer_placeholder.md",
                "tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/skill_builder_placeholder.md",
                "tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/auto_grader_placeholder.md",
            ],
            "ready_for_one_sample_harness_adaptation",
            {
                "dataset": "real public OfficeQA and SealQA samples are present",
                "prompts": "upstream prompt files and paper appendix prompt placeholders are copied; exact runtime prompt assembly still needs a harness adapter",
                "model": "Codex/gpt-5.4-mini remains the approved substitution",
                "environment": "OfficeQA source text for UID0001 is present; full scorer/corpus is deferred",
            },
            "Adapt p5_evoskill_iteration to consume the OfficeQA/SealQA sample paths instead of its synthetic Treasury notation fixture.",
        ),
        make_lane(
            manifest,
            "trace2skill",
            "Trace2Skill",
            [
                "tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_skill_preloaded_full_system_v1.txt",
                "tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_only_full_system_v1.txt",
                "tmp/repros/trace2skill-upstream/skill_evolver/prompts/skill_evolving_agent/system_prompt_base.txt",
                "tmp/repros/trace2skill-upstream/skill_evolver/prompts/skill_evolving_agent/error_analysis_records_header.txt",
                "tmp/repros/trace2skill-upstream/skill_evolver/prompts/success_evolving_agent/success_merge_system_prompt.txt",
                "tmp/repros/trace2skill-upstream/released_skills/trace2skill-xlsx-35B-combined/SKILL.md",
            ],
            "ready_for_one_sample_live_attempt",
            {
                "dataset": "real SpreadsheetBench Verified case 13-1 is present with init/golden xlsx and prompt.txt",
                "prompts": "upstream spreadsheet agent system prompts and skill-evolver prompt fragments are copied",
                "model": "Codex/gpt-5.4-mini substitutes for released Qwen-family runs",
                "environment": "xlsx files are present; a cheap live attempt can run without full 400-case sweep",
            },
            "Build a one-case Trace2Skill runner around case 13-1 and upstream prompt fragments.",
        ),
        make_lane(
            manifest,
            "memento-skills",
            "Memento-Skills",
            [
                "tmp/skill_opt_sources/arx_2603.18743/full_source.md",
                "examples/memento_skills_read_write/scripts/run_tiny_live.sh",
                "examples/memento_skills_read_write/README.md",
            ],
            "blocked_on_gated_benchmark_access",
            {
                "dataset": "GAIA and HLE are gated in current Hugging Face auth state; access_blocked reports are present",
                "prompts": "paper/tiny-run prompt shape is available, but exact upstream Memento prompt text is not yet pinned",
                "model": "Codex/gpt-5.4-mini would be the approved substitution after data access",
                "environment": "do not substitute non-GAIA/HLE data and call it exact",
            },
            "Ask for HF access/token acceptance or approve a non-exact public surrogate lane.",
        ),
        make_lane(
            manifest,
            "d2skill",
            "D2Skill",
            [
                "tmp/repros/d2skill-agenticrl/examples_d2skill/run_alfworld_d2skill.sh",
                "tmp/repros/d2skill-agenticrl/examples_d2skill/run_webshop_d2skill.sh",
                "tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/alfworld.py",
                "tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/webshop.py",
                "tmp/repros/d2skill-agenticrl/agent_system/memory/README.md",
            ],
            "ready_for_one_sample_env_prompt_preflight",
            {
                "dataset": "real ALFWorld train trajectory JSON is present from official release zip",
                "prompts": "D2Skill ALFWorld/WebShop prompt source and run scripts are copied",
                "model": "Codex/gpt-5.4-mini substitutes for Qwen models; full RL/vLLM training is out of scope without approval",
                "environment": "one train trajectory is present; full TextWorld/THOR runtime and detector install are deferred",
            },
            "Build a one-trajectory prompt/render preflight that formats ALFWorld observation/action state through the D2Skill prompt template.",
        ),
        make_lane(
            manifest,
            "skillreducer",
            "SkillReducer",
            [
                "tmp/repros/skillsbench/tasks/jax-computing-basics/task.toml",
                "tmp/repros/skillsbench/tasks/jax-computing-basics/instruction.md",
                "tmp/repros/skillsbench/tasks/jax-computing-basics/environment/problem.json",
                "tmp/repros/skillsbench/.agents/skills/skill-creator/SKILL.md",
                "examples/skillreducer_tiny/scripts/run_tiny_live.sh",
            ],
            "ready_for_one_sample_skill_reduction_attempt",
            {
                "dataset": "real SkillsBench task plus real skill file are present",
                "prompts": "SkillReducer exact prompt text is not exposed in local paper source; current lane can use paper-faithful local live prompts only until upstream prompts are found",
                "model": "Codex/gpt-5.4-mini remains the approved substitution",
                "environment": "task/verifier files are present; full 600-skill sweep is deferred",
            },
            "Run the tiny SkillReducer flow on the copied real skill and SkillsBench task, labeling prompt non-exactness explicitly.",
        ),
    ]

    rendered_preflights = render_preflights()
    summary = {
        "sample_manifest": str(manifest_path.relative_to(REPO_ROOT)),
        "lane_root": str(LANE_ROOT.relative_to(REPO_ROOT)),
        "lanes": lanes,
        "rendered_preflights": rendered_preflights,
    }
    write_json(LANE_ROOT / "manifest.json", summary)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
