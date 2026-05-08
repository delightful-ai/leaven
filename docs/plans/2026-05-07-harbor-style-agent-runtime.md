# Harbor-Style Agent Runtime Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Leaven's agent execution path work like Harbor's proven model: stages allocate a workspace, materialize the artifact, register skills/MCP/config in the runtime's native layout, execute the provider CLI inside that workspace, collect durable logs/transcripts/output files, parse them into typed proposals or assessments, and resume long runs from stored evidence/checkpoints.

**Architecture:** Keep the engine and cold core out of agent semantics. Harden `leaven-workspace` as the backend-neutral command/file substrate; keep `leaven-agent` as provider-neutral session vocabulary; add command-backed provider leaf runtimes for Codex CLI and Claude Code-style CLIs; keep `leaven-agent-codex-app-server` as the local-mount app-server adapter rather than the container default. `leaven-agentic` remains the adapter layer from agent sessions to `Proposer`/`Evaluator`.

**Tech Stack:** Rust 2024, existing Leaven crates, `jj`, `tokio`, provider CLIs (`codex`, Claude Code-compatible binaries), local workspace tests, live `gpt-5.4-mini` low EvoSkill proof.

---

## Source Grounding

Harbor reference behavior:

- Agent abstraction: `/Users/darin/vendor/github.com/laude-institute/harbor/src/harbor/agents/base.py`
- Environment abstraction: `/Users/darin/vendor/github.com/laude-institute/harbor/src/harbor/environments/base.py`
- Durable trial paths: `/Users/darin/vendor/github.com/laude-institute/harbor/src/harbor/models/trial/paths.py`
- Codex installed-agent wrapper: `/Users/darin/vendor/github.com/laude-institute/harbor/src/harbor/agents/installed/codex.py`
- Claude Code installed-agent wrapper: `/Users/darin/vendor/github.com/laude-institute/harbor/src/harbor/agents/installed/claude_code.py`
- Trial lifecycle: `/Users/darin/vendor/github.com/laude-institute/harbor/src/harbor/trial/trial.py`

Current Leaven surfaces:

- Workspace substrate: `crates/leaven-workspace/src/{workspace.rs,view.rs,command.rs}`
- Local backend: `crates/leaven-workspace-local/src/factory.rs`
- Runtime trait/session vocabulary: `crates/leaven-agent/src/{runtime.rs,session.rs,transcript.rs,error.rs}`
- Agentic stage adapters: `crates/leaven-agentic/src/{proposer.rs,evaluator.rs,repairing_proposer.rs,parser.rs,repair.rs,error.rs}`
- Skill artifact: `crates/leaven-artifact-skill/src/*`
- Skill materializer/parser helpers: `crates/leaven-agentic-skill/src/*`
- Codex app-server adapter: `crates/leaven-agent-codex-app-server/src/*`
- Live EvoSkill proof: `examples/p5_evoskill_iteration`

## Non-Goals

- Do not move agent execution into `leaven-engine`.
- Do not make artifacts know how to run themselves.
- Do not make `AgentRuntime` know `CandidateId`, `Proposal`, `Assessment`, `RunGraph`, `SkillBank`, GEPA, or paper-specific loops.
- Do not make Codex app-server over stdio the default container/runtime path.
- Do not encode EvoSkill, Trace2Skill, or any paper as engine behavior.
- Do not add compatibility shims. Hard cutovers only.

## Definition Of Done

