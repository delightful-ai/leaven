#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  bash examples/trace2skill_tiny_live/scripts/run_tiny_live.sh --preflight
  LEAVEN_CODEX_LIVE=1 bash examples/trace2skill_tiny_live/scripts/run_tiny_live.sh --live

Environment:
  LEAVEN_CODEX_BIN          Override Codex binary. Defaults to codex on PATH, then ~/.bun/bin/codex.
  LEAVEN_TRACE2SKILL_OUT    Override output directory.
USAGE
}

mode="${1:---preflight}"
case "$mode" in
  --preflight|--live) ;;
  --help|-h) usage; exit 0 ;;
  *) usage >&2; exit 2 ;;
esac

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
example_dir="$(cd "$script_dir/.." && pwd)"
repo_root="$(cd "$example_dir/../.." && pwd)"
"$repo_root/scripts/ensure_leaven_papers_workspace.sh"

run_id="$(date -u +%Y%m%dT%H%M%SZ)"
out_dir="${LEAVEN_TRACE2SKILL_OUT:-$repo_root/tmp/trace2skill_tiny_live/$run_id}"
work_dir="$out_dir/workspace"
codex_bin="${LEAVEN_CODEX_BIN:-}"
if [[ -z "$codex_bin" ]]; then
  if command -v codex >/dev/null 2>&1; then
    codex_bin="$(command -v codex)"
  else
    codex_bin="$HOME/.bun/bin/codex"
  fi
fi

mkdir -p "$work_dir/skill_initial" "$work_dir/skill_evolved" "$work_dir/inputs" "$work_dir/expected" "$work_dir/outputs" "$work_dir/prompts" "$work_dir/output" "$out_dir"
cp "$example_dir/fixtures/initial_skill/SKILL.md" "$work_dir/skill_initial/SKILL.md"
cp "$example_dir/fixtures/initial_skill/SKILL.md" "$work_dir/skill_evolved/SKILL.md"
cp "$example_dir/fixtures/tasks/failure_input.csv" "$work_dir/inputs/failure_input.csv"
cp "$example_dir/fixtures/tasks/success_input.csv" "$work_dir/inputs/success_input.csv"
cp "$example_dir/fixtures/tasks/failure_expected.csv" "$work_dir/expected/failure_expected.csv"
cp "$example_dir/fixtures/tasks/success_expected.csv" "$work_dir/expected/success_expected.csv"
cp "$example_dir/fixtures/tasks/tasks.json" "$work_dir/tasks.json"

cat > "$out_dir/preflight.json" <<JSON
{
  "paper": "Trace2Skill",
  "proof_class": "$([[ "$mode" == "--live" ]] && echo tiny_live_trajectory_patch_consolidation_attempt || echo no_spend_preflight_only)",
  "trajectory_count": 2,
  "model": "gpt-5.4-mini",
  "codex_bin": "$codex_bin",
  "paper_loop": [
    "trajectory_generation_with_frozen_skill",
    "independent_error_analyst_patch",
    "independent_success_analyst_patch",
    "hierarchical_consolidation",
    "conflict_free_patch_application",
    "replay_failed_task_with_evolved_skill"
  ],
  "source_anchors": [
    "tmp/skill_opt_sources/arx_2603.25158/full_source.md:19",
    "tmp/skill_opt_sources/arx_2603.25158/full_source.md:107",
    "tmp/skill_opt_sources/arx_2603.25158/full_source.md:111",
    "tmp/skill_opt_sources/arx_2603.25158/full_source.md:984",
    "tmp/skill_opt_sources/arx_2603.25158/full_source.md:1045"
  ],
  "deviations": [
    "Codex/GPT-5.4-mini replaces Qwen3.5 models",
    "CSV files replace xlsx files while preserving spreadsheet edit failure mode",
    "serial analyst calls preserve independence but not parallel wall-clock execution",
    "one merge level replaces a deep hierarchical merge tree"
  ]
}
JSON

if [[ "$mode" == "--preflight" ]]; then
  printf 'wrote preflight: %s\n' "$out_dir/preflight.json"
  exit 0
fi

if [[ "${LEAVEN_CODEX_LIVE:-}" != "1" ]]; then
  echo "live Trace2Skill run requires LEAVEN_CODEX_LIVE=1" >&2
  exit 2
fi

if [[ ! -x "$codex_bin" ]]; then
  echo "Codex binary is not executable: $codex_bin" >&2
  exit 2
