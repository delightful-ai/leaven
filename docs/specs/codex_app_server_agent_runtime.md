# Leaven v0.2.5 - Codex App-Server Agent Runtime Adapter

> Status: pre-implementation provider-adapter spec.  
> Date: 2026-05-07.  
> Governing spec: `docs/specs/initial_library.md`.  
> Runtime companion: `docs/specs/agentic_stage_runtime.md`.  
> Skill companion: `docs/specs/agentic_skill_optimization_primitives.md`.  
> Reference implementation read: `/Users/darin/src/personal/DSRs/crates/dsrs-codex-agent`
> and `/Users/darin/src/personal/DSRs/crates/dsrs-repo-agent`.

This document specifies the first real Codex provider adapter for Leaven:
`leaven-agent-codex-app-server`.

`leaven-agent-codex` is only the Codex provider-family facade. It must not
own app-server protocol code directly because Codex app-server, Codex CLI,
and future hosted/container transports have different operational semantics.

The purpose is narrow:

```text
given an already materialized workspace and an AgentRunRequest,
run one Codex app-server session,
normalize the transcript into AgentSession,
validate the requested output contract,
and return a Metered<AgentSession>.
```

Everything else remains outside the provider adapter. Codex does not become a
core concept, an engine concept, a skill concept, or a GEPA concept.

---

## 1. Layer Boundary

The Codex adapter is a provider runtime implementation. It is not an agentic
stage adapter and it is not a repo optimizer.

```text
leaven-agent
  owns provider-neutral AgentRuntime, AgentRunRequest, AgentSession

leaven-agent-codex
  owns Codex provider-family facade and optional re-exports

leaven-agent-codex-app-server
  owns CodexAppServerRuntime: AgentRuntime
  owns Codex app-server protocol, transcript normalization, errors
  owns CodexAppServerConnector / CodexAppServerTransport seams
  owns stdio only as one connector, not as the adapter identity

leaven-agentic
  owns AgenticProposer, AgenticEvaluator, parsers

leaven-artifact-skill / leaven-agentic-skill
  own skill folders, validation, materialization layouts

optimizer crates
  own search rhythm, candidate selection, repair policy, population updates
```

`leaven-agent-codex-app-server` must not know:

```text
OptimizationProblem
CandidateId
Proposal
Assessment
RunGraph
Population
Gepa
SkillBank
GitArtifact
```

It may know:

```text
AgentRuntime
AgentRunRequest
AgentSession
AgentTranscript
OutputContract
WorkspaceView
WorkspacePath
Cost
Fingerprint
Codex app-server protocol types
CodexAppServerConnector
CodexAppServerTransport
```

This keeps the provider copyable. The DSRs code combines Codex app-server
threads with repo materialization and git readback. Leaven should copy the
transport/client/session/history ideas, not the DSRs repo-agent ownership
boundary.

---

## 2. Crate and Feature Shape

`leaven-agent-codex-app-server` is the concrete app-server leaf provider
crate. `leaven-agent-codex` is a facade crate and may depend optionally on
`leaven-agent-codex-app-server` only for re-export ergonomics.

Allowed dependencies:

```text
leaven-agent-codex-app-server
  -> leaven-agent
  -> leaven-kernel
  -> leaven-workspace
  -> codex-app-server-protocol   // leaf-only, feature-gated
  -> codex-protocol              // leaf-only, feature-gated typed config fields
  -> serde / serde_json          // provider protocol/config only
  -> tokio process/io/time       // provider connection only
```

Forbidden dependencies:

```text
leaven-agent-codex-app-server -> leaven-core
leaven-agent-codex-app-server -> leaven-engine
leaven-agent-codex-app-server -> leaven-agentic
leaven-agent-codex-app-server -> leaven-artifact-skill
leaven-agent-codex-app-server -> leaven-gepa
leaven-agent -> codex-app-server-protocol
leaven-agentic -> codex-app-server-protocol
leaven-workspace -> codex-app-server-protocol
```

Feature policy:

```toml
[features]
default = []
app-server = [
  "dep:codex-app-server-protocol",
  "dep:codex-protocol",
  "dep:serde",
  "dep:serde_json",
  "dep:tokio",
]
stdio = ["app-server"]
live-codex-tests = ["stdio"]
```