- `leaven-workspace` can express the command shape needed by provider CLIs: cwd, env, stdin, timeout, output limits, and optional user identity or a typed refusal when unsupported.
- Provider-neutral `AgentSession` and transcript records are serde-compatible enough to store and restore without every example inventing `StoredAgentSession`.
- A command-backed runtime substrate exists below provider crates and above `leaven-workspace`.
- Codex has a Harbor-style CLI runtime path that runs `codex exec` inside the materialized workspace with `gpt-5.4-mini` and low effort, registers skills from the materialized skill root, captures native session/log files, validates output contracts, and returns `Metered<AgentSession>`.
- Claude Code-compatible CLI support is either implemented in `leaven-agent-claude-code` or explicitly left as a follow-on with the same command-backed substrate.
- P5 EvoSkill no longer hand-rolls provider/session persistence beyond paper-specific prompts/scoring. Runtime/session/evidence/checkpoint behavior should fall out of Leaven primitives.
- Rerunning P5 after completion detects the stored checkpoint/evidence and resumes rather than repeating the completed iteration.
- Every changed invariant has a law/example/scenario/regression test at the lowest clean layer.
- Verification passes:
  - narrow crate tests while iterating
  - `just test`
  - `LEAVEN_CODEX_LIVE=1 LEAVEN_CODEX_BIN=$HOME/.bun/bin/codex just milestone-p5`
  - `just check` before claiming completion, unless live provider flakiness is separately documented with exact failure evidence.

## Crate Graph

### Existing crates to keep

```text
leaven-workspace
  owns Workspace, WorkspaceView, WorkspaceBackend, Command, WorkspacePath

leaven-agent
  owns AgentRuntime, AgentRunRequest, AgentSession, transcript/session vocabulary

leaven-agentic
  owns AgenticProposer, AgenticEvaluator, RepairingAgenticProposer, parser traits

leaven-agentic-skill
  owns SkillBank materialization/parsing helpers

leaven-agent-codex-app-server
  owns Codex app-server protocol adapter; requires local mount for stdio connector

leaven-agent-codex
  provider-family facade and feature-gated re-exports only

leaven-agent-claude-code
  Claude Code-compatible CLI runtime adapter
```

### New crate to add

```text
leaven-agent-command
  owns reusable command-backed runtime machinery
  depends on: leaven-agent, leaven-kernel, leaven-workspace
  must not depend on: leaven-core, leaven-engine, leaven-agentic, provider protocol crates
  exports:
    CommandAgentRuntime
    CommandAgentConfig
    CommandSessionLayout
    CommandPromptMode
    CommandLogCapture
    CommandSessionParser
```

Why a crate, not a module in `leaven-agent`: command execution is a concrete runtime strategy, not the provider-neutral trait truth. Codex CLI, Claude Code, OpenCode, Mini-SWE-Agent-style wrappers, and test agents can share it without putting command-launch policy into the neutral runtime vocabulary.

### New optional crate if needed

```text
leaven-agent-codex-cli
  owns Codex CLI runtime around `codex exec`
  depends on: leaven-agent, leaven-agent-command, leaven-kernel, leaven-workspace
  must not depend on: leaven-core, leaven-engine, leaven-agentic, codex app-server protocol
```

If the implementation stays tiny, this can be `leaven-agent-codex::cli` behind a feature, but the first preference is a narrow crate because app-server and CLI behavior are operationally different.

Topology contract updates must accompany any new crate:

- `Cargo.toml`
- `crates/leaven/tests/topology_contract.rs`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- crate `src/lib.rs` as map-only

## Task 1: Spec Cutover For Harbor-Style Runtime

**Files:**

- Modify: `docs/specs/agentic_stage_runtime.md`
- Modify: `docs/specs/codex_app_server_agent_runtime.md`
- Optional modify: `docs/specs/agentic_skill_optimization_primitives.md`

**Steps:**

1. Add a Harbor-style execution subsection to `agentic_stage_runtime.md`.
2. State the default product path:

   ```text
   materializer writes workspace
   provider runtime registers native config/skills/MCP
   provider CLI runs inside workspace/backend
   runtime collects native logs and output files
   parser turns session output into typed proposals/evidence
   ```

3. Amend `codex_app_server_agent_runtime.md` to say app-server stdio is a local-mount adapter, not the backend-neutral/container default.
4. Add laws:
   - workspace mutations are not graph mutations
   - runtime setup files are not artifact state unless materialized from artifact input
   - provider raw logs must be retained or explicitly disabled by policy
   - output contracts are validated before parser success is reported
