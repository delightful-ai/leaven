# Leaven v0.2.7 - Codex CLI Agent Runtime Adapter

> Status: implemented provider-adapter contract.  
> Date: 2026-05-07.  
> Governing spec: `docs/specs/initial_library.md`.  
> Runtime companion: `docs/specs/agentic_stage_runtime.md`.

`leaven-agent-codex-cli` is the backend-neutral Codex product path. It runs
`codex exec` through `WorkspaceView::run_command` after a stage has already
materialized the candidate world.

The adapter is intentionally boring:

```text
AgentRunRequest
  -> rendered instructions on stdin
  -> codex exec --json --output-last-message ...
  -> CommandRecord + raw stdout/stderr
  -> AgentSession transcript + artifact files
```

It does not know candidates, proposals, assessments, GEPA, skill banks, git, or
workspace layouts beyond the command it needs to run.

## 1. Crate Boundary

```text
leaven-agent-codex
  optional facade feature `cli`

leaven-agent-codex-cli
  -> leaven-agent
  -> leaven-agent-command
  -> leaven-kernel
  -> leaven-workspace
```

Forbidden dependencies:

```text
leaven-agent-codex-cli -> leaven-core
leaven-agent-codex-cli -> leaven-engine
leaven-agent-codex-cli -> leaven-agentic
leaven-agent-codex-cli -> leaven-artifact-skill
leaven-agent-codex-cli -> codex-app-server-protocol
leaven-agent-codex-cli -> codex-protocol
```

The CLI adapter shells out to the installed `codex` binary. It must not vendor
or expose Codex app-server protocol types. App-server support remains in
`leaven-agent-codex-app-server`.

## 2. Type Shape

```rust
pub struct CodexCliConfig {
    pub codex_bin: String,
    pub model: String,
    pub reasoning_effort: CodexCliReasoningEffort,
    pub goal_mode: CodexCliGoalMode,
    pub approval: CodexCliApproval,
    pub last_message_path: WorkspacePath,
    pub timeout: Option<Duration>,
    pub retain_raw_stdout: bool,
    pub retain_raw_stderr: bool,
    pub codex_home: Option<String>,
}

pub struct CodexCliRuntime {
    inner: CommandAgentRuntime<CodexCliSessionParser>,
}
```

`CodexCliRuntime` implements provider-neutral `AgentRuntime` by delegating to
`leaven-agent-command`.

Default execution:

```text
codex exec
  --json
  --skip-git-repo-check
  --model gpt-5.4-mini
  --config model_reasoning_effort="low"
  --output-last-message .leaven/codex-last-message.txt
  --sandbox workspace-write
  -
```

The milestone live path may explicitly choose
`--dangerously-bypass-approvals-and-sandbox` when the workspace backend is
already the sandbox boundary.

Goal-mode execution is an explicit feature opt-in:

```text
CodexCliConfig { goal_mode: CodexCliGoalMode::Enabled, .. }
  -> codex exec --enable goals ...
```

The default remains disabled. Goal planning, spec checking, jj snapshot policy,
and evaluation interpretation stay above this provider leaf.

## 3. Skill Layout

The CLI adapter does not own skill layout and does not copy skills into a
private Codex home.

Codex currently scans repo `.agents/skills` roots. Leaven stages materialize
Agent Skills folders in that layout when they want Codex-native skill
discovery. The runtime simply runs Codex with the workspace cwd; Codex's own
loader decides which skills are visible.

This preserves the ownership split:

```text
SkillBank / SKILL.md validation / mutations
  -> leaven-artifact-skill and leaven-agentic-skill

projection to .agents/skills
  -> materializer or stage layout

Codex discovery of .agents/skills
  -> Codex provider behavior, exercised by the runtime
```

## 4. Session Parsing

The adapter uses `--output-last-message` as the stable final-message channel and
retains `--json` stdout/stderr as raw provider events. It does not rely on the
current JSONL event schema for normalized transcript text.

Laws:

- If the last-message file is present and non-empty, it becomes the final
  assistant message.
- If the last-message file is absent, non-empty stdout is a fallback assistant
  message so command-backed test binaries remain usable.
- Raw stdout/stderr are retained when configured.
- Non-zero Codex exit status marks the session failed, but the session is still
  returned so callers can persist evidence.
- Output contracts are validated by `leaven-agent` after parsing.

## 5. Verification

Required local checks:

```text
cargo nextest run -p leaven-agent-codex-cli
cargo check -p leaven-agent-codex --features cli
cargo test -p leaven --test topology_contract
```

The live P5 gate must exercise the actual Codex CLI with `gpt-5.4-mini`, low
reasoning, developer instructions rendered into stdin, durable `AgentSession`
evidence, and one completed EvoSkill-shaped skill mutation iteration.