The exact dependency list may change during implementation, but the rule does
not: Codex protocol dependencies are confined to
`leaven-agent-codex-app-server` and are not enabled by the umbrella `leaven`
crate unless the user opts into a concrete provider feature.

The crate must compile with:

```text
cargo check -p leaven-agent-codex --no-default-features
cargo check -p leaven-agent-codex-app-server --no-default-features
cargo check -p leaven-agent-codex-app-server --features app-server
cargo check -p leaven-agent-codex-app-server --features stdio
```

The no-default-features build may expose config vocabulary and a clear
"app-server feature disabled" constructor error. It must not pull the Codex
protocol crate.

---

## 3. Runtime Type Shape

The provider implementation should be boring:

```rust
pub struct CodexAppServerRuntime<C> {
    config: CodexAppServerConfig,
    connector: C,
}

impl<C: CodexAppServerConnector> AgentRuntime for CodexAppServerRuntime<C> {
    fn id(&self) -> AgentRuntimeId;
    fn fingerprint(&self) -> Fingerprint;
    fn capabilities(&self) -> AgentRuntimeCapabilities;

    fn run_session<'a>(
        &'a self,
        workspace: &'a mut WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'a>,
    ) -> impl Future<Output = Result<Metered<AgentSession>, AgentRuntimeError>> + Send + 'a;
}
```

`CodexAppServerRuntime` has no type parameter for `OptimizationProblem`, artifact,
surface, evidence, or parser.

The runtime is generic over a connector because "app-server over stdio on the
host" and "app-server reached inside a container/sandbox" are not the same
workspace problem. Stdio is just `StdioCodexAppServerConnector`, which
advertises `WorkspaceAccessMode::RequiresLocalMount`; container or backend
connectors can later return a transport and app-server cwd without forcing a
host-local path.

Suggested config shape:

```rust
pub struct CodexAppServerConfig {
    pub initialize: CodexAppServerInitializeConfig,
    pub thread: CodexAppServerThreadConfig,
    pub turn: CodexAppServerTurnConfig,
    pub approval_mode: CodexAppServerApprovalMode,
    pub retain_raw_events: CodexRawEventPolicy,
}

pub struct StdioCodexAppServerConnector {
    pub codex_bin: PathBuf,
    pub config_overrides: Vec<String>,
}

pub struct CodexAppServerInitializeConfig {
    pub client_name: String,
    pub client_title: Option<String>,
    pub experimental_api: bool,
    pub opt_out_notification_methods: Option<Vec<String>>,
}

pub struct CodexAppServerThreadConfig {
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub service_tier: Option<String>,
    pub sandbox: Option<CodexSandboxMode>,
    pub approval_policy: Option<CodexApprovalPolicy>,
    pub approvals_reviewer: Option<CodexApprovalsReviewer>,
    pub ephemeral: bool,
    pub service_name: Option<String>,
}

pub struct CodexAppServerTurnConfig {
    pub model: Option<String>,
    pub effort: Option<CodexReasoningEffort>,
    pub summary: Option<CodexReasoningSummary>,
    pub sandbox_policy: Option<CodexSandboxPolicy>,
    pub approval_policy: Option<CodexApprovalPolicy>,
    pub approvals_reviewer: Option<CodexApprovalsReviewer>,
}

pub enum CodexAppServerApprovalMode {
    Error,
    Accept,
    Decline,
    Cancel,
}
```

`CodexAppServerApprovalMode::Error` is the default. It matches the DSRs default
and is the right optimization default: an unattended optimization run should
not silently grant extra permissions when a provider asks.

`CodexAppServerThreadConfig::ephemeral` defaults to `false`. Leaven optimization
runs should preserve provider thread history by default because transcripts are
evidence, debugging material, and checkpoint/replay substrate. Ephemeral threads
remain available for explicit throwaway sessions, but the runtime must not call
`thread/read includeTurns` for them; Codex app-server intentionally refuses that
operation for unmaterialized threads.