fi

run_codex() {
  local prompt_file="$1"
  local last_message="$2"
  local stdout_file="$3"
  local stderr_file="$4"
  (
    cd "$work_dir"
    "$codex_bin" exec \
      --json \
      --skip-git-repo-check \
      --model gpt-5.4-mini \
      --config 'model_reasoning_effort="low"' \
      --output-last-message "$last_message" \
      --dangerously-bypass-approvals-and-sandbox \
      - < "$prompt_file" > "$stdout_file" 2> "$stderr_file"
  )
}

cat > "$work_dir/prompts/trajectory_failure.md" <<'PROMPT'
You are Trace2Skill Stage 1 trajectory generator.

The frozen target skill is preloaded below. Use it as the skill available to the
agent. Do not edit the skill.

Skill:
`skill_initial/SKILL.md`

Task metadata:
`tasks.json` key `failure`

Input file:
`inputs/failure_input.csv`

Run the task exactly as an agent using the frozen skill would. Write the edited
CSV to `outputs/failure_output.csv`. Then write strict JSON to
`output/trajectory_failure.json`:
{
  "trajectory_id": "traj-failure",
  "task_id": "csv-range-delete-void",
  "skill_version": "initial",
  "steps": [{"action":"...", "observation":"...", "why":"..."}],
  "final_output": "outputs/failure_output.csv"
}
PROMPT
run_codex "$work_dir/prompts/trajectory_failure.md" output/trajectory_failure_last_message.txt "$out_dir/trajectory_failure_stdout.jsonl" "$out_dir/trajectory_failure_stderr.txt"

cat > "$work_dir/prompts/trajectory_success.md" <<'PROMPT'
You are Trace2Skill Stage 1 trajectory generator.

The frozen target skill is preloaded below. Use it as the skill available to the
agent. Do not edit the skill.

Skill:
`skill_initial/SKILL.md`

Task metadata:
`tasks.json` key `success`

Input file:
`inputs/success_input.csv`

Run the task exactly as an agent using the frozen skill would. Write the edited
CSV to `outputs/success_output.csv`. Then write strict JSON to
`output/trajectory_success.json`:
{
  "trajectory_id": "traj-success",
  "task_id": "csv-global-delete-void",
  "skill_version": "initial",
  "steps": [{"action":"...", "observation":"...", "why":"..."}],
  "final_output": "outputs/success_output.csv"
}
PROMPT
run_codex "$work_dir/prompts/trajectory_success.md" output/trajectory_success_last_message.txt "$out_dir/trajectory_success_stdout.jsonl" "$out_dir/trajectory_success_stderr.txt"

python3 - "$work_dir" <<'PY'
import csv, json, pathlib, sys
root = pathlib.Path(sys.argv[1])
def rows(path):
    with (root / path).open(newline="") as handle:
        return list(csv.DictReader(handle))
def eval_task(name, expected, actual):
    exp = rows(expected)
    got = rows(actual) if (root / actual).exists() else []
    record = {
        "trajectory_id": f"traj-{name}",
        "expected": expected,
        "actual": actual,
        "success": got == exp,
        "expected_rows": exp,
        "actual_rows": got,
    }
    (root / f"output/eval_{name}.json").write_text(json.dumps(record, indent=2))
    return record
failure = eval_task("failure", "expected/failure_expected.csv", "outputs/failure_output.csv")
success = eval_task("success", "expected/success_expected.csv", "outputs/success_output.csv")
(root / "output/stage1_trajectory_summary.json").write_text(json.dumps({
    "failure_success": failure["success"],
    "success_success": success["success"],
    "labeled_trajectories": [
        {"id": "traj-failure", "label": "failure"},
        {"id": "traj-success", "label": "success"}
    ]
}, indent=2))
PY

cat > "$work_dir/prompts/error_analyst.md" <<'PROMPT'
You are Trace2Skill Stage 2 error analyst A^-.

You receive one failed trajectory and a frozen copy of the original skill. You
may inspect the full trace, input/output files, expected output, evaluator
record, and the frozen skill. Do not inspect any other analyst patch because
analysts are independent.

Files:
- frozen skill: `skill_initial/SKILL.md`
- task metadata: `tasks.json` key `failure`
- input: `inputs/failure_input.csv`
- actual output: `outputs/failure_output.csv`
- expected output: `expected/failure_expected.csv`
- trajectory: `output/trajectory_failure.json`
- evaluation: `output/eval_failure.json`

