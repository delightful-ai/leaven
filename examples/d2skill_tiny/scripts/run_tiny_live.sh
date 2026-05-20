#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  bash examples/d2skill_tiny/scripts/run_tiny_live.sh --preflight
  LEAVEN_CODEX_LIVE=1 bash examples/d2skill_tiny/scripts/run_tiny_live.sh --live

Environment:
  LEAVEN_CODEX_BIN    Override Codex binary. Defaults to codex on PATH, then ~/.bun/bin/codex.
  LEAVEN_D2SKILL_OUT  Override output directory.
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
out_dir="${LEAVEN_D2SKILL_OUT:-$repo_root/tmp/d2skill_tiny/$run_id}"
work_dir="$out_dir/workspace"
codex_bin="${LEAVEN_CODEX_BIN:-}"
if [[ -z "$codex_bin" ]]; then
  if command -v codex >/dev/null 2>&1; then
    codex_bin="$(command -v codex)"
  else
    codex_bin="$HOME/.bun/bin/codex"
  fi
fi

mkdir -p "$work_dir/output" "$work_dir/prompts" "$work_dir/skill_bank/task" "$work_dir/skill_bank/step" "$out_dir"
cp "$example_dir/fixtures/task.json" "$work_dir/task.json"
cp "$example_dir/fixtures/seed_bank.json" "$work_dir/skill_bank.json"

cat > "$out_dir/preflight.json" <<JSON
{
  "paper": "D2Skill",
  "proof_class": "$([[ "$mode" == "--live" ]] && echo tiny_live_dual_granularity_attempt || echo no_spend_preflight_only)",
  "task_count": 1,
  "model": "gpt-5.4-mini",
  "codex_bin": "$codex_bin",
  "paper_loop": [
    "paired_baseline_rollout",
    "skill_injected_rollout",
    "hindsight_utility_gap",
    "reflection_task_and_step_skill",
    "next_iteration_retrieval",
    "utility_update",
    "capacity_pruning"
  ],
  "source_anchors": [
    "tmp/skill_opt_sources/arx_2603.28716/full_source.md:104",
    "tmp/skill_opt_sources/arx_2603.28716/full_source.md:119",
    "tmp/skill_opt_sources/arx_2603.28716/full_source.md:183",
    "tmp/skill_opt_sources/arx_2603.28716/full_source.md:196",
    "tmp/skill_opt_sources/arx_2603.28716/full_source.md:212"
  ],
  "deviations": [
    "Codex/GPT-5.4-mini replaces Qwen policy and Gemini/O3 reflector roles",
    "tiny textual environment replaces ALFWorld/WebShop",
    "logged hindsight return replaces real GRPO parameter update",
    "lexical key similarity replaces embedding cosine similarity"
  ]
}
JSON

if [[ "$mode" == "--preflight" ]]; then
  printf 'wrote preflight: %s\n' "$out_dir/preflight.json"
  exit 0
fi

if [[ "${LEAVEN_CODEX_LIVE:-}" != "1" ]]; then
  echo "live D2Skill run requires LEAVEN_CODEX_LIVE=1" >&2
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

python3 - "$work_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
bank = json.loads((root / "skill_bank.json").read_text())
for pool in ("task_skills", "step_skills"):
    for skill in bank[pool]:
        directory = root / "skill_bank" / ("task" if pool == "task_skills" else "step")
        (directory / f"{skill['id']}.md").write_text(f"# {skill['id']}\n\nKey: {skill['key']}\nUtility: {skill['utility']}\n\n{skill['body']}\n")
PY

cat > "$work_dir/prompts/baseline_iteration0.md" <<'PROMPT'
You are the D2Skill baseline group policy rollout for iteration 0.

Do not use any persistent skill-bank guidance. Use only `task.json` and the
visible observation. If the local station policy is not visible, follow the
ordinary MiniShop return flow.

Write strict JSON to `output/baseline_iteration0.json`:
{
  "trajectory_id": "base-iter0",
  "group": "baseline",
  "steps": [{"observation":"...", "action":"...", "why":"..."}],
  "final_action": "..."
}
PROMPT
run_codex "$work_dir/prompts/baseline_iteration0.md" output/baseline_iteration0_last_message.txt "$out_dir/baseline_iteration0_stdout.jsonl" "$out_dir/baseline_iteration0_stderr.txt"

cat > "$work_dir/prompts/skill_iteration0.md" <<'PROMPT'
You are the D2Skill skill-injected group policy rollout for iteration 0.