The first implementation ships `StdioCodexAppServerConnector`, but the runtime
is generic over `CodexAppServerConnector`. Remote/container transports should
be new connector implementations, not new meanings for stdio.

---

## 4. Workspace Semantics

`StdioCodexAppServerConnector` requires a local mount.

```rust
fn capabilities(&self) -> AgentRuntimeCapabilities {
    AgentRuntimeCapabilities {
        workspace_access: WorkspaceAccessMode::RequiresLocalMount,
        supports_commands: true,
        supports_raw_provider_events: true,
    }
}
```

For the stdio connector at `run_session` entry:

1. Resolve `request.cwd` against `workspace.local_mount()`.
2. If no local mount exists, return
   `AgentRuntimeError::LocalMountRequired { runtime: self.id() }`.
3. Spawn `codex app-server --listen stdio://` with the resolved working
   directory supplied through Codex `thread/start` or `turn/start` params.
4. Apply `request.env` only to the app-server child process environment or
   provider request config. Do not write env vars to the workspace and do not
   echo them into transcripts.

This means a pure-remote workspace backend such as E2B, Kubernetes, or
Firecracker is not supported by the stdio connector unless it can present a
real local mount usable by the host-side Codex CLI. That is honest and better
than pretending `ws.path()` exists for every backend.

Future backend-neutral Codex execution would require one of:

- a Codex app-server transport that runs inside the workspace backend and
  exposes JSON-RPC over a backend channel
- a workspace backend that can spawn a long-lived stdio process and stream
  JSON-RPC
- a provider-managed workspace mode where Codex owns the workspace and Leaven
  reads back a snapshot afterward

None of those are part of v0.2.5.

---

## 5. Request Mapping

`AgentRunRequest` remains provider-neutral. The Codex adapter maps it into
Codex `thread/start` and `turn/start`.

### 5.1 Instructions

Mapping:

| Leaven value | Codex field |
|---|---|
| `request.instructions.system` | `ThreadStartParams::developerInstructions` |
| `request.instructions.task` | first `UserInput::Text` in `TurnStartParams::input` |
| `request.instructions.context` | provider-neutral path context appended to the turn text unless a provider-specific helper maps a context ref to `UserInput::Skill`, `Mention`, or `LocalImage` |
| `request.cwd` | `ThreadStartParams::cwd` and/or `TurnStartParams::cwd` |

The default context mapping is text-only:

```text
Context:
- <label>: <workspace-relative-path> (<media-type if present>)
```

The runtime must not read referenced files automatically. Reading context files
is the responsibility of the agent inside the workspace, or of a renderer that
explicitly inlines file contents before the request reaches the runtime.

Provider-specific helpers may build richer Codex `UserInput` values, but those
helpers are not allowed to depend on `SkillBank`. A helper can accept a name
and already-materialized path; it must not own skill validation or
materialization.

### 5.2 Output contracts

Mapping:

| `OutputContract` | Runtime responsibility |
|---|---|
| `Files { paths }` | after turn completion, verify every path exists in the workspace |
| `JsonFile { path, schema }` | verify the path exists; if a schema validator is available in `leaven-agent`, validate syntax/schema there, otherwise leave schema validation to the stage parser |
| `FinalMessage` | require a non-empty final assistant message in the normalized transcript |
| `WorkspaceDiff { roots }` | require the roots to exist; do not compute or parse the diff |

`WorkspaceDiff` is intentionally weak at the runtime layer. A stage-owned
stage-owned proposal parser decides how to turn a diff, git commit, snapshot,
or folder replacement into `ProposalBatch`.

Codex `TurnStartParams::outputSchema` constrains final assistant messages, not
workspace files. The first Leaven adapter must not misuse it for `JsonFile`.
Add a separate `FinalMessageJson` output contract later if that becomes a real
need.

### 5.3 Tool policy

`AgentToolPolicy` is a promise the runtime must either enforce or reject.

Rules:

- If `request.tool_policy.allow_shell == false` and the Codex config cannot
  enforce "no shell", return `AgentRuntimeError::Policy`.
- If `allowed_tools` is non-empty and the Codex config cannot enforce exactly
  that allowlist, return `AgentRuntimeError::Policy`.
