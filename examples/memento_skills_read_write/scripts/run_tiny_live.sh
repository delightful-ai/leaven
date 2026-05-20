#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  bash examples/memento_skills_read_write/scripts/run_tiny_live.sh --preflight
  LEAVEN_CODEX_LIVE=1 bash examples/memento_skills_read_write/scripts/run_tiny_live.sh --live

Environment:
  LEAVEN_CODEX_BIN       Override Codex binary. Defaults to codex on PATH, then ~/.bun/bin/codex.
  LEAVEN_MEMENTO_OUT    Override output directory.
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
out_dir="${LEAVEN_MEMENTO_OUT:-$repo_root/tmp/memento_skills_read_write/$run_id}"
work_dir="$out_dir/workspace"
codex_bin="${LEAVEN_CODEX_BIN:-}"
if [[ -z "$codex_bin" ]]; then
  if command -v codex >/dev/null 2>&1; then
    codex_bin="$(command -v codex)"
  else
    codex_bin="$HOME/.bun/bin/codex"
  fi
fi

mkdir -p "$work_dir/.agents/skills" "$work_dir/output" "$work_dir/prompts" "$out_dir"
cp -R "$example_dir/fixtures/skills/house-checksum" "$work_dir/.agents/skills/"
cp "$example_dir/fixtures/train_cases.json" "$work_dir/train_cases.json"

cat > "$out_dir/preflight.json" <<JSON
{
  "paper": "Memento-Skills",
  "proof_class": "$([[ "$mode" == "--live" ]] && echo tiny_live_read_write_attempt || echo no_spend_preflight_only)",
  "case_count": 1,
  "model": "gpt-5.4-mini",
  "codex_bin": "$codex_bin",
  "paper_loop": ["observe", "read", "act", "feedback", "write", "retry"],
  "source_anchors": [
    "tmp/skill_opt_sources/arx_2603.18743/full_source.md:319",
    "tmp/skill_opt_sources/arx_2603.18743/all_text_sources.md:1182"
  ],
  "deviations": [
    "Codex/GPT-5.4-mini replaces Gemini-3.1-Flash",
    "description-only Codex selector replaces trained Memento-Qwen router",
    "one exact-match synthetic train case replaces GAIA/HLE",
    "tiny static unit gate replaces generated test suite"
  ]
}
JSON

if [[ "$mode" == "--preflight" ]]; then
  printf 'wrote preflight: %s\n' "$out_dir/preflight.json"
  exit 0
fi

if [[ "${LEAVEN_CODEX_LIVE:-}" != "1" ]]; then
  echo "live Memento-Skills run requires LEAVEN_CODEX_LIVE=1" >&2
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

cat > "$work_dir/prompts/read_router.md" <<'PROMPT'
You are the Memento-Skills Read step.

Observe the routing goal and select the one skill that should condition the
frozen agent's action. You may inspect only skill names and descriptions, not
full skill bodies.

Routing goal:
Use the house checksum protocol to encode batch alpha.

Available skills:
- name: house-checksum
  description: Encode small integer batches using the house checksum protocol.

Write JSON to output/read_selection.json:
{"selected_skill":"house-checksum","why":"..."}

Do not solve the task. This step only selects the skill.
PROMPT
run_codex "$work_dir/prompts/read_router.md" output/read_last_message.txt "$out_dir/read_stdout.jsonl" "$out_dir/read_stderr.txt"

cat > "$work_dir/prompts/act_before_write.md" <<'PROMPT'
You are the Memento-Skills Act step.

The Read step selected `.agents/skills/house-checksum/SKILL.md`. Use that
selected skill as the policy for this task. Do not silently correct the skill
from outside knowledge; this run is measuring the behaviour of the current
skill library.

Task:
Use the house checksum protocol to encode batch alpha: values 14, 19, and 27.
Return only the final checksum string.

Write JSON to output/act_before_write.json:
{"answer":"CHECK-...","reasoning":"..."}
PROMPT
run_codex "$work_dir/prompts/act_before_write.md" output/act_before_last_message.txt "$out_dir/act_before_stdout.jsonl" "$out_dir/act_before_stderr.txt"

answer_before="$(ruby -rjson -e 'print JSON.parse(File.read(ARGV[0])).fetch("answer")' "$work_dir/output/act_before_write.json")"
if [[ "$answer_before" == "CHECK-180" ]]; then
  feedback="correct"
else
  feedback="incorrect"
fi

