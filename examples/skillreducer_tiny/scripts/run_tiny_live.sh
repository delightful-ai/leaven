#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  bash examples/skillreducer_tiny/scripts/run_tiny_live.sh --preflight
  LEAVEN_CODEX_LIVE=1 bash examples/skillreducer_tiny/scripts/run_tiny_live.sh --live

Environment:
  LEAVEN_CODEX_BIN          Override Codex binary. Defaults to codex on PATH, then ~/.bun/bin/codex.
  LEAVEN_SKILLREDUCER_OUT   Override output directory.
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
bash "$repo_root/scripts/ensure_leaven_workspace.sh"

run_id="$(date -u +%Y%m%dT%H%M%SZ)"
out_dir="${LEAVEN_SKILLREDUCER_OUT:-$repo_root/tmp/skillreducer_tiny/$run_id}"
work_dir="$out_dir/workspace"
codex_bin="${LEAVEN_CODEX_BIN:-}"
if [[ -z "$codex_bin" ]]; then
  if command -v codex >/dev/null 2>&1; then
    codex_bin="$(command -v codex)"
  else
    codex_bin="$HOME/.bun/bin/codex"
  fi
fi

mkdir -p "$work_dir/original_skill" "$work_dir/output" "$work_dir/prompts" "$out_dir"
cp "$example_dir/fixtures/skills/product-marketing-pmm/SKILL.md" "$work_dir/original_skill/SKILL.md"

cat > "$out_dir/preflight.json" <<JSON
{
  "paper": "SkillReducer",
  "proof_class": "$([[ "$mode" == "--live" ]] && echo tiny_live_debloating_attempt || echo no_spend_preflight_only)",
  "skill_count": 1,
  "task_count": 1,
  "model": "gpt-5.4-mini",
  "codex_bin": "$codex_bin",
  "paper_loop": [
    "stage1_description_candidates",
    "simulated_oracle",
    "real_trigger_validation",
    "stage2_taxonomy_classification",
    "faithfulness_gate",
    "condition_a_original_task",
    "condition_c_compressed_task",
    "feedback_promotion_if_regression"
  ],
  "source_anchors": [
    "tmp/skill_opt_sources/arx_2603.29919/full_source.md:108",
    "tmp/skill_opt_sources/arx_2603.29919/full_source.md:136",
    "tmp/skill_opt_sources/arx_2603.29919/full_source.md:170",
    "tmp/skill_opt_sources/arx_2603.29919/full_source.md:194"
  ],
  "deviations": [
    "Codex/GPT-5.4-mini replaces DeepSeek, Qwen, and Claude Code roles",
    "tiny candidate set replaces full ddmin over hundreds of skills",
    "Codex prompt validation replaces Claude Code stream event parsing",
    "one deterministic task replaces five generated Gate 2 tasks"
  ]
}
JSON

if [[ "$mode" == "--preflight" ]]; then
  printf 'wrote preflight: %s\n' "$out_dir/preflight.json"
  exit 0
fi

if [[ "${LEAVEN_CODEX_LIVE:-}" != "1" ]]; then
  echo "live SkillReducer run requires LEAVEN_CODEX_LIVE=1" >&2
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

cat > "$work_dir/description_candidates.json" <<'JSON'
[
  {
    "id": "full",
    "description": "Product marketing, positioning, GTM strategy, competitive intelligence, ICP definition, April Dunford methodology, product launch planning, messaging workshops, battlecards, market entry guides, sales enablement, persona messaging, category design, analyst relations, pricing narrative, segmentation, and when a user mentions product marketing, positioning, GTM, go-to-market, launch, PMM, competitive teardown, differentiated messaging, buyers, personas, sales deck, or battlecard."
  },
  {
    "id": "minus_trigger_list",
    "description": "Product marketing, positioning, GTM strategy, competitive intelligence, ICP definition, launch planning, messaging, battlecards, market entry guides, sales enablement, and segmentation."
  },
  {
    "id": "minimal",
    "description": "Product marketing, positioning, GTM strategy, competitive intelligence. Tools: ICP definition, launch playbooks, battlecards, market entry guides."
  }
]
JSON

cat > "$work_dir/prompts/stage1_oracle.md" <<'PROMPT'
You are the SkillReducer Stage 1 simulated routing oracle.

Read `description_candidates.json`. Evaluate each target description candidate
against this candidate pool:

- target skill name: product-marketing-pmm
- target body: `original_skill/SKILL.md`
- distractor: sales-call-coach - improves live sales discovery calls and objection handling.
- distractor: customer-success-qbr - prepares quarterly business reviews and renewal risk plans.
- distractor: brand-voice-editor - rewrites copy to match a brand tone guide.

