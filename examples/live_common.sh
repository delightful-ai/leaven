#!/usr/bin/env bash

leaven_resolve_codex_bin() {
  local configured="${LEAVEN_CODEX_BIN:-}"
  if [[ -n "$configured" ]]; then
    printf '%s\n' "$configured"
    return 0
  fi
  if command -v codex >/dev/null 2>&1; then
    command -v codex
  else
    printf '%s\n' "$HOME/.bun/bin/codex"
  fi
}

leaven_require_live_codex() {
  local proof_name="$1"
  local codex_bin="$2"
  if [[ "${LEAVEN_CODEX_LIVE:-}" != "1" ]]; then
    echo "live $proof_name run requires LEAVEN_CODEX_LIVE=1" >&2
    exit 2
  fi
  if [[ ! -x "$codex_bin" ]]; then
    echo "Codex binary is not executable: $codex_bin" >&2
    exit 2
  fi
}

leaven_run_codex_json() {
  local work_dir="$1"
  local codex_bin="$2"
  local prompt_file="$3"
  local last_message="$4"
  local stdout_file="$5"
  local stderr_file="$6"
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