cat > "$work_dir/output/feedback_before_write.json" <<JSON
{
  "case_id": "house-checksum-alpha",
  "selected_skill": "house-checksum",
  "answer": "$answer_before",
  "expected": "CHECK-180",
  "reward": "$feedback",
  "judge_feedback": "The selected skill caused the failure: the house checksum protocol multiplier is 3, not 2. Preserve the sum-then-format workflow and rewrite the skill so future executions return CHECK-180 for values 14, 19, and 27."
}
JSON

cat > "$work_dir/prompts/write_update.md" <<'PROMPT'
You are the Memento-Skills Write step.

Perform skill-level failure attribution and file-level rewriting. The failed
interaction selected `.agents/skills/house-checksum/SKILL.md`.

Read:
- output/act_before_write.json
- output/feedback_before_write.json
- .agents/skills/house-checksum/SKILL.md

If feedback is incorrect, rewrite `.agents/skills/house-checksum/SKILL.md` to
fix the attributed skill while preserving its generality. Do not write the
final task answer directly as a special case; update the reusable procedure.

Then write `output/write_update.json`:
{
  "target_skill": "house-checksum",
  "mutation": "rewrite" | "none",
  "attribution": "...",
  "unit_test_gate": "pending"
}
PROMPT
run_codex "$work_dir/prompts/write_update.md" output/write_last_message.txt "$out_dir/write_stdout.jsonl" "$out_dir/write_stderr.txt"

if rg -q 'multiply .* by 3|multiplier is 3|Multiply .* by 3' "$work_dir/.agents/skills/house-checksum/SKILL.md"; then
  gate="pass"
else
  gate="fail"
fi
printf '{"unit_test_gate":"%s"}\n' "$gate" > "$work_dir/output/unit_test_gate.json"
if [[ "$gate" != "pass" ]]; then
  echo "unit-test gate failed; see $work_dir/.agents/skills/house-checksum/SKILL.md" >&2
  exit 1
fi

cat > "$work_dir/prompts/act_after_write.md" <<'PROMPT'
You are the Memento-Skills feedback retry step.

Use the updated selected skill `.agents/skills/house-checksum/SKILL.md`.

Task:
Use the house checksum protocol to encode batch alpha: values 14, 19, and 27.
Return only the final checksum string.

Write JSON to output/act_after_write.json:
{"answer":"CHECK-...","reasoning":"..."}
PROMPT
run_codex "$work_dir/prompts/act_after_write.md" output/act_after_last_message.txt "$out_dir/act_after_stdout.jsonl" "$out_dir/act_after_stderr.txt"

answer_after="$(ruby -rjson -e 'print JSON.parse(File.read(ARGV[0])).fetch("answer")' "$work_dir/output/act_after_write.json")"
if [[ "$answer_after" == "CHECK-180" ]]; then
  final_reward="correct"
else
  final_reward="incorrect"
fi

cp "$work_dir/.agents/skills/house-checksum/SKILL.md" "$out_dir/house-checksum-after.md"
cat > "$out_dir/report.json" <<JSON
{
  "paper": "Memento-Skills",
  "proof_class": "tiny_live_read_write_attempt",
  "model": "gpt-5.4-mini",
  "case_id": "house-checksum-alpha",
  "loop": {
    "observe": true,
    "read_selection_file": "workspace/output/read_selection.json",
    "act_before_answer": "$answer_before",
    "feedback_before_write": "$feedback",
    "write_update_file": "workspace/output/write_update.json",
    "unit_test_gate": "$gate",
    "act_after_answer": "$answer_after",
    "final_reward": "$final_reward"
  },
  "artifacts": [
    "preflight.json",
    "read_stdout.jsonl",
    "act_before_stdout.jsonl",
    "write_stdout.jsonl",
    "act_after_stdout.jsonl",
    "workspace/output/read_selection.json",
    "workspace/output/act_before_write.json",
    "workspace/output/feedback_before_write.json",
    "workspace/output/write_update.json",
    "workspace/output/unit_test_gate.json",
    "workspace/output/act_after_write.json",
    "house-checksum-after.md"
  ],
  "deferred_full_replication": [
    "trained Memento-Qwen router",
    "8k/3k skill catalog and synthetic router queries",
    "GAIA/HLE datasets and splits",
    "Gemini-3.1-Flash model profile",
    "three reflective retries over benchmark train sets",
    "test-set reporting and Read-Write ablation"
  ]
}
JSON

printf 'wrote live report: %s\n' "$out_dir/report.json"
if [[ "$final_reward" != "correct" ]]; then
  exit 1
fi