- If the runtime maps `allowed_tools` into Codex provider config, the mapping
  must be explicit and covered by tests.
- Do not silently treat a Leaven tool policy as advisory.

The first implementation may support only the default permissive Leaven policy
plus conservative Codex approval handling. That is acceptable as long as
unsupported stricter policies fail before the session starts.

### 5.4 Limits and cancellation

`AgentLimits` map to runtime control:

- `timeout`: wrap app-server launch and turn streaming in a timeout.
- `max_turns`: first implementation supports only one turn; values other than
  `None` or `Some(1)` are rejected with `AgentRuntimeError::Policy`.
- `max_output_bytes`: cap retained transcript/raw-event bytes. If the cap is
  exceeded, return a session with `AgentStatus::Failed` or a runtime error
  before unbounded memory growth.
- `CancellationRef`: check before launch, before turn start, and while
  streaming notifications. On cancellation, send `turn/interrupt` when a turn
  is active, then return `AgentStatus::Cancelled`.

---

## 6. Session Lifecycle

One `AgentRuntime::run_session` call corresponds to one Codex thread and one
Codex turn in v0.2.5.

```text
spawn app-server transport
initialize
thread/start
turn/start
stream notifications until matching turn/completed
thread/read if needed to refresh history
validate output contract
shutdown transport
return Metered<AgentSession>
```

The DSRs reference already supports `thread/resume`, `turn/steer`, and
multi-turn completion. Leaven should not expose those as the first provider
contract. Reproposal and repair are Leaven stage policy, and should route back
through the same proposer by issuing a new runtime session with explicit repair
instructions and prior failure context.

Provider-native continuation can be added later behind a separate
`CodexThreadHandle` if a real repair loop proves that preserving the same
Codex thread is worth the lifecycle complexity.

---

## 7. Transcript Normalization

The runtime returns Leaven's provider-neutral transcript plus optional raw
provider events.

Required normalization:

| Codex notification/item | Leaven output |
|---|---|
| user message | `TranscriptEvent::Message { role: User, .. }` |
| agent message delta / completed agent message | assistant message text |
| plan delta / plan item | raw provider event, optionally tool-style transcript event later |
| reasoning summary/text | raw provider event only by default |
| command execution item | `CommandRecord` and `TranscriptEvent::ToolCall` when command text is available |
| command output delta | aggregate into matching command output |
| file change item | add changed paths to `output_files` when paths are parseable; always preserve raw event |
| MCP/dynamic tool call | `ToolCallRecord` plus raw event |
| warning/guardian/deprecation | raw provider event |
| error | session error/status plus raw event |
| unknown notification | raw provider event with stable `kind`; no panic |

The DSRs `CodexAgentHistory` shape is a good source to copy:

- accumulate turns by id
- upsert items by provider item id
- append text deltas to the matching item
- keep commands, file changes, tool calls, warnings, and errors separate
- expose a `final_assistant_text()` helper

Leaven differences:

- do not derive DSRs/BAML traits
- do not render transcripts with templates in the runtime
- do not convert file-change debug strings into typed paths unless the Codex
  protocol gives a parseable path
- preserve provider item ids in raw events or metadata where possible

### 7.1 Raw event policy

Raw provider events are useful for debugging optimizer behavior and replaying
paper reproductions, but they can be large and may contain sensitive data.

`RawEventPolicy` should support at least:

```rust
pub enum RawEventPolicy {
    None,
    Truncated { max_event_bytes: usize, max_total_bytes: usize },
    Full,
}
```

Default: `Truncated`.

The runtime must truncate deterministically and indicate truncation in the raw
event payload. It must not silently drop all raw provider events when
`supports_raw_provider_events` is true.

---

## 8. Error and Status Mapping

Provider errors should be mapped into `AgentRuntimeError` without losing source
errors.

Suggested provider error enum:

```rust
pub enum CodexError {
    Io(std::io::Error),
    Json(serde_json::Error),
    ConnectionClosed,
    Protocol(String),
    JsonRpc {
        id: String,
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
    UnsupportedServerRequest { method: String },
    ApprovalRequested,
    ProcessExited { status: String, stderr: String },
    Timeout,
}
```