Read `task.json`, `skill_bank/task/task-standard-returns.md`, and
`skill_bank/step/step-decorative-stripes.md`. These are the current retrieved
task and step skills from the bank. Use the retrieved skills as persistent
guidance when choosing from the admissible actions.

Write strict JSON to `output/skill_iteration0.json`:
{
  "trajectory_id": "skill-iter0",
  "group": "skill",
  "retrieved_skills": ["task-standard-returns", "step-decorative-stripes"],
  "steps": [{"observation":"...", "action":"...", "why":"..."}],
  "final_action": "..."
}
PROMPT
run_codex "$work_dir/prompts/skill_iteration0.md" output/skill_iteration0_last_message.txt "$out_dir/skill_iteration0_stdout.jsonl" "$out_dir/skill_iteration0_stderr.txt"

python3 - "$work_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
task = json.loads((root / "task.json").read_text())
expected = task["expected_action"]
def eval_one(name):
    obj = json.loads((root / f"output/{name}.json").read_text())
    success = obj.get("final_action") == expected
    record = {
        "trajectory_id": obj.get("trajectory_id", name),
        "group": obj.get("group"),
        "final_action": obj.get("final_action"),
        "expected_action": expected,
        "success": success,
        "reward": 1.0 if success else 0.0,
        "earliest_failure_step": None if success else task["earliest_failure_step"],
        "hidden_policy_reason": task["hidden_policy_reason"],
    }
    (root / f"output/{name}_eval.json").write_text(json.dumps(record, indent=2))
    return record
base = eval_one("baseline_iteration0")
skill = eval_one("skill_iteration0")
gap = skill["reward"] - base["reward"]
(root / "output/iteration0_group_metrics.json").write_text(json.dumps({
    "baseline_success_rate": base["reward"],
    "skill_success_rate": skill["reward"],
    "delta_task": gap,
    "reflection_triggered": skill["reward"] < 0.5,
    "tau_ref": 0.5,
}, indent=2))
PY

cat > "$work_dir/prompts/reflect_skills.md" <<'PROMPT'
You are the D2Skill external reflector model.

Reflection is triggered because the skill-injected group performed below
tau_ref. Read:
- `task.json`
- `output/baseline_iteration0.json`
- `output/baseline_iteration0_eval.json`
- `output/skill_iteration0.json`
- `output/skill_iteration0_eval.json`
- the current skill bank files under `skill_bank/`

Generate at most one task skill and at most one step skill. The task skill
should provide high-level guidance for MiniShop returns. The step skill should
correct the earliest failure step. Each skill must include a retrieval key.

Write strict JSON to `output/reflection_skills.json`:
{
  "task_skill": {
    "id": "task-...",
    "key": "minishop_returns",
    "body": "..."
  },
  "step_skill": {
    "id": "step-...",
    "key": "minishop_returns teal stripe",
    "body": "..."
  },
  "diagnosis": "..."
}
PROMPT
run_codex "$work_dir/prompts/reflect_skills.md" output/reflect_skills_last_message.txt "$out_dir/reflect_skills_stdout.jsonl" "$out_dir/reflect_skills_stderr.txt"

python3 - "$work_dir" <<'PY'
import json, pathlib, re, sys
root = pathlib.Path(sys.argv[1])
bank = json.loads((root / "skill_bank.json").read_text())
reflection = json.loads((root / "output/reflection_skills.json").read_text())

def clean_id(value, prefix):
    raw = re.sub(r"[^a-z0-9-]+", "-", value.lower()).strip("-")
    return raw if raw.startswith(prefix) else f"{prefix}-{raw}"

for source_key, pool, granularity, prefix in [
    ("task_skill", "task_skills", "task", "task"),
    ("step_skill", "step_skills", "step", "step"),
]:
    skill = reflection[source_key]
    skill_id = clean_id(skill["id"], prefix)
    record = {
        "id": skill_id,
        "granularity": granularity,
        "key": skill["key"],
        "utility": 0.0,
        "retrieval_count": 0,
        "created_iteration": 1,
        "body": skill["body"],
    }
    if not any(existing["id"] == skill_id for existing in bank[pool]):
        bank[pool].append(record)
    directory = root / "skill_bank" / granularity
    directory.mkdir(parents=True, exist_ok=True)
    (directory / f"{skill_id}.md").write_text(f"# {skill_id}\n\nKey: {record['key']}\nUtility: {record['utility']}\n\n{record['body']}\n")