First generate one adversarial shadow skill that is topically close to product
marketing but functionally distinct. Then, for each description candidate and
each query below, decide which skill should be selected. A candidate passes only
if every query selects product-marketing-pmm for semantic reasons, not position.

Queries:
1. "Help position a payroll API for CFO buyers and pick a launch channel."
2. "Create a battlecard angle against a legacy HR suite."

Select the shortest passing candidate. Write strict JSON to
`output/stage1_oracle.json`:
{
  "adversarial_skill": {"name":"...", "description":"..."},
  "candidate_results": [
    {"id":"...", "selected_for_all_queries":true, "why":"..."}
  ],
  "accepted_description_id":"...",
  "accepted_description":"..."
}
PROMPT
run_codex "$work_dir/prompts/stage1_oracle.md" output/stage1_last_message.txt "$out_dir/stage1_stdout.jsonl" "$out_dir/stage1_stderr.txt"

python3 - "$work_dir" <<'PY'
import json, pathlib, re, sys
root = pathlib.Path(sys.argv[1])
data = json.loads((root / "output/stage1_oracle.json").read_text())
desc = data["accepted_description"]
orig = (root / "original_skill/SKILL.md").read_text()
updated = re.sub(r"description: .*\n", "description: " + desc.replace("\n", " ") + "\n", orig, count=1)
skill_dir = root / "compressed_description_skill"
skill_dir.mkdir(exist_ok=True)
(skill_dir / "SKILL.md").write_text(updated)
PY

cat > "$work_dir/prompts/real_trigger.md" <<'PROMPT'
You are the SkillReducer Stage 1 real-environment validation pass.

The deployed compressed skill is at `compressed_description_skill/SKILL.md`.
Treat its frontmatter description as the only routing signal. For each query,
decide whether a real skill runtime should trigger this skill rather than no
skill or a nearby non-PMM skill.

Queries:
1. "Help position a payroll API for CFO buyers and pick a launch channel."
2. "Create a battlecard angle against a legacy HR suite."

Write strict JSON to `output/real_trigger.json`:
{
  "triggered_all_queries": true,
  "per_query": [{"query":"...", "triggered":true, "why":"..."}]
}
PROMPT
run_codex "$work_dir/prompts/real_trigger.md" output/real_trigger_last_message.txt "$out_dir/real_trigger_stdout.jsonl" "$out_dir/real_trigger_stderr.txt"

cat > "$work_dir/prompts/stage2_compress.md" <<'PROMPT'
You are the SkillReducer Stage 2 taxonomy classifier and body optimizer.

Read `compressed_description_skill/SKILL.md`. Segment the body into paragraph
items and classify each item as one of:
- core_rule
- background
- example
- template
- redundant

Create an optimized tiered skill under `output/optimized_skill/`:
- `SKILL.md` keeps the same name and compressed description.
- `SKILL.md` keeps only actionable core rules, compressed to be shorter than
  the original body.
- Move examples to `references/examples.md`.
- Move templates to `references/templates.md`.
- Move background to `references/background.md`.
- Add short "when" and "topics" metadata at the top of each reference file so
  an agent can decide whether to read it.

Do not remove the budget sanity check, buyer/pain/capability/proof/channel
method, or anti-patterns from the always-loaded core.

Write strict JSON to `output/body_classification.json`:
{
  "items": [{"id":"item-1", "type":"core_rule", "summary":"..."}],
  "optimized_skill_dir":"output/optimized_skill",
  "core_shorter_than_original": true
}
PROMPT
run_codex "$work_dir/prompts/stage2_compress.md" output/stage2_last_message.txt "$out_dir/stage2_stdout.jsonl" "$out_dir/stage2_stderr.txt"

cat > "$work_dir/prompts/gate1_faithfulness.md" <<'PROMPT'
You are SkillReducer Gate 1.

Compare the original body in `compressed_description_skill/SKILL.md` with the
optimized core and references under `output/optimized_skill/`.

Gate 1 passes only if every operational concept needed for product marketing
task execution is preserved either in the always-loaded core or in an annotated
reference module.

Write strict JSON to `output/gate1_faithfulness.json`:
{
  "pass": true,
  "missing_operational_concepts": [],
  "preserved_concepts": ["..."]
}
PROMPT
run_codex "$work_dir/prompts/gate1_faithfulness.md" output/gate1_last_message.txt "$out_dir/gate1_stdout.jsonl" "$out_dir/gate1_stderr.txt"

cat > "$work_dir/prompts/task_condition_a.md" <<'PROMPT'
You are SkillReducer Gate 2 Condition A.