Mapping:

| Provider failure | Leaven result |
|---|---|
| no local mount | `AgentRuntimeError::LocalMountRequired` |
| unsupported Leaven tool policy | `AgentRuntimeError::Policy` |
| provider approval requested while mode is `Error` | `AgentRuntimeError::Policy` or `AgentStatus::Failed`, but must be distinguishable as policy/approval |
| JSON-RPC request failed before turn starts | `AgentRuntimeError::with_source` |
| turn completed with provider error | `AgentSession { status: Failed { reason }, ... }` if transcript exists; otherwise runtime error |
| timeout | interrupt when possible, then `AgentStatus::TimedOut` |
| cancellation | interrupt when possible, then `AgentStatus::Cancelled` |
| output contract failure | `AgentStatus::OutputContractViolation` or `AgentRuntimeError::OutputContract`; choose one policy and keep it uniform |

Recommended policy: transport/protocol failures are errors; completed turns
that failed semantically are `AgentSession` facts with non-success status. That
lets evaluators preserve failed trajectories as evidence.

---

## 9. Cost, Fingerprint, and Reproducibility

The runtime must not invent token costs.

If Codex exposes token/cost usage in protocol metadata, map it into `Cost`.
If not, charge the mechanical `llm_calls` axis for the session and leave token
axes at zero. Preserve provider metadata in raw events. Stages may add external
cost accounting later, but the runtime should not guess token or dollar costs.

`CodexAppServerRuntime::fingerprint()` must include stable execution-shaping inputs:

- provider adapter name and crate version
- transcript normalization version
- app-server connector kind
- connector binary path or configured endpoint identity
- Codex CLI version if cheaply available
- app-server protocol feature/version if available
- model, model provider, reasoning effort, service tier
- sandbox/approval config
- raw event retention policy

It must not include:

- host workspace absolute path
- secret environment values
- transient thread id or turn id
- budget snapshot
- candidate id or graph id

For live provider tests, the default model should be `gpt-5.4-mini` with low
reasoning effort unless a test explicitly overrides it. This keeps real-agent
proofs cheap and aligned with the Leaven operator contract.

---

## 10. Materializers, Parsers, Skills, and Git

Codex sees files. Leaven decides what those files mean.

Leaven cannot leave Codex skill layout ownerless. If Codex app-server expects a
specific workspace layout or `UserInput::Skill` shape, that is app-server
provider ABI and should be recorded in `leaven-agent-codex-app-server`.

The distinction:

```text
Codex provider ABI:
  where Codex-readable skills are mounted
  how a mounted skill is referenced in Codex inputs
  which provider config points Codex at the workspace layout

Skill artifact semantics:
  what a SkillFolder is
  how SKILL.md is parsed and validated
  what changes are legal
  how skill mutations become proposals
```

The first belongs to `leaven-agent-codex-app-server`. The second belongs to
`leaven-artifact-skill`, `leaven-agentic-skill`, or paper-specific stages.

Suggested provider ABI types:

```rust
pub struct CodexWorkspaceLayout {
    pub skills_root: WorkspacePath,
    pub agent_docs: Vec<WorkspacePath>,
}

pub struct CodexSkillRef {
    pub name: String,
    pub path: WorkspacePath,
}
```

These types describe how an already-materialized skill is presented to Codex.
They do not validate, parse, or mutate the skill itself.

`CodexAppServerRuntime` does not materialize a skill bank, clone a repo, parse
`SKILL.md`, or commit changes. Those responsibilities belong to
materializers, parsers, and paper-specific stages.

Typical Codex-backed proposer:

```text
AgenticProposer
  -> skill materializer writes SkillBank to workspace
  -> task/history renderer builds AgentInstructions
  -> CodexAppServerRuntime runs one session
  -> validator checks edited skill folders
  -> workspace proposal parser returns ProposalBatch
```

Typical Codex-backed evaluator:

```text
AgenticEvaluator
  -> artifact materializer writes candidate agent world
  -> task materializer writes evaluation case
  -> CodexAppServerRuntime runs one session
  -> evidence parser reads transcript/files
  -> evaluator returns Assessment
```