Diagnose the earliest causal failure and propose a generalizable skill patch.
The patch must not be a one-row benchmark fix; it should apply to future range-
bounded spreadsheet row deletion tasks.

Write strict JSON to `output/error_patch.json`:
{
  "patch_id": "error-range-boundary",
  "source_trajectory": "traj-failure",
  "causal_diagnosis": "...",
  "operations": [
    {
      "file": "SKILL.md",
      "operation": "append_section",
      "section_title": "...",
      "content": "...",
      "justification": "..."
    }
  ]
}
PROMPT
run_codex "$work_dir/prompts/error_analyst.md" output/error_analyst_last_message.txt "$out_dir/error_analyst_stdout.jsonl" "$out_dir/error_analyst_stderr.txt"

cat > "$work_dir/prompts/success_analyst.md" <<'PROMPT'
You are Trace2Skill Stage 2 success analyst A^+.

You receive one successful trajectory and a frozen copy of the original skill.
This is a single-pass analysis. Do not inspect any other analyst patch because
analysts are independent.

Files:
- frozen skill: `skill_initial/SKILL.md`
- task metadata: `tasks.json` key `success`
- input: `inputs/success_input.csv`
- actual output: `outputs/success_output.csv`
- expected output: `expected/success_expected.csv`
- trajectory: `output/trajectory_success.json`
- evaluation: `output/eval_success.json`

Identify generalizable behavior patterns that contributed to the correct
answer. Propose a concise patch only if it reinforces useful behavior without
conflicting with range-boundary safety.

Write strict JSON to `output/success_patch.json`:
{
  "patch_id": "success-readback",
  "source_trajectory": "traj-success",
  "success_pattern": "...",
  "operations": [
    {
      "file": "SKILL.md",
      "operation": "append_section",
      "section_title": "...",
      "content": "...",
      "justification": "..."
    }
  ]
}
PROMPT
run_codex "$work_dir/prompts/success_analyst.md" output/success_analyst_last_message.txt "$out_dir/success_analyst_stdout.jsonl" "$out_dir/success_analyst_stderr.txt"

cat > "$work_dir/prompts/consolidate.md" <<'PROMPT'
You are Trace2Skill Stage 3 skill edit coordinator.

You receive multiple independently proposed patches for the frozen skill. Merge
them into one coherent, non-redundant patch. Follow these rules:

1. Deduplicate similar edits.
2. Resolve conflicts by preserving range-boundary safety.
3. Preserve unique insights from different trajectories.
4. Keep the merged patch no longer than necessary.
5. Ensure operations are line-level independent; no two operations may target
   the same passage.
6. Prefer prevalent/general principles over instance-specific facts.

Files:
- frozen skill: `skill_initial/SKILL.md`
- error patch: `output/error_patch.json`
- success patch: `output/success_patch.json`

The final patch must include a rule equivalent to:
"When a task names a target row range or answer range, apply row deletion only
inside that range and preserve matching rows outside it. Verify retained
row_ids after writing."

Write strict JSON to `output/consolidated_patch.json`:
{
  "reasoning": "...",
  "operations": [
    {
      "file": "SKILL.md",
      "operation": "append_section",
      "section_title": "...",
      "content": "...",
      "supporting_patch_ids": ["..."]
    }
  ]
}
PROMPT
run_codex "$work_dir/prompts/consolidate.md" output/consolidate_last_message.txt "$out_dir/consolidate_stdout.jsonl" "$out_dir/consolidate_stderr.txt"

python3 - "$work_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
patch = json.loads((root / "output/consolidated_patch.json").read_text())
skill_path = root / "skill_evolved/SKILL.md"
original = skill_path.read_text()
seen_titles = set()
guardrail = {
    "pass": True,
    "checked": ["existing file targets", "append-only operations", "duplicate section titles"],
    "rejected_operations": [],
    "applied_operations": [],
}
updated = original
for index, op in enumerate(patch.get("operations", [])):
    reason = None
    if op.get("file") != "SKILL.md":
        reason = "target file does not exist or is not SKILL.md"
    elif op.get("operation") != "append_section":
        reason = "only append_section is supported in this tiny guardrail"
    elif op.get("section_title") in seen_titles or f"\n## {op.get('section_title')}\n" in updated:
        reason = "duplicate section title"
    if reason:
        guardrail["pass"] = False
        guardrail["rejected_operations"].append({"index": index, "reason": reason, "operation": op})
        continue
    seen_titles.add(op["section_title"])
    section = f"\n## {op['section_title']}\n\n{op['content'].strip()}\n"
    updated += section
    guardrail["applied_operations"].append({"index": index, "section_title": op["section_title"]})