5. Run a docs path check:

   ```bash
   rg -n "agent-server|app-server|Harbor|command-backed|local mount" docs/specs
   ```

**Commit:**

```bash
jj describe -m "spec harbor-style agent runtime path" && jj new
```

## Task 2: Workspace Command Contract

**Files:**

- Modify: `crates/leaven-workspace/src/command.rs`
- Modify: `crates/leaven-workspace/src/view.rs`
- Modify: `crates/leaven-workspace/src/workspace.rs`
- Modify: `crates/leaven-workspace-local/src/factory.rs`
- Test: `crates/leaven-workspace/tests/workspace_view.rs`
- Test: `crates/leaven-workspace-local/tests/local_workspace.rs`

**Type shape:**

```rust
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<WorkspacePath>,
    pub env: BTreeMap<String, String>,
    pub stdin: CommandStdin,
    pub limits: CommandLimits,
    pub user: Option<CommandUser>,
}

pub enum CommandStdin {
    Empty,
    Bytes(Vec<u8>),
}

pub struct CommandLimits {
    pub timeout: Option<Duration>,
    pub max_stdout_bytes: Option<u64>,
    pub max_stderr_bytes: Option<u64>,
}

pub enum CommandUser {
    Name(String),
    Uid(u32),
}

pub struct CommandOutput {
    pub status: ExitStatus,
    pub stdout: CapturedOutput,
    pub stderr: CapturedOutput,
    pub duration: Duration,
}

pub struct CapturedOutput {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}
```

`CommandUser` is optional because local and remote backends differ. A backend that cannot honor it must return `WorkspaceError::UnsupportedOperation { operation: "run_command.user" }`, not silently ignore it.

**Tests:**

- Workspace view scopes cwd through subdir views.
- Local backend passes env vars to a child process.
- Local backend passes stdin to a child process.
- Local backend records duration and exit code.
- Local backend enforces stdout/stderr byte limits with truncation flags.
- Local backend refuses `CommandUser` if unsupported.
- Timeout behavior is either implemented or returns a typed unsupported/timeout error. Do not let a timeout request hang indefinitely.

**Commands:**

```bash
cargo nextest run -p leaven-workspace -p leaven-workspace-local
```

**Commit:**

```bash
jj describe -m "harden workspace command contract" && jj new
```

## Task 3: Durable Agent Sessions

**Files:**

- Modify: `crates/leaven-agent/src/session.rs`
- Modify: `crates/leaven-agent/src/transcript.rs`
- Modify: `crates/leaven-agent/src/error.rs`
- Test: `crates/leaven-agent/tests/runtime_contract.rs`
- Maybe modify: `crates/leaven-evidence/src/command.rs`
- Maybe test: `crates/leaven-evidence/tests/command.rs`

**Type additions:**

```rust
pub struct AgentSession {
    pub session_id: AgentSessionId,
    pub status: AgentStatus,
    pub transcript: AgentTranscript,
    pub commands: Vec<CommandRecord>,
    pub output_files: Vec<WorkspacePath>,
    pub artifact_files: Vec<AgentSessionArtifact>,
    pub raw_provider_events: Vec<RawProviderEvent>,
}

pub struct AgentSessionArtifact {
    pub kind: AgentSessionArtifactKind,
    pub path: WorkspacePath,
    pub media_type: Option<String>,
}

pub enum AgentSessionArtifactKind {
    NativeLog,
    ProviderSession,
    NormalizedTrajectory,
    Debug,
    Other(String),
}
```

Add serde derives to `AgentSession`, transcript records, command records, `WorkspacePath`, and any nested kernel/workspace values needed for file-backed evidence/checkpoint use.

**Laws:**

- A session can be serialized and deserialized without losing status, transcript order, command order, output file paths, artifact file paths, or raw event count.
- `OutputContract::Files` and `OutputContract::JsonFile` validate through `WorkspacePath`, not host paths.
- Raw provider events are retained only when the runtime config asks for them; artifact file paths may still point to native logs.

**Commands:**