Use the original skill in `compressed_description_skill/SKILL.md`. Complete the
task and write strict JSON to `output/task_a.json`.

Task:
Position a B2B payroll API for CFOs at 500 employee companies that overspend on
manual payroll exception handling. Recommend one launch channel. Include a
budget sanity check.

Required schema:
{
  "buyer": "...",
  "urgent_pain": "...",
  "differentiated_capability": "...",
  "proof_points": ["..."],
  "positioning_statement": "...",
  "launch_channel": "...",
  "budget_sanity_check": "..."
}
PROMPT
run_codex "$work_dir/prompts/task_condition_a.md" output/task_a_last_message.txt "$out_dir/task_a_stdout.jsonl" "$out_dir/task_a_stderr.txt"

cat > "$work_dir/prompts/task_condition_c.md" <<'PROMPT'
You are SkillReducer Gate 2 Condition C.

Use the compressed skill in `output/optimized_skill/SKILL.md`. You may inspect
on-demand references under `output/optimized_skill/references/` if their when
metadata is relevant. Complete the task and write strict JSON to
`output/task_c.json`.

Task:
Position a B2B payroll API for CFOs at 500 employee companies that overspend on
manual payroll exception handling. Recommend one launch channel. Include a
budget sanity check.

Required schema:
{
  "buyer": "...",
  "urgent_pain": "...",
  "differentiated_capability": "...",
  "proof_points": ["..."],
  "positioning_statement": "...",
  "launch_channel": "...",
  "budget_sanity_check": "..."
}
PROMPT
run_codex "$work_dir/prompts/task_condition_c.md" output/task_c_last_message.txt "$out_dir/task_c_stdout.jsonl" "$out_dir/task_c_stderr.txt"

python3 - "$work_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])

required = [
    "buyer",
    "urgent_pain",
    "differentiated_capability",
    "proof_points",
    "positioning_statement",
    "launch_channel",
    "budget_sanity_check",
]

def load(path):
    return json.loads((root / path).read_text())

def score(obj):
    checks = {}
    text = json.dumps(obj).lower()
    checks["schema_complete"] = all(k in obj and obj[k] for k in required)
    checks["cfo_buyer"] = "cfo" in text or "finance" in text
    checks["payroll_pain"] = "payroll" in text and ("manual" in text or "exception" in text)
    checks["budget_sanity_check"] = "budget" in str(obj.get("budget_sanity_check", "")).lower()
    checks["launch_channel"] = bool(obj.get("launch_channel"))
    return checks, sum(checks.values()) / len(checks)

a = load("output/task_a.json")
c = load("output/task_c.json")
a_checks, a_score = score(a)
c_checks, c_score = score(c)
retention = 1.0 if a_score == 0 else min(1.0, c_score / a_score)
gate = {
    "score_a": a_score,
    "score_c": c_score,
    "retention": retention,
    "pass": retention >= 1.0,
    "checks_a": a_checks,
    "checks_c": c_checks,
}
(root / "output/gate2_scores.json").write_text(json.dumps(gate, indent=2))
PY

feedback_applied=false
if python3 - "$work_dir" <<'PY'
import json, pathlib, sys
gate = json.loads((pathlib.Path(sys.argv[1]) / "output/gate2_scores.json").read_text())
raise SystemExit(0 if not gate["pass"] else 1)
PY
then
  feedback_applied=true
  cat > "$work_dir/prompts/feedback_promote.md" <<'PROMPT'
You are the SkillReducer Gate 2 feedback loop.

Condition C regressed against Condition A. Read:
- `output/gate2_scores.json`
- `compressed_description_skill/SKILL.md`
- `output/optimized_skill/SKILL.md`
- `output/optimized_skill/references/`

Identify which non-core original items are needed to satisfy the failed rubric
criteria. Promote only those items into the always-loaded core in
`output/optimized_skill/SKILL.md`, preserving their original wording when
possible. Write strict JSON to `output/feedback_promotion.json`:
{
  "promoted_items": [{"source":"...", "why":"..."}],
  "updated_core_file":"output/optimized_skill/SKILL.md"
}
PROMPT
  run_codex "$work_dir/prompts/feedback_promote.md" output/feedback_last_message.txt "$out_dir/feedback_stdout.jsonl" "$out_dir/feedback_stderr.txt"

  cat > "$work_dir/prompts/task_condition_c_retry.md" <<'PROMPT'
You are SkillReducer Gate 2 Condition C after feedback promotion.

Use the updated compressed skill in `output/optimized_skill/SKILL.md`. You may
inspect on-demand references under `output/optimized_skill/references/`.
Complete the same task and write strict JSON to `output/task_c_retry.json`.

