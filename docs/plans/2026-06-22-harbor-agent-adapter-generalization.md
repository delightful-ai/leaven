# Harbor Agent-Kit Adapter Generalization

Status: design + implementation plan.
Date: 2026-06-22.
Governs: `sdk/python/src/leaven/x/harbor/` (rollout/agent surface) and the
`sdk/python/examples/codex_terminal_bench` migration.
Supersedes the Codex-only `rollout.codex_agent_kit` surface from
`docs/specs/harbor_leaven_adapter.md` §3/§6 (that spec is updated in the same
change).

## Problem

The committed Harbor adapter wires exactly one Harbor agent (`LeavenCodex`) and
hardcodes the task working directory `/app`:

- `lv.x.harbor.rollout.codex_agent_kit(...)` is Codex-only.
- `LeavenCodex` uploads `AGENTS.md` to `<workdir>/AGENTS.md` and skills to
  `<workdir>/.agents/skills`, with `workdir` defaulting to the literal `/app`.

`/app` is not a Codex fact and not a Harbor fact. It is the working directory of
one task image (terminal-bench-2 `regex-log`, whose Dockerfile sets `WORKDIR
/app`). Harbor's own canonical paths are `/logs`, `/tests`, `/solution`,
`/harbor/skills` — `/app` appears nowhere in the Harbor package. So the current
adapter silently assumes a task-image detail and supports only one of Harbor's
~28 agents.

## Goal

A user should select any supported Harbor agent for an `AgentKitArtifact`
rollout through one generic entry, with the kit injected through that agent's
real configuration surface — not a guessed working directory:

```python
rollout = lv.x.harbor.rollout.agent_kit(
    agent="claude-code",                 # or "codex", or an import_path
    model="anthropic/claude-sonnet-4-6",
    placement="user",                    # "user" (workdir-independent) | "repo"
    trials_dir=".leaven/harbor-trials",
)
```

Hard cutover: `codex_agent_kit` is removed; the terminal-bench example moves to
`agent_kit(agent="codex", placement="repo", workdir="/app")` (workdir is now an
explicit, overridable parameter, not a buried constant).

## How agents actually load a kit (grounded)

An `AgentKitArtifact` is `system_prompt: str` + `skills: list[{path, content}]`.
Each Harbor agent exposes two channels for these, both confirmed against
`harbor==0.13.1`, the Codex CLI 0.141 docs, and the Claude Code docs:

| Channel | Claude Code | Codex |
|---|---|---|
| system prompt, **user scope** | `--append-system-prompt` (Harbor `CliFlag`, set via `AgentConfig.kwargs`, *appends* to the base prompt) | `$CODEX_HOME/AGENTS.md` (read globally; *appended* context, unlike `model_instructions_file` which replaces) |
| system prompt, **repo scope** | `<workdir>/CLAUDE.md` | `<workdir>/AGENTS.md` (Codex scans cwd→repo-root) |
| skills, **user scope** | `AgentConfig.skills` → `$CLAUDE_CONFIG_DIR/skills/` | `AgentConfig.skills` → `$HOME/.agents/skills/` (both are documented user-global skill dirs) |
| skills, **repo scope** | `<workdir>/.claude/skills/<n>/SKILL.md` | `<workdir>/.agents/skills/<path>` |

Harbor sets `CLAUDE_CONFIG_DIR` to a per-trial directory, so Claude Code
user-scope injection is fully isolated per trial and needs **no subclass** —
pure `AgentConfig(name="claude-code", model_name=, skills=[...],
kwargs={"append_system_prompt": ...})`.

Skills are uniform across agents via `AgentConfig.skills` for user scope. The
only per-agent, per-scope code is the system-prompt installer.

## Surface

```text
leaven.x.harbor.rollout
  agent_kit(*, agent, model, placement="user", workdir="/app",
            task_key="harbor_task", trials_dir=".leaven/harbor-trials",
            timeout_multiplier=1.0, api_key_env=None, trial_runner=None) -> lv.Rollout

leaven.x.harbor.agents            # the registry
  AGENTS: dict[str, HarborAgentAdapter]   # "codex", "claude-code"
  resolve(agent) -> HarborAgentAdapter    # name or import_path

leaven.x.harbor                   # subclasses used by repo scope / codex user scope
  LeavenCodex          # writes $CODEX_HOME/AGENTS.md (user) or <workdir> files (repo)
  LeavenClaudeCode     # writes <workdir>/CLAUDE.md + .claude/skills (repo only)
```

