#!/usr/bin/env python3
"""Run one-sample paper-exact lane preflights.

This is intentionally lightweight: it proves the prompt/setup/data plumbing for
one real case per paper without starting full-corpus, GPU, or RL workloads.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
import zipfile
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
LANE_ROOT = REPO_ROOT / "tmp" / "paper_exact_lanes"
SAMPLE_ROOT = REPO_ROOT / "tmp" / "paper_exact_samples"
RUN_ROOT = REPO_ROOT / "tmp" / "paper_exact_lane_runs"


def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def write_text(path: Path, data: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(data)


def rel(path: Path) -> str:
    return str(path.relative_to(REPO_ROOT))


def xlsx_sheet_names(path: Path) -> list[str]:
    with zipfile.ZipFile(path) as zf:
        workbook = zf.read("xl/workbook.xml").decode("utf-8", errors="replace")
    return re.findall(r'<sheet[^>]* name="([^"]+)"', workbook)


def xlsx_dimensions(path: Path) -> dict[str, str]:
    dims: dict[str, str] = {}
    with zipfile.ZipFile(path) as zf:
        workbook = zf.read("xl/workbook.xml").decode("utf-8", errors="replace")
        names = re.findall(r'<sheet[^>]* name="([^"]+)"[^>]* sheetId="([^"]+)"', workbook)
        for name, sheet_id in names:
            sheet_path = f"xl/worksheets/sheet{sheet_id}.xml"
            if sheet_path not in zf.namelist():
                continue
            sheet = zf.read(sheet_path).decode("utf-8", errors="replace")
            match = re.search(r'<dimension ref="([^"]+)"', sheet)
            if match:
                dims[name] = match.group(1)
    return dims


def run_codex(prompt_path: Path, output_dir: Path) -> dict[str, Any]:
    codex_bin = os.environ.get("LEAVEN_CODEX_BIN") or shutil.which("codex") or str(Path.home() / ".bun/bin/codex")
    if not Path(codex_bin).exists():
        return {"status": "skipped", "reason": f"codex binary not found: {codex_bin}"}
    if os.environ.get("LEAVEN_CODEX_LIVE") != "1":
        return {"status": "skipped", "reason": "set LEAVEN_CODEX_LIVE=1 to run live Codex calls"}
    last_message = output_dir / "codex_last_message.txt"
    stdout = output_dir / "codex_stdout.jsonl"
    stderr = output_dir / "codex_stderr.txt"
    cmd = [
        codex_bin,
        "exec",
        "--json",
        "--skip-git-repo-check",
        "--model",
        "gpt-5.4-mini",
        "--config",
        'model_reasoning_effort="low"',
        "--output-last-message",
        str(last_message),
        "--dangerously-bypass-approvals-and-sandbox",
        "-",
    ]
    with prompt_path.open("rb") as stdin, stdout.open("wb") as out, stderr.open("wb") as err:
        proc = subprocess.run(cmd, cwd=output_dir, stdin=stdin, stdout=out, stderr=err, check=False)
    return {
        "status": "completed" if proc.returncode == 0 else "failed",
        "returncode": proc.returncode,
        "last_message": rel(last_message),
        "stdout": rel(stdout),
        "stderr": rel(stderr),
    }


def trace2skill(out: Path, live: bool) -> dict[str, Any]:
    case = read_json(SAMPLE_ROOT / "trace2skill" / "spreadsheetbench_verified" / "dataset_first_case.json")
    sample_dir = SAMPLE_ROOT / "trace2skill" / "spreadsheetbench_verified" / "13-1"
    init_xlsx = sample_dir / "1_13-1_init.xlsx"
    golden_xlsx = sample_dir / "1_13-1_golden.xlsx"
    system_prompt = (
        REPO_ROOT
        / "tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_skill_preloaded_full_system_v1.txt"
    ).read_text()
    skill = (
        REPO_ROOT
        / "tmp/repros/trace2skill-upstream/released_skills/trace2skill-xlsx-35B-combined/SKILL.md"
    ).read_text()
    prompt = out / "trace2skill_prompt.md"
    write_text(
        prompt,
        "\n".join(
            [
                "# Trace2Skill One-Case Lane",
                "",
                "Use the exact upstream spreadsheet-agent system prompt, released xlsx skill,",
                "and SpreadsheetBench Verified case 13-1. Do not solve unrelated cases.",
                "",
                "## System Prompt",
                system_prompt,
                "",
                "## Released Skill",
                skill[:12000],
                "",
                "## Case",
                json.dumps(case, indent=2),
                "",
                "## Files",
                f"- init workbook: {rel(init_xlsx)}",
                f"- golden workbook: {rel(golden_xlsx)}",
                "",
                "Return strict JSON with keys: task_understanding, workbook_plan,",
                "skill_lessons, deviations. Do not modify files in this preflight.",
                "",
            ]
        ),
    )
    report = {
        "paper": "Trace2Skill",
        "status": "preflight_ready",
        "case_id": case["id"],
        "prompt": rel(prompt),
        "init_workbook": {"path": rel(init_xlsx), "sheets": xlsx_sheet_names(init_xlsx), "dimensions": xlsx_dimensions(init_xlsx)},
        "golden_workbook": {"path": rel(golden_xlsx), "sheets": xlsx_sheet_names(golden_xlsx), "dimensions": xlsx_dimensions(golden_xlsx)},
        "exactness": {
            "data": "real SpreadsheetBench Verified case and workbooks",
            "prompts": "exact upstream system prompt and released skill file used",
            "model": "Codex/gpt-5.4-mini if live mode is enabled",
            "loop": "preflight renders one-case trajectory/skill-analysis input; no full 400-case evolution tree",
        },
    }
    if live:
        report["live"] = run_codex(prompt, out)
    return report


def d2skill(out: Path, live: bool) -> dict[str, Any]:
    traj = read_json(SAMPLE_ROOT / "d2skill" / "alfworld" / "first_traj_data.json")
    anns = traj["turk_annotations"]["anns"][0]
    prompt_source = (
        REPO_ROOT / "tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/alfworld.py"
    ).read_text()
    template = prompt_source.split('ALFWORLD_TEMPLATE = """', 1)[1].split('"""', 1)[0].strip()
    actions = [
        step["discrete_action"]["action"] + "(" + ", ".join(step["discrete_action"].get("args", [])) + ")"
        for step in traj["plan"]["high_pddl"]
    ]
    rendered = template.format(
        task_description=anns["task_desc"],
        step_count=0,
        history_length=0,
        action_history="[]",
        current_step=1,
        current_observation="Initial ALFWorld trajectory state from official traj_data.json; runtime observation requires TextWorld/THOR environment startup.",
        admissible_actions=", ".join(actions),
    )
    prompt = out / "d2skill_alfworld_prompt.md"
    write_text(
        prompt,
        "\n".join(
            [
                "# D2Skill ALFWorld One-Trajectory Lane",
                "",
                rendered,
                "",
                "Return strict JSON with keys: task_goal, first_action_choice, skill_memory_update_candidate, deviations.",
                "",
            ]
        ),
    )
    report = {
        "paper": "D2Skill",
        "status": "preflight_ready",
        "case_id": traj["task_id"],
        "task_type": traj["task_type"],
        "prompt": rel(prompt),
        "reference_task_desc": anns["task_desc"],
        "reference_high_level_actions": actions,
        "exactness": {
            "data": "real official ALFWorld train trajectory",
            "prompts": "exact D2Skill ALFWorld prompt template rendered",
            "model": "Codex/gpt-5.4-mini if live mode is enabled",
            "loop": "one prompt/trajectory step only; no RL/vLLM training or live TextWorld environment",
        },
    }
    if live:
        report["live"] = run_codex(prompt, out)
    return report


def evoskill(out: Path, live: bool) -> dict[str, Any]:
    officeqa = read_json(SAMPLE_ROOT / "evoskill" / "officeqa" / "officeqa_pro_first_case.json")
    source_text = (SAMPLE_ROOT / "evoskill" / "officeqa" / officeqa["source_files"]).read_text()
    proposer = (REPO_ROOT / "tmp/repros/evoskill/src/agent_profiles/skill_proposer/prompt.py").read_text()
    builder = (REPO_ROOT / "tmp/repros/evoskill/src/agent_profiles/skill_generator/prompt.py").read_text()
    prompt = out / "evoskill_officeqa_prompt.md"
    write_text(
        prompt,
        "\n".join(
            [
                "# EvoSkill OfficeQA One-Case Lane",
                "",
                "This renders the real OfficeQA case against upstream EvoSkill prompt sources.",
                "The full loop adapter still needs to replace the existing synthetic P5 fixture.",
                "",
                "## Case",
                json.dumps(officeqa, indent=2),
                "",
                "## Source Text Excerpt",
                source_text[:8000],
                "",
                "## Skill Proposer Prompt Source Excerpt",
                proposer[:8000],
                "",
                "## Skill Generator Prompt Source Excerpt",
                builder[:4000],
                "",
                "Return strict JSON with keys: failure_hypothesis, proposed_skill, builder_plan, deviations.",
                "",
            ]
        ),
    )
    report = {
        "paper": "EvoSkill",
        "status": "preflight_ready",
        "case_id": officeqa["uid"],
        "prompt": rel(prompt),
        "source_file": rel(SAMPLE_ROOT / "evoskill" / "officeqa" / officeqa["source_files"]),
        "exactness": {
            "data": "real OfficeQA Pro row and source text",
            "prompts": "upstream skill_proposer and skill_generator prompt sources rendered",
            "model": "Codex/gpt-5.4-mini if live mode is enabled",
            "loop": "one-case proposer/builder preflight; not yet full P5 frontier/admission loop on OfficeQA scorer",
        },
    }
    if live:
        report["live"] = run_codex(prompt, out)
    return report


def skillreducer(out: Path, live: bool) -> dict[str, Any]:
    task_root = SAMPLE_ROOT / "skillreducer" / "skillsbench" / "task"
    skill = (SAMPLE_ROOT / "skillreducer" / "skillsbench" / "skills" / "skill-creator" / "SKILL.md").read_text()
    instruction = (task_root / "instruction.md").read_text()
    problem = read_json(task_root / "environment" / "problem.json")
    prompt = out / "skillreducer_prompt.md"
    write_text(
        prompt,
        "\n".join(
            [
                "# SkillReducer SkillsBench One-Skill Lane",
                "",
                "Reduce the given skill for the given SkillsBench task while preserving only",
                "task-relevant behavior. Exact SkillReducer paper prompt text is not exposed",
                "in the local source, so this is a paper-faithful runner input, not a 1:1 prompt.",
                "",
                "## Task Instruction",
                instruction,
                "",
                "## problem.json",
                json.dumps(problem, indent=2),
                "",
                "## Skill",
                skill[:12000],
                "",
                "Return strict JSON with keys: keep_sections, remove_sections, reduced_skill_outline, deviations.",
                "",
            ]
        ),
    )
    report = {
        "paper": "SkillReducer",
        "status": "preflight_ready_prompt_non_exact",
        "case_id": "jax-computing-basics",
        "prompt": rel(prompt),
        "exactness": {
            "data": "real SkillsBench task and real skill fixture",
            "prompts": "prompt text is paper-faithful, not exact; upstream exact prompt not found locally",
            "model": "Codex/gpt-5.4-mini if live mode is enabled",
            "loop": "one-skill reduction input only; no 600-skill population sweep",
        },
    }
    if live:
        report["live"] = run_codex(prompt, out)
    return report


def memento(out: Path, live: bool) -> dict[str, Any]:
    gaia = read_json(SAMPLE_ROOT / "memento-skills" / "gaia" / "access_blocked.json")
    hle = read_json(SAMPLE_ROOT / "memento-skills" / "hle" / "access_blocked.json")
    report = {
        "paper": "Memento-Skills",
        "status": "blocked_on_gated_data",
        "gaia": gaia,
        "hle": hle,
        "exactness": {
            "data": "blocked; current HF auth cannot access GAIA or HLE",
            "prompts": "do not render a non-exact surrogate without approval",
            "model": "Codex/gpt-5.4-mini would be substitution after access",
            "loop": "no exact lane execution until benchmark data access is resolved",
        },
    }
    write_text(
        out / "memento_blocked.md",
        f"# Memento-Skills Blocked\n\nGAIA: {gaia['reason']}\n\nHLE: {hle['reason']}\n",
    )
    return report


LANES = {
    "trace2skill": trace2skill,
    "d2skill": d2skill,
    "evoskill": evoskill,
    "skillreducer": skillreducer,
    "memento-skills": memento,
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--lane", choices=[*LANES.keys(), "all"], default="all")
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--run-id", default=time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()))
    args = parser.parse_args()

    out = RUN_ROOT / args.run_id
    selected = list(LANES) if args.lane == "all" else [args.lane]
    reports = {}
    for lane in selected:
        lane_out = out / lane
        lane_out.mkdir(parents=True, exist_ok=True)
        report = LANES[lane](lane_out, args.live)
        write_json(lane_out / "report.json", report)
        reports[lane] = report

    manifest = {
        "run_id": args.run_id,
        "live_requested": args.live,
        "reports": {lane: rel(out / lane / "report.json") for lane in reports},
        "statuses": {lane: report["status"] for lane, report in reports.items()},
    }
    write_json(out / "manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