skill_path.write_text(updated)
(root / "output/patch_guardrail.json").write_text(json.dumps(guardrail, indent=2))
PY

cat > "$work_dir/prompts/replay_failure_evolved.md" <<'PROMPT'
You are Trace2Skill Stage 3 replay after consolidated patch application.

Use the evolved skill at `skill_evolved/SKILL.md`. Run the original failed task
again.

Task metadata:
`tasks.json` key `failure`

Input file:
`inputs/failure_input.csv`

Write the edited CSV to `outputs/failure_output_evolved.csv`. Then write strict
JSON to `output/trajectory_failure_evolved.json`:
{
  "trajectory_id": "traj-failure-evolved",
  "task_id": "csv-range-delete-void",
  "skill_version": "evolved",
  "steps": [{"action":"...", "observation":"...", "why":"..."}],
  "final_output": "outputs/failure_output_evolved.csv"
}
PROMPT
run_codex "$work_dir/prompts/replay_failure_evolved.md" output/replay_failure_evolved_last_message.txt "$out_dir/replay_failure_evolved_stdout.jsonl" "$out_dir/replay_failure_evolved_stderr.txt"

python3 - "$work_dir" <<'PY'
import csv, json, pathlib, sys
root = pathlib.Path(sys.argv[1])
def rows(path):
    with (root / path).open(newline="") as handle:
        return list(csv.DictReader(handle))
expected = rows("expected/failure_expected.csv")
actual = rows("outputs/failure_output_evolved.csv") if (root / "outputs/failure_output_evolved.csv").exists() else []
record = {
    "trajectory_id": "traj-failure-evolved",
    "expected": "expected/failure_expected.csv",
    "actual": "outputs/failure_output_evolved.csv",
    "success": actual == expected,
    "expected_rows": expected,
    "actual_rows": actual,
}
(root / "output/eval_failure_evolved.json").write_text(json.dumps(record, indent=2))
PY

python3 - "$work_dir" "$out_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
out = pathlib.Path(sys.argv[2])
def load(path):
    return json.loads((root / path).read_text())
report = {
    "paper": "Trace2Skill",
    "proof_class": "tiny_live_trajectory_patch_consolidation_attempt",
    "model": "gpt-5.4-mini",
    "loop": {
        "stage1": load("output/stage1_trajectory_summary.json"),
        "error_patch": load("output/error_patch.json")["patch_id"],
        "success_patch": load("output/success_patch.json")["patch_id"],
        "consolidated_operations": load("output/consolidated_patch.json")["operations"],
        "guardrail": load("output/patch_guardrail.json"),
        "evolved_replay": load("output/eval_failure_evolved.json"),
    },
    "artifacts": [
        "preflight.json",
        "trajectory_failure_stdout.jsonl",
        "trajectory_success_stdout.jsonl",
        "error_analyst_stdout.jsonl",
        "success_analyst_stdout.jsonl",
        "consolidate_stdout.jsonl",
        "replay_failure_evolved_stdout.jsonl",
        "workspace/output/trajectory_failure.json",
        "workspace/output/trajectory_success.json",
        "workspace/output/eval_failure.json",
        "workspace/output/eval_success.json",
        "workspace/output/error_patch.json",
        "workspace/output/success_patch.json",
        "workspace/output/consolidated_patch.json",
        "workspace/output/patch_guardrail.json",
        "workspace/skill_evolved/SKILL.md",
        "workspace/output/trajectory_failure_evolved.json",
        "workspace/output/eval_failure_evolved.json"
    ],
    "deferred_full_replication": [
        "SpreadsheetBench-Verified xlsx execution",
        "128-way analyst parallelism",
        "multi-level hierarchical merge tree",
        "programmatic JSON patch to unified diff lowering",
        "WikiTQ/math/VQA transfer evaluation",
        "cross-model author/user experiments and score tables"
    ],
}
(out / "report.json").write_text(json.dumps(report, indent=2))
PY

printf 'wrote live report: %s\n' "$out_dir/report.json"