(root / "skill_bank_after_reflection.json").write_text(json.dumps(bank, indent=2))
PY

python3 - "$work_dir" <<'PY'
import json, math, pathlib, sys
root = pathlib.Path(sys.argv[1])
task = json.loads((root / "task.json").read_text())
bank = json.loads((root / "skill_bank_after_reflection.json").read_text())
query_task = task["task_group"]
query_step = f"{task['task_group']} teal stripe"

def similarity(query, key):
    q = set(query.replace("_", " ").lower().split())
    k = set(key.replace("_", " ").lower().split())
    if not q or not k:
        return 0.0
    return len(q & k) / len(q | k)

def rank(pool_name, query, top_m=2, top_k=1):
    pool = bank[pool_name]
    total_retrievals = sum(item["retrieval_count"] for item in pool)
    candidates = []
    for item in pool:
        sim = similarity(query, item["key"])
        if sim <= 0:
            continue
        bonus = math.sqrt(math.log(total_retrievals + 2) / (item["retrieval_count"] + 1))
        score = sim + item["utility"] + 0.25 * bonus
        candidates.append({**item, "similarity": sim, "selection_score": score})
    candidates.sort(key=lambda item: item["selection_score"], reverse=True)
    return candidates[:top_m], candidates[:top_k]

task_candidates, task_selected = rank("task_skills", query_task)
step_candidates, step_selected = rank("step_skills", query_step)
for item in task_selected:
    item["retrieval_count"] += 1
for item in step_selected:
    item["retrieval_count"] += 1
selected_ids = {item["id"] for item in task_selected + step_selected}
for pool in ("task_skills", "step_skills"):
    for item in bank[pool]:
        if item["id"] in selected_ids:
            item["retrieval_count"] += 1
(root / "skill_bank_after_retrieval.json").write_text(json.dumps(bank, indent=2))
(root / "output/iteration1_retrieval.json").write_text(json.dumps({
    "task_query": query_task,
    "step_query": query_step,
    "task_candidates": task_candidates,
    "step_candidates": step_candidates,
    "selected_task_skills": [item["id"] for item in task_selected],
    "selected_step_skills": [item["id"] for item in step_selected],
}, indent=2))
for item in task_selected:
    (root / "output/retrieved_task_skill.md").write_text(f"# {item['id']}\n\n{item['body']}\n")
for item in step_selected:
    (root / "output/retrieved_step_skill.md").write_text(f"# {item['id']}\n\n{item['body']}\n")
PY

cat > "$work_dir/prompts/baseline_iteration1.md" <<'PROMPT'
You are the D2Skill baseline group policy rollout for iteration 1.

Do not use persistent skill-bank guidance. Use only `task.json` and the visible
observation. If the local station policy is not visible, follow the ordinary
MiniShop return flow.

Write strict JSON to `output/baseline_iteration1.json`:
{
  "trajectory_id": "base-iter1",
  "group": "baseline",
  "steps": [{"observation":"...", "action":"...", "why":"..."}],
  "final_action": "..."
}
PROMPT
run_codex "$work_dir/prompts/baseline_iteration1.md" output/baseline_iteration1_last_message.txt "$out_dir/baseline_iteration1_stdout.jsonl" "$out_dir/baseline_iteration1_stderr.txt"

cat > "$work_dir/prompts/skill_iteration1.md" <<'PROMPT'
You are the D2Skill skill-injected group policy rollout for iteration 1.

Read `task.json`, `output/iteration1_retrieval.json`,
`output/retrieved_task_skill.md`, and `output/retrieved_step_skill.md`. These
are the top-k task and step skills selected by D2Skill's retrieval stage. Use
them as persistent guidance when choosing from the admissible actions.

Write strict JSON to `output/skill_iteration1.json`:
{
  "trajectory_id": "skill-iter1",
  "group": "skill",
  "retrieved_skills": [],
  "steps": [{"observation":"...", "action":"...", "why":"..."}],
  "final_action": "..."
}
PROMPT
run_codex "$work_dir/prompts/skill_iteration1.md" output/skill_iteration1_last_message.txt "$out_dir/skill_iteration1_stdout.jsonl" "$out_dir/skill_iteration1_stderr.txt"