Task:
Position a B2B payroll API for CFOs at 500 employee companies that overspend on
manual payroll exception handling. Recommend one launch channel. Include a
budget sanity check.

Required schema:
{
  "buyer": "...",
  "urgent_pain": "...",
  "differentiated_capability": "...",
  "proof_points": ["..."],
  "positioning_statement": "...",
  "launch_channel": "...",
  "budget_sanity_check": "..."
}
PROMPT
  run_codex "$work_dir/prompts/task_condition_c_retry.md" output/task_c_retry_last_message.txt "$out_dir/task_c_retry_stdout.jsonl" "$out_dir/task_c_retry_stderr.txt"

  python3 - "$work_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
required = ["buyer", "urgent_pain", "differentiated_capability", "proof_points", "positioning_statement", "launch_channel", "budget_sanity_check"]
def score(obj):
    text = json.dumps(obj).lower()
    checks = {
        "schema_complete": all(k in obj and obj[k] for k in required),
        "cfo_buyer": "cfo" in text or "finance" in text,
        "payroll_pain": "payroll" in text and ("manual" in text or "exception" in text),
        "budget_sanity_check": "budget" in str(obj.get("budget_sanity_check", "")).lower(),
        "launch_channel": bool(obj.get("launch_channel")),
    }
    return checks, sum(checks.values()) / len(checks)
a = json.loads((root / "output/task_a.json").read_text())
c = json.loads((root / "output/task_c_retry.json").read_text())
a_checks, a_score = score(a)
c_checks, c_score = score(c)
retention = 1.0 if a_score == 0 else min(1.0, c_score / a_score)
(root / "output/gate2_retry_scores.json").write_text(json.dumps({
    "score_a": a_score,
    "score_c_retry": c_score,
    "retention": retention,
    "pass": retention >= 1.0,
    "checks_a": a_checks,
    "checks_c_retry": c_checks,
}, indent=2))
PY
fi

python3 - "$work_dir" "$out_dir" "$feedback_applied" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
out = pathlib.Path(sys.argv[2])
feedback = sys.argv[3] == "true"
stage1 = json.loads((root / "output/stage1_oracle.json").read_text())
trigger = json.loads((root / "output/real_trigger.json").read_text())
classify = json.loads((root / "output/body_classification.json").read_text())
gate1 = json.loads((root / "output/gate1_faithfulness.json").read_text())
gate2 = json.loads((root / "output/gate2_scores.json").read_text())
final_gate2 = json.loads((root / ("output/gate2_retry_scores.json" if feedback else "output/gate2_scores.json")).read_text())
report = {
    "paper": "SkillReducer",
    "proof_class": "tiny_live_debloating_attempt",
    "model": "gpt-5.4-mini",
    "skill": "product-marketing-pmm",
    "loop": {
        "accepted_description_id": stage1["accepted_description_id"],
        "real_trigger_validation": trigger["triggered_all_queries"],
        "classified_items": len(classify["items"]),
        "core_shorter_than_original": classify["core_shorter_than_original"],
        "gate1_pass": gate1["pass"],
        "gate2_initial_pass": gate2["pass"],
        "feedback_applied": feedback,
        "gate2_final": final_gate2,
    },
    "artifacts": [
        "preflight.json",
        "stage1_stdout.jsonl",
        "real_trigger_stdout.jsonl",
        "stage2_stdout.jsonl",
        "gate1_stdout.jsonl",
        "task_a_stdout.jsonl",
        "task_c_stdout.jsonl",
        "workspace/output/stage1_oracle.json",
        "workspace/output/real_trigger.json",
        "workspace/output/body_classification.json",
        "workspace/output/optimized_skill/SKILL.md",
        "workspace/output/gate1_faithfulness.json",
        "workspace/output/task_a.json",
        "workspace/output/task_c.json",
        "workspace/output/gate2_scores.json",
    ],
    "deferred_full_replication": [
        "full ddmin over semantic clauses and large distractor pools",
        "Claude Code stream-event RealTrigger validation",
        "DeepSeek/Qwen role separation",
        "five generated Gate 2 tasks per skill",
        "600-skill evaluation, SkillsBench, wild-skill sampling, and statistical reporting",
        "tiktoken token accounting and cost curves",
    ],
}
if feedback:
    report["artifacts"].extend([
        "workspace/output/feedback_promotion.json",
        "workspace/output/task_c_retry.json",
        "workspace/output/gate2_retry_scores.json",
    ])
(out / "report.json").write_text(json.dumps(report, indent=2))
PY

printf 'wrote live report: %s\n' "$out_dir/report.json"