If a Codex-compatible layout uses `.agents/skills/<skill-name>/SKILL.md`, that
layout is a Codex provider ABI value. A materializer targets that ABI; the
runtime may use the resulting `CodexSkillRef`s to build `UserInput::Skill`
items, but it must not own `SkillBank` or skill validation.

If a Codex-backed proposer uses git, git readback still belongs to a
stage-owned parser:

```text
workspace diff / commit
  -> GitSnapshotProposalParser
  -> GitChange::AdvanceTo or SkillBankChange
  -> ProposalBatch
```

The runtime returns session facts. It does not decide graph causality.

---

## 11. Laws

### 11.1 Provider containment law

No crate below `leaven-agent-codex-app-server` may import Codex protocol types.

```text
Codex protocol type appears in public API
  => the owning crate must be leaven-agent-codex-app-server or a Codex-specific helper crate
```

### 11.2 Workspace honesty law

If the configured Codex connector needs a local path, it must fail before launch
when `WorkspaceView::local_mount()` is `None`.

No provider adapter may reconstruct a host path by string concatenation or
assume that workspace-relative paths are host filesystem paths.

### 11.3 Request ownership law

`CodexAppServerRuntime` may consume `AgentRunRequest`, but it must not retain borrowed
workspace views, graph views, or stage contexts after `run_session` returns.

### 11.4 Transcript totality law

Unknown provider events never panic transcript normalization. They become raw
events with a stable kind.

### 11.5 Tool policy law

Unsupported stricter policies fail closed. The runtime may be permissive only
when the request policy is permissive or the provider mapping can enforce the
requested restriction.

### 11.6 Output contract law

Successful `AgentStatus::Succeeded` means the requested `OutputContract` passed
runtime-level validation. If the output contract fails, success is forbidden.

### 11.7 Graph purity law

Running Codex cannot mutate Leaven's graph. Only the caller stage can translate
runtime outputs into proposals or assessments through engine stage APIs.

---

## 12. Tests

### 12.1 Unit tests with mock transport

Copy the DSRs mock-transport pattern.

Required tests:

- `initialize` writes `initialize` then `initialized`
- `thread/start` request includes configured cwd/model/approval settings
- `turn/start` request includes task text and context path framing
- streaming assistant deltas accumulates final assistant message
- command execution deltas aggregate into one `CommandRecord`
- unknown notification becomes `RawProviderEvent`
- provider error maps to failed session or runtime error according to the
  chosen policy
- approval request with `CodexApprovalMode::Error` fails closed
- timeout/cancellation interrupts an active turn when possible

### 12.2 Runtime contract tests

Required tests:

- local-mount-required backend returns `LocalMountRequired`
- `Files` output contract fails when a file is missing
- `FinalMessage` output contract fails on empty final assistant text
- `WorkspaceDiff` validates roots but does not parse diffs
- unsupported strict `AgentToolPolicy` returns `Policy`
- raw event truncation is deterministic
- fingerprint changes when model/effort/sandbox config changes
- fingerprint does not include workspace absolute path

### 12.3 Topology tests

Add or extend crate topology tests to prove:

- no non-provider crate depends on `codex-app-server-protocol`
- `leaven-agent-codex --no-default-features` does not enable provider deps
- `leaven-agent-codex-app-server --no-default-features` does not enable provider deps
- umbrella `leaven` does not enable Codex by default
- no umbrella Codex feature exists until the import-experience design explicitly
  names one. Users depend on `leaven-agent-codex-app-server` directly, or on the
  `leaven-agent-codex` facade with a concrete provider feature.

### 12.4 Live tests

Live Codex tests are opt-in:

```text
LEAVEN_CODEX_LIVE=1 cargo test -p leaven-agent-codex-app-server --features live-codex-tests -- --ignored
```

The live smoke defaults to `$HOME/.bun/bin/codex` so it does not accidentally
use a multi-account wrapper that resolves earlier on `PATH`; set
`LEAVEN_CODEX_BIN=/path/to/codex` to override. It requires local Codex auth.