python3 - "$work_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
task = json.loads((root / "task.json").read_text())
expected = task["expected_action"]
def eval_one(name):
    obj = json.loads((root / f"output/{name}.json").read_text())
    success = obj.get("final_action") == expected
    record = {
        "trajectory_id": obj.get("trajectory_id", name),
        "group": obj.get("group"),
        "final_action": obj.get("final_action"),
        "expected_action": expected,
        "success": success,
        "reward": 1.0 if success else 0.0,
        "earliest_failure_step": None if success else task["earliest_failure_step"],
    }
    (root / f"output/{name}_eval.json").write_text(json.dumps(record, indent=2))
    return record
base = eval_one("baseline_iteration1")
skill = eval_one("skill_iteration1")
retrieval = json.loads((root / "output/iteration1_retrieval.json").read_text())
bank = json.loads((root / "skill_bank_after_retrieval.json").read_text())
delta = skill["reward"] - base["reward"]
alpha = 0.5
selected = set(retrieval["selected_task_skills"] + retrieval["selected_step_skills"])
for pool in ("task_skills", "step_skills"):
    for item in bank[pool]:
        if item["id"] in selected:
            item["utility"] = (1 - alpha) * item["utility"] + alpha * delta
metrics = {
    "baseline_success_rate": base["reward"],
    "skill_success_rate": skill["reward"],
    "delta_task": delta,
    "intrinsic_reward": delta,
    "alpha": alpha,
    "selected_skills_updated": sorted(selected),
}
(root / "output/iteration1_group_metrics.json").write_text(json.dumps(metrics, indent=2))
(root / "skill_bank_after_utility.json").write_text(json.dumps(bank, indent=2))
PY

python3 - "$work_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
bank = json.loads((root / "skill_bank_after_utility.json").read_text())
capacity = {"task_skills": 1, "step_skills": 1}
pruned = {}
for pool, limit in capacity.items():
    ranked = sorted(
        bank[pool],
        key=lambda item: (item["utility"] + 0.05 * item["retrieval_count"], item["created_iteration"]),
        reverse=True,
    )
    kept = ranked[:limit]
    removed = ranked[limit:]
    bank[pool] = kept
    pruned[pool] = {
        "kept": [item["id"] for item in kept],
        "removed": [item["id"] for item in removed],
        "capacity": limit,
    }
(root / "skill_bank_after_prune.json").write_text(json.dumps(bank, indent=2))
(root / "output/pruning.json").write_text(json.dumps(pruned, indent=2))
PY

python3 - "$work_dir" "$out_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
out = pathlib.Path(sys.argv[2])
def load(path):
    return json.loads((root / path).read_text())
report = {
    "paper": "D2Skill",
    "proof_class": "tiny_live_dual_granularity_attempt",
    "model": "gpt-5.4-mini",
    "task_id": load("task.json")["task_id"],
    "loop": {
        "iteration0": load("output/iteration0_group_metrics.json"),
        "reflection_generated": {
            "task_skill": load("output/reflection_skills.json")["task_skill"]["id"],
            "step_skill": load("output/reflection_skills.json")["step_skill"]["id"],
        },
        "iteration1_retrieval": load("output/iteration1_retrieval.json"),
        "iteration1": load("output/iteration1_group_metrics.json"),
        "pruning": load("output/pruning.json"),
    },
    "artifacts": [
        "preflight.json",
        "baseline_iteration0_stdout.jsonl",
        "skill_iteration0_stdout.jsonl",
        "reflect_skills_stdout.jsonl",
        "baseline_iteration1_stdout.jsonl",
        "skill_iteration1_stdout.jsonl",
        "workspace/output/baseline_iteration0.json",
        "workspace/output/skill_iteration0.json",
        "workspace/output/iteration0_group_metrics.json",
        "workspace/output/reflection_skills.json",
        "workspace/skill_bank_after_reflection.json",
        "workspace/output/iteration1_retrieval.json",
        "workspace/output/baseline_iteration1.json",
        "workspace/output/skill_iteration1.json",
        "workspace/output/iteration1_group_metrics.json",
        "workspace/skill_bank_after_utility.json",
        "workspace/output/pruning.json",
        "workspace/skill_bank_after_prune.json"
    ],
    "deferred_full_replication": [
        "ALFWorld/WebShop environments and grouped rollouts",
        "embedding cosine retrieval",
        "GRPO policy parameter updates",
        "large task groups and validation curves",
        "teacher model comparison across Gemini/O3",
        "benchmark success-rate tables and ablations"
    ],
}
(out / "report.json").write_text(json.dumps(report, indent=2))
PY

printf 'wrote live report: %s\n' "$out_dir/report.json"