```bash
cargo nextest run -p leaven-agent -p leaven-evidence
```

**Commit:**

```bash
jj describe -m "make agent sessions durable" && jj new
```

## Task 4: Reusable Command Agent Runtime

**Files:**

- Add: `crates/leaven-agent-command/Cargo.toml`
- Add: `crates/leaven-agent-command/src/lib.rs`
- Add: `crates/leaven-agent-command/src/{config.rs,runtime.rs,parser.rs,error.rs}`
- Add: `crates/leaven-agent-command/tests/command_runtime.rs`
- Modify: root `Cargo.toml`
- Modify: `crates/leaven/tests/topology_contract.rs`

**Core shape:**

```rust
pub struct CommandAgentRuntime<Parser> {
    config: CommandAgentConfig,
    parser: Parser,
}

pub struct CommandAgentConfig {
    pub id: AgentRuntimeId,
    pub fingerprint_seed: String,
    pub setup: Vec<CommandTemplate>,
    pub run: CommandTemplate,
    pub layout: CommandSessionLayout,
    pub retain_raw_stdout: bool,
    pub retain_raw_stderr: bool,
}

pub struct CommandTemplate {
    pub program: String,
    pub args: Vec<CommandTemplateArg>,
    pub cwd: Option<WorkspacePath>,
    pub env: BTreeMap<String, String>,
    pub stdin: CommandPromptMode,
}

pub enum CommandPromptMode {
    None,
    StdinTask,
    ArgTask,
    ArgJsonRequest,
}

pub trait CommandSessionParser: Send + Sync {
    fn parse_session(
        &self,
        request: &AgentRunRequest,
        setup_outputs: &[CommandOutput],
        run_output: &CommandOutput,
        workspace: &mut WorkspaceView<'_>,
    ) -> Result<AgentSession, CommandAgentError>;
}
```

This runtime does not know Codex, Claude, skills, proposals, evidence, or candidates. It only handles setup command(s), run command, output contract validation, raw log artifact collection, cancellation-before-start, timeout mapping, and parser handoff.

**Tests:**

- Setup command can create provider config files before run command.
- Run command receives prompt through stdin.
- Run command receives prompt as a CLI arg when configured.
- Runtime records command outputs.
- Runtime validates output contract after parser returns.
- Runtime maps nonzero exit code into `AgentStatus::Failed` or typed error according to config.
- Runtime does cleanup through the caller's workspace lifecycle, not its own tempdir.

**Commands:**

```bash
cargo nextest run -p leaven-agent-command
cargo test -p leaven --test topology_contract
```

**Commit:**

```bash
jj describe -m "add command-backed agent runtime substrate" && jj new
```

## Task 5: Codex CLI Runtime

**Files:**

- Add preferred: `crates/leaven-agent-codex-cli/*`
- Or modify if keeping inside facade: `crates/leaven-agent-codex/src/{lib.rs,cli.rs}`
- Modify: `crates/leaven-agent-codex/Cargo.toml`
- Modify: root `Cargo.toml`
- Modify: `crates/leaven/tests/topology_contract.rs`
- Test: `crates/leaven-agent-codex-cli/tests/codex_cli_config.rs`
- Optional live test: `crates/leaven-agent-codex-cli/tests/live_codex_cli.rs`

**Runtime behavior:**

Mirror Harbor's Codex wrapper, but use Leaven types:

```text
setup:
  create provider home under workspace, e.g. .leaven/codex-home
  write auth/config references without leaking secrets into artifact state
  copy or expose materialized skills to provider-native skill path
  write MCP config if request/provider config includes it

run:
  codex exec
    --dangerously-bypass-approvals-and-sandbox only when explicit policy allows
    --skip-git-repo-check
    --model gpt-5.4-mini
    --json
    -c model_reasoning_effort=low
    -- <instructions>

collect:
  native stdout/stderr log
  Codex session jsonl files when present
  normalized AgentTranscript
  output contract files
```

**Config shape:**