They should be signed/ignored by default and must use a temporary workspace.

Minimum live proof:

```text
materialize temp repo
run Codex with gpt-5.4-mini, low effort
ask it to write a known output file
verify output contract
verify transcript contains command/tool evidence when Codex used tools
verify no graph/engine dependency is involved
```

That provider-adapter proof is separate from an agentic paper gate. A paper gate
may intentionally use `OutputContract::FinalMessage` and no shell tools when the
stage contract is "return a typed proposal/evidence object." In that case the
runtime proof is the live Codex session, developer-instruction bootstrap,
transcript capture, output-contract validation, and durable stage evidence. Tool
and command evidence should be required only for stages that actually grant and
expect tool use.

Git commit/readback live tests belong in the stage/parser crate, not in
`leaven-agent-codex-app-server`.

---

## 13. Implementation Ladder

1. Port DSRs-style `CodexAppServerTransport` into
   `leaven-agent-codex-app-server` behind the `app-server` feature.
2. Port the typed JSON-RPC client methods needed for one-session execution:
   `initialize`, `thread/start`, `turn/start`, `turn/interrupt`,
   `thread/read`, notification streaming, shutdown.
3. Port the history accumulator without DSRs-specific derives or transcript
   templating.
4. Implement `CodexAppServerRuntime::capabilities()` from the configured
   `CodexAppServerConnector`; stdio reports `RequiresLocalMount`.
5. Implement `CodexWorkspaceLayout` and `CodexSkillRef` as provider ABI
   vocabulary.
6. Implement request mapping from `AgentRunRequest` to Codex params.
7. Implement transcript normalization to `AgentTranscript`, `CommandRecord`,
   `ToolCallRecord`, `output_files`, and `RawProviderEvent`.
8. Implement output contract validation.
9. Implement provider error mapping into `AgentRuntimeError` and
   `AgentStatus`.
10. Add mock-transport unit tests and runtime contract tests.
11. Add topology/feature tests.
12. Add one opt-in live Codex smoke.
13. Only after the provider adapter passes, connect it to
    `leaven-agentic` proposer/evaluator tests over real `SkillBank`.

---

## 14. Rejected Designs

### 14.1 Codex-aware `AgentRuntime`

Rejected. `leaven-agent` stays provider-neutral.

### 14.2 Runtime owns skill artifact semantics

Rejected. Codex provider ABI owns provider-specific layout vocabulary, but the
runtime must not parse `SKILL.md`, validate skill folders, or own `SkillBank`.
Skills remain artifact/materializer concerns.

### 14.3 Runtime-owned git commits

Rejected. Git commit, snapshot readback, and artifact proposal construction are
stage parser responsibilities.

### 14.4 First-class thread continuation in v0.2.5

Rejected for the first implementation. The DSRs reference proves it is possible,
but Leaven's generic repair primitive is proposer-owned reproposal. Native
thread continuation should wait for a real user that needs it.

### 14.5 Silent provider policy downgrade

Rejected. If Leaven asks for a stricter tool policy than Codex can enforce, the
runtime fails closed.

---

## 15. Open Questions

1. **Exact Codex tool allowlist mapping.** The app-server protocol has provider
   tool and approval configuration, but the first adapter should not claim an
   exact allowlist until the mapping is tested against real Codex behavior.
2. **Schema validation placement.** `JsonFile` schema validation may belong in
   `leaven-agent` if a lightweight validator is acceptable, or in stage parsers
   if schema validation would add dependency weight.
3. **Provider-native skills input.** Codex `UserInput::Skill` exists. Leaven
   should use it through `CodexSkillRef` values over already-materialized
   paths, not through runtime ownership of `SkillBank`.
4. **Remote app-server transport.** If E2B/Firkin/K8s can run Codex app-server
   inside the sandbox and expose JSON-RPC, add a new
   `CodexAppServerConnector`. Do not fake backend neutrality in
   `StdioCodexAppServerConnector`.
5. **Cost metadata.** If Codex starts emitting stable token usage through the
   app-server stream, map it into `Cost` and add tests. Until then, cost is
   one `llm_calls` charge plus raw metadata, with token axes at zero.