`HarborAgentAdapter` is a small protocol per agent:

```python
class HarborAgentAdapter:
    key: str
    default_model: str
    api_key_env: str                 # "ANTHROPIC_API_KEY" | "OPENAI_API_KEY"
    def build_agent_config(self, *, kit, model, placement, workdir,
                           kit_dir, skills_dirs, api_key) -> AgentConfig: ...
```

The rollout materializes the kit once (system prompt file + skill dirs) and the
adapter projects it into the right `AgentConfig` (name vs import_path, skills,
kwargs, env) for the chosen placement.

## Placement semantics

- `placement="user"` (default; workdir-independent):
  - Claude Code: `AgentConfig(name="claude-code", skills=[...],
    kwargs={"append_system_prompt": kit.system_prompt})`. No subclass.
  - Codex: `AgentConfig(import_path="leaven.x.harbor:LeavenCodex", skills=[...],
    kwargs={"system_prompt": kit.system_prompt})`; `LeavenCodex.run` writes
    `$CODEX_HOME/AGENTS.md` before `super().run()`.
- `placement="repo"` (requires `workdir`):
  - Both agents use a Leaven subclass that uploads the materialized kit into
    `<workdir>` (Codex: `AGENTS.md` + `.agents/skills`; Claude Code: `CLAUDE.md`
    + `.claude/skills/<n>/SKILL.md`) before `super().run()`.
  - This is the proven terminal-bench path, now with `workdir` configurable.

## Trial outcome / scoring / trajectory / import / dependency boundary

Unchanged from `docs/specs/harbor_leaven_adapter.md` §7–§12. `HarborTrialOutcome`,
`rewards.map_key`/`ctrf_fraction`, `trajectory_excerpt`, `import_trial_result`,
and the optional-dependency + no-spend `trial_runner` seam all carry over.

## Implementation slices

1. `agents.py`: `HarborAgentAdapter` protocol + `_CodexAdapter`,
   `_ClaudeCodeAdapter`, `AGENTS` registry, `resolve()`.
2. `_kit.py`: materialize kit → (system-prompt text, skill dirs) and the
   repo-scope on-disk layout per agent.
3. `_agent.py`: migrate `LeavenCodex` off the `/app` constant (user scope →
   `$CODEX_HOME/AGENTS.md`; repo scope → configurable `<workdir>`); add
   `LeavenClaudeCode` (repo scope). Delete `DEFAULT_WORKDIR="/app"`.
4. `rollout.py`: replace `codex_agent_kit` with `agent_kit(agent=...)`; build the
   `TrialConfig` via the resolved adapter.
5. `__init__.py`: export `agent_kit`, `agents`, `LeavenClaudeCode`; drop
   Codex-only `DEFAULT_WORKDIR`/`SKILLS_SUBDIR` exports.
6. `tests/x/test_harbor.py`: agent_kit deterministic tests — Claude Code
   user-scope `AgentConfig` (name, model, append_system_prompt, skills, no
   `/app`); Codex user-scope (`$CODEX_HOME` not `/app`) and repo-scope (workdir
   honored); fake-`trial_runner` mechanics; import boundary.
7. `examples/codex_terminal_bench`: migrate `trial.py`/`agent.py` to
   `agent_kit(agent="codex", placement="repo", workdir="/app")`; keep the
   no-spend mechanics test green.
8. Claude Code live runbook: a single-trial example + exact command. Live run
   requires Docker (present) + `ANTHROPIC_API_KEY` or `CLAUDE_CODE_OAUTH_TOKEN`
   (not in repo env), so it is left runnable, not auto-run.

## Verification

- `uv run --project sdk/python pytest sdk/python/tests/x/test_harbor.py -q`
- `uv run --project sdk/python pytest sdk/python/tests -q`
- `uv run --project sdk/python/examples/codex_terminal_bench pytest tests -q`
- `uvx ruff check sdk/python/src/leaven/x/harbor`, `uvx ty` on the package
- `uv run --project sdk/python python sdk/python/scripts/check_quality_contract.py`
- Live (operator, with key): the Claude Code `regex-log` trial command in §8.

## Non-goals

Same as `docs/specs/harbor_leaven_adapter.md` §13, plus: no per-agent skill
*format* translation beyond writing `{path: content}` into the agent's skill
dir; no auto-detection of a task's working directory (`workdir` is explicit).