```rust
pub struct CodexCliRuntime {
    command: CommandAgentRuntime<CodexCliSessionParser>,
}

pub struct CodexCliConfig {
    pub codex_bin: String,
    pub model: String,
    pub reasoning_effort: CodexReasoningEffort,
    pub home: WorkspacePath,
    pub skills_root: Option<WorkspacePath>,
    pub approval: CodexCliApprovalPolicy,
    pub retain_native_session: bool,
}
```

Default model for live tests: `gpt-5.4-mini`, effort `low`.

**Tests:**

- Unit tests build exact command args without running Codex.
- Skill registration copies from materialized `skills/` to provider-native `$HOME/.agents/skills`.
- Parser converts minimal Codex JSON/JSONL events into transcript messages/tool calls.
- Raw native session files are returned as `AgentSessionArtifact`.
- Live test is ignored or feature-gated, but `just milestone-p5` must exercise the real path.

**Commands:**

```bash
cargo nextest run -p leaven-agent-codex-cli
cargo check -p leaven-agent-codex --no-default-features
```

**Commit:**

```bash
jj describe -m "add codex cli agent runtime" && jj new
```

## Task 6: Claude Code-Compatible CLI Runtime

**Files:**

- Modify: `crates/leaven-agent-claude-code/src/{lib.rs,config.rs,runtime.rs}`
- Test: `crates/leaven-agent-claude-code/tests/claude_code_runtime.rs`

**Runtime behavior:**

Use the same command substrate, with binary path configurable. It should work with official Claude Code or the `claude-code-best` compatible CLI if the flags are compatible:

```text
claude --verbose --output-format=stream-json
  --permission-mode=bypassPermissions
  --print -- <instructions>
```

Register skills under the provider-native config directory. Preserve native logs/session files as `AgentSessionArtifact`.

**Constraints:**

- Do not modify `~/vendor/github.com/claude-code-best/claude-code` unless the CLI surface cannot satisfy Leaven's runtime contract.
- Do not make Claude Code support block Codex/EvoSkill completion. This task may land after Task 9 if needed.

**Commands:**

```bash
cargo nextest run -p leaven-agent-claude-code
```

**Commit:**

```bash
jj describe -m "add claude code cli agent runtime" && jj new
```

## Task 7: Skill Runtime Registration Product Path

**Files:**

- Modify: `crates/leaven-agentic-skill/src/layout.rs`
- Modify: `crates/leaven-agentic-skill/src/materializer.rs`
- Maybe add: `crates/leaven-agentic-skill/src/runtime_layout.rs`
- Test: `crates/leaven-agentic-skill/tests/skill_agentic.rs`

**Goal:**

Materialization remains provider-neutral:

```text
workspace/skills/<skill-name>/SKILL.md
workspace/skills/<skill-name>/scripts/...
```

Provider runtime config decides how to register that directory:

```text
Codex CLI: copy/link to $HOME/.agents/skills
Claude Code: copy/link to $CLAUDE_CONFIG_DIR/skills
Other: provider-specific
```

**Tests:**

- Materializer preserves arbitrary files and executable bits.
- Runtime registration does not mutate the `SkillBank` artifact.
- Missing or invalid `SKILL.md` fails before runtime execution.
- Rename remains first-class and continuity-preserving at artifact level.

**Commands:**

```bash
cargo nextest run -p leaven-artifact-skill -p leaven-agentic-skill
```

**Commit:**

```bash
jj describe -m "wire skill registration through runtime layout" && jj new
```

## Task 8: Evidence And Checkpoint Product Path

**Files:**

- Modify: `crates/leaven-evidence/src/command.rs`
- Modify: `crates/leaven-store/src/checkpoint.rs`
- Modify as needed: `crates/leaven-store-file/src/*`
- Test: `crates/leaven-evidence/tests/command.rs`
- Test: `crates/leaven-store-file/tests/file_stores.rs`

**Goal:**

Make the P5 pattern generic:

```text
AgentSession
  -> AgentTrajectoryEvidence or session evidence ref
  -> EvidenceStore
  -> CheckpointStore records run phase
  -> rerun resumes from latest checkpoint
```

The checkpoint store may still be bytes-oriented, but examples should not need to invent bespoke `StoredAgentSession` just to preserve transcript/status/output files.

**Tests:**

- Agent trajectory evidence can hold an inline transcript or blob-backed transcript.
- File evidence store round-trips session-derived evidence.
- Checkpoint store can record a typed JSON checkpoint payload through the bytes interface.
- Resume test proves "complete iteration" is idempotent.

**Commands:**

```bash
cargo nextest run -p leaven-evidence -p leaven-store-file
```

**Commit:**

```bash
jj describe -m "make agent evidence and checkpoints reusable" && jj new
```

## Task 9: Cut P5 EvoSkill Onto The Product Path

**Files:**

- Modify: `examples/p5_evoskill_iteration/src/codex.rs`
- Modify: `examples/p5_evoskill_iteration/src/evidence.rs`
- Modify: `examples/p5_evoskill_iteration/src/checkpoint.rs`
- Modify as needed: `examples/p5_evoskill_iteration/src/main.rs`
- Test by running: `just milestone-p5`

**Goal:**

P5 remains paper-specific only where it should be:

- EvoSkill prompts
- fixture data
- failure selection
- proposal parsing
- scoring/admission logic

Everything else should be generic Leaven:

- skill artifact validation
- skill materialization
- Codex runtime execution
- session transcript/artifact capture
- output contract validation
- evidence storage
- checkpoint/resume

**Live command:**

```bash
rm -rf tmp/p5_evoskill_iteration/live
LEAVEN_CODEX_LIVE=1 LEAVEN_CODEX_BIN=$HOME/.bun/bin/codex just milestone-p5
LEAVEN_CODEX_LIVE=1 LEAVEN_CODEX_BIN=$HOME/.bun/bin/codex just milestone-p5
```

Expected:

- First run completes one EvoSkill iteration.
- First run stores evidence and checkpoints under `tmp/p5_evoskill_iteration/live`.
- Second run reports the completed resume state and does not redo the live Codex work.
- Codex uses `gpt-5.4-mini` low effort.
- Developer instructions are included in the runtime request, not smuggled through example-only globals.

**Commit:**

```bash
jj describe -m "run evoskill through generic agent runtime path" && jj new
```

## Task 10: Full Verification And Documentation Closure

**Files:**

- Modify if behavior changed: `docs/testing/README.md`
- Modify if topology changed: `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- Modify if new runtime behavior is stable API: relevant crate docs

**Commands:**

```bash
cargo fmt --check
cargo nextest run -p leaven-workspace -p leaven-workspace-local -p leaven-agent -p leaven-agentic -p leaven-agentic-skill
cargo test -p leaven --test topology_contract
just test
LEAVEN_CODEX_LIVE=1 LEAVEN_CODEX_BIN=$HOME/.bun/bin/codex just milestone-p5
just check
```

If `just check` repeats the live P5 path through coverage and provider flakiness appears, record the exact failure, leave the deterministic/law suites green, and rerun the live command once before deciding whether it is infrastructure flake or product failure.

**Commit:**

```bash
jj describe -m "verify harbor-style agent runtime path" && jj new
```

## Open Design Checks During Implementation

- If `CommandUser` makes local implementation too platform-specific, keep the type but let local return a typed unsupported error. Do not silently ignore it.
- If `leaven-agent-command` becomes only 50 lines, keep it anyway if Codex and Claude both use it. The boundary prevents provider-specific command logic from leaking into `leaven-agent`.
- If Codex CLI cannot expose native session files reliably, preserve stdout/stderr stream JSON as the required native log artifact and treat session JSONL as best effort.
- If Claude Code Best differs materially from official Claude Code stream JSON, model that as a runtime flavor in `leaven-agent-claude-code`, not as engine behavior.
- If P5 still needs a bespoke session wrapper after Task 8, the generic session/evidence design is not done.

