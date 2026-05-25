# Leaven v0.2.3 - Agentic Stage Runtime and Materialization Contract

> Status: pre-implementation minor spec bump.  
> Date: 2026-05-07.  
> Governing spec: `docs/specs/initial_library.md`.  
> Topology companion: `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`.  
> Stage-materialization companion: `docs/specs/agentic_stage_materialization.md`.  
> Purpose: specify how Leaven runs agents without making the engine, artifacts,
> or cold core know what an agent is.

This document resolves the agentic execution seam that remains after the
renderer/materializer split:

```text
Leaven runs optimizers.
Optimizers own algorithm rhythm.
Stages may run agents.
Materializers make agent-readable worlds.
Renderers build prompt/config values.
Agent runtimes execute one session inside an already-built world.
Parsers translate session outputs or workspace changes back into typed
proposals or assessments.
```

Optimizer-stage workspace setup and query-derived entries are specified in
`docs/specs/agentic_stage_materialization.md`. This document keeps the runtime
split: runtimes execute one session in an already-built workspace; they do not
own optimizer-stage planning, graph reads, or receipt recording.

The main design pressure is agentic optimization over evolving codebases,
skill libraries, harnesses, `AGENTS.md` files, manifests, and traces. The
execution agent may run inside the candidate's world. The reflector, proposer,
judge, or evaluator stage that interprets that run is held fixed unless it is
itself part of the optimized artifact.

---

## 1. Non-Negotiable Split

The engine must not learn a generic theory of agents.

```text
Engine::run()
  -> Optimizer::step(ctx)
      -> Proposer::propose(...)      // may run an agent
      -> Evaluator::evaluate(...)    // may run an agent
          -> Materializer writes workspace files
          -> Renderer builds prompt/config
          -> AgentRuntime executes one session
          -> stage parser reads workspace/session output
          -> returns ProposalBatch or Assessment
```

The answer to "where does the thing run?" is:

```text
inside a concrete proposer or evaluator stage
```

It is not inside the engine, not inside the artifact, and not inside
`AgentRuntime` as an optimizer-aware abstraction.

### 1.1 What each layer knows

| Layer | Knows | Does not know |
|---|---|---|
| `leaven-core` | artifact, proposal, evidence, evaluation vocabulary | workspaces, agents, renderers, graph, surfaces |
| `leaven-workspace` | backend-neutral files/commands/cleanup | artifacts, proposals, evidence, graph, agents |
| `leaven-agent` | one runtime session over a workspace | candidates, assessments, proposals, optimizer rhythm |
| `leaven-engine` | graph, contexts, stages, trust, budget | provider SDK details |
| `leaven-agentic` | reusable adapters from agents to stages | provider internals, optimizer-specific search policy |
| optimizer crate | when and why to call stages | provider-specific runtime mechanics |

### 1.2 Workspace mutation is not graph mutation

An agent may edit files, generate skills, rewrite a harness, create commits, or
leave outputs in a workspace. None of that mutates the run graph by itself.

Durable graph mutation happens only when a stage returns typed proposal or
assessment data:

```text
workspace/session output
  -> stage parser
  -> ProposalEffect::Create or ProposalEffect::Change
  -> RunContext::apply_proposal/apply_batch
```

Similarly, an agentic evaluator may generate logs, traces, and files, but the
graph records only the returned `Assessment`s, costs, events, and error records.

---

## 2. Semantic Artifact vs Operational Layout

Agentic optimization hides two different maps:

```text
semantic map:
  this harness uses these skills
  this skill is enabled for these task families
  this harness expects skill ABI v3
  this AGENTS.md changes the candidate's behavior

operational map:
  write harness/main.py to /workspace/harness/main.py
  write skills to /workspace/skills/*.md
  write manifest to /workspace/agent.toml
  run python harness/main.py --task task/input.json
```

The rule:

```text
if changing it changes the candidate being optimized,
  put it in the Artifact.

if changing it only changes how a consumer sees/runs the candidate,
  put it in Renderer, Materializer, or stage config.
```

### 2.1 Composite agent artifacts

Agentic optimizers will often optimize a composite artifact:

```rust
pub struct AgentKit {
    pub harness: HarnessArtifact,
    pub skills: SkillDir,
    pub manifest: AgentManifest,
    pub agent_docs: AgentDocs,
}

pub struct AgentManifest {
    pub entrypoint: HarnessEntrypoint,
    pub enabled_skills: Vec<SkillId>,
    pub skill_mounts: BTreeMap<SkillId, SkillMount>,
    pub tool_policy: ToolPolicyName,
    pub skill_abi: SkillAbiVersion,
}
```

This is semantic state. If the optimizer can enable a skill, change a mount,
change the entrypoint, update `AGENTS.md`, or rewrite a harness, that value is
part of the candidate.

An edit surface over that artifact can expose targetable parts:

```rust
pub enum AgentKitPartId {
    Harness,
    Skill(SkillId),
    Manifest,
    AgentDocs,
}

pub enum AgentKitChange {
    Harness(HarnessChange),
    Skill {
        skill: SkillId,
        change: SkillChange,
    },
    Manifest(ManifestChange),
    AgentDocs(AgentDocsChange),
    Atomic(Vec<AgentKitChange>),
}
```

GEPA, TextGrad-like methods, skill-library optimizers, and harness optimizers
can then route blame or edits to the harness, a skill, the manifest, or the
agent docs without the engine knowing that these concepts exist.

### 2.2 Operational layout is a materializer decision

A materializer projects the composite artifact into a workspace ABI:

```text
/workspace/
  agent.toml
  AGENTS.md
  harness/
    main.py
  skills/
    refactor.md
    pytest-debugging.md
  task/
    input.json
  output/
```

That layout is not the optimized object. It is the presentation of the
optimized object to a specific runtime or subprocess.

```rust
pub struct AgentKitMaterializer {
    pub layout: AgentWorkspaceLayout,
}

impl<P> Materializer<P, AgentKit> for AgentKitMaterializer
where
    P: OptimizationProblem<Artifact = AgentKit>,
{
    async fn materialize_into(
        &self,
        kit: &AgentKit,
        ws: &mut WorkspaceView<'_>,
        ctx: MaterializeContext<'_, P>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        self.write_manifest(&kit.manifest, ws, &ctx).await?;
        self.write_harness(&kit.harness, ws, &ctx).await?;
        self.write_skills(&kit.skills, &kit.manifest, ws, &ctx).await?;
        self.write_agent_docs(&kit.agent_docs, ws, &ctx).await?;

        Ok(Metered::new(
            MaterializationReport::default(),
            Cost::zero(),
        ))
    }
}
```

The materializer may compose smaller materializers, but it must still respect
the caller's read scope. Hidden partitions, hidden traces, and hidden case
content must not be written into the workspace.

---

## 3. Workspace Semantics

`Workspace` is a Leaven-owned lease handle. Users implement or choose
`WorkspaceFactory` and `WorkspaceBackend`; ordinary stage code uses a concrete
`Workspace`/`WorkspaceView` API.

The backend-neutral law:

```text
Materializer and AgentRuntime code should use WorkspacePath, write_file,
read_file, list_files, executable-bit helpers, and run_command. They should not
require host PathBufs.
```

### 3.1 Local backend scenario

```text
factory.allocate()
  -> tempdir on host

WorkspacePath("skills/foo.md")
  -> /tmp/leaven-.../skills/foo.md

write_file
  -> tokio::fs::write

run_command
  -> local child process with cwd at workspace root

local_mount()
  -> Some(tempdir.path())

cleanup()
  -> remove tempdir or git worktree
```

Local workspaces are useful for tests and trusted local development. They are
not a security boundary. A malicious local process can often reach outside its
working directory unless the runtime or operating system sandbox prevents it.

### 3.2 E2B-style backend scenario

```text
factory.allocate()
  -> one remote sandbox lease

WorkspacePath("skills/foo.md")
  -> /workspace/leaven/<run>/skills/foo.md inside sandbox

write_file
  -> sandbox file API

read_file
  -> sandbox file API

run_command
  -> sandbox command API with cwd at workspace root

local_mount()
  -> None

cleanup()
  -> kill sandbox or release it to a pool
```

The same `AgentKitMaterializer` should work for local and E2B-style backends.
If a runtime needs a host path, it is not backend-neutral and must declare that
capability requirement explicitly.

### 3.3 Runtime capability matching

Some agent runtimes can operate only through `WorkspaceView` operations. Others
may require a local filesystem mount because the upstream provider attaches to
a host checkout.

The runtime contract must make this visible:

```rust
pub enum WorkspaceAccessMode {
    /// Uses only WorkspacePath, file APIs, and run_command.
    BackendNeutral,

    /// Requires Workspace::local_mount() to return Some.
    RequiresLocalMount,

    /// Provider owns the sandbox and Leaven only exchanges files/metadata
    /// through the provider adapter.
    ProviderManaged,
}

pub struct AgentRuntimeCapabilities {
    pub workspace_access: WorkspaceAccessMode,
    pub supports_commands: bool,
    pub supports_raw_provider_events: bool,
}
```

An adapter with `RequiresLocalMount` may be useful, but it must fail early with
`AgentRuntimeError::LocalMountRequired` if paired with E2B, K8s, Firecracker,
or any backend whose `local_mount()` is `None`.

### 3.4 Harbor-style command-backed provider path

The default product path for provider CLIs is command-backed execution inside
the workspace backend:

```text
stage allocates workspace
  -> materializer writes artifact/task/data into workspace
  -> provider runtime writes native config, home, skill, or MCP setup files
  -> provider runtime calls WorkspaceView::run_command(...)
  -> backend runs the provider CLI in the requested workspace cwd
  -> runtime captures stdout/stderr/native logs/session files
  -> runtime validates OutputContract
  -> stage parser turns session/workspace output into proposal or evidence
```

This is the path used for container, E2B, K8s, Firkin, Firecracker, and local
execution when a provider can be launched as a CLI process in the candidate
world. It is backend-neutral because the runtime never asks for a host path; it
only asks the workspace backend to write files, run commands, and read files
back.

Provider setup files are operational presentation, not artifact state:

```text
.leaven/codex-home/config.toml
.agents/skills/<skill>/SKILL.md
.mcp.json
provider session jsonl
native stdout/stderr logs
```

They become optimized artifact state only when they are materialized from the
candidate artifact or parsed back into a typed proposal. A runtime may create
or modify operational files during execution, but those mutations do not enter
the run graph until a stage parser explicitly translates them.

The command-backed path is deliberately separate from app-server-over-stdio
paths. A stdio app-server connector can be a useful local adapter, but it is
`RequiresLocalMount`; it is not the container default and must not be hidden
behind a backend-neutral runtime claim.

### 3.5 Command execution laws

`WorkspaceBackend::run_command` is a capability with stronger laws than "spawn
a process":

- `cwd` is always a `WorkspacePath` resolved by the backend, never a host path
  supplied by a stage.
- `env` values are passed to the child process and are not written into the
  workspace unless stage code explicitly writes them.
- `stdin` is explicit. Empty stdin and byte stdin are distinguishable.
- timeouts and output limits are enforced by the backend or rejected with a
  typed unsupported error before an unbounded run can start.
- if a requested `CommandUser` cannot be honored, the backend returns an
  unsupported-operation error instead of silently running as the wrong user.
- captured stdout and stderr preserve bytes plus a truncation flag.
- duration and exit status are session facts and should be retained in
  transcript/evidence surfaces when available.

---

## 4. Provider-Neutral `AgentRuntime`

`AgentRuntime` executes one session inside an already-materialized workspace.
It does not know what Leaven is optimizing.

```rust
pub trait AgentRuntime: Send + Sync {
    fn id(&self) -> AgentRuntimeId;

    /// Stable identity for cache/debug/replay accounting. Includes provider,
    /// runtime version, model defaults, tool adapter version, and policy defaults
    /// that affect behavior.
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

The trait lives in `leaven-agent`. That crate may depend on `leaven-kernel` and
`leaven-workspace`, but not on `leaven-engine`, `leaven-core`,
`leaven-gepa`, or optimizer crates.

### 4.1 What `AgentRuntime` must not know

`AgentRuntime` must not mention:

```text
OptimizationProblem
Artifact
CandidateId
Proposal
ProposalBatch
Assessment
EvaluationSet
RunGraph
RunContext
Population
GEPA
SkillDir
HarnessArtifact
```

If a runtime needs to know those things, the adapter is at the wrong layer. Put
that knowledge in an agentic proposer/evaluator stage or in a parser.

### 4.2 Run request

`AgentRunRequest` is execution vocabulary, not optimizer vocabulary.

```rust
pub struct AgentRunRequest {
    pub instructions: AgentInstructions,
    pub cwd: WorkspacePath,
    pub output_contract: OutputContract,
    pub env: BTreeMap<String, String>,
    pub tool_policy: AgentToolPolicy,
    pub limits: AgentLimits,
}

pub struct AgentInstructions {
    pub system: Vec<String>,
    pub task: String,
    pub context: Vec<AgentContextRef>,
}

pub enum AgentContextRef {
    InlineText {
        label: String,
        text: String,
    },
    WorkspacePath {
        label: String,
        path: WorkspacePath,
    },
}
```

There is intentionally no universal `Rendered` enum in core or engine.
`AgentInstructions` is a small `leaven-agent` execution type because agent
runtimes need a portable prompt/config shape. More specialized provider crates
may translate it into provider-native messages or CLI arguments.

### 4.3 Output contract

The output contract says what the runtime should leave behind. It does not say
how to convert that output into Leaven proposals or assessments.

```rust
pub enum OutputContract {
    Files {
        required: Vec<WorkspacePath>,
        optional: Vec<WorkspacePath>,
    },
    JsonFile {
        path: WorkspacePath,
        schema: Option<JsonSchemaRef>,
    },
    FinalMessage,
    WorkspaceDiff {
        root: WorkspacePath,
    },
}
```

Import is stage-owned:

```text
OutputContract::JsonFile("output/proposals.json")
  -> ProposalParser<P>
  -> ProposalBatch<P>

OutputContract::Files {
    paths: vec![
        WorkspacePath::new("output/evidence.json")?,
        WorkspacePath::new("output/transcript.json")?,
    ],
}
  -> EvidenceParser<P>
  -> Vec<Assessment<P>>

OutputContract::WorkspaceDiff(".")
  -> ProposalParser<P, I>
  -> ProposalBatch<P>
```

`OutputContract::WorkspaceDiff` does not make the runtime understand Leaven
artifacts. It only tells the runtime/stage that changed workspace state is an
expected output. The stage-owned proposal parser translates those changes into
typed proposal data.

### 4.4 Run context

`AgentRunContext` is not `RunContext`. It carries execution facts that are
safe for provider-neutral runtime code:

```rust
pub struct AgentRunContext<'a> {
    pub trace_id: TraceId,
    pub deadline: Option<Deadline>,
    pub budget_snapshot: BudgetSnapshot,
    pub cancellation: CancellationRef<'a>,
}
```

The runtime reports cost in `Metered<AgentSession>`. The owning proposer or
evaluator charges that cost to the engine ledger through its stage context.
This keeps `leaven-agent` independent of `leaven-engine`.

### 4.5 Session result

```rust
pub struct AgentSession {
    pub status: AgentStatus,
    pub transcript: AgentTranscript,
    pub commands: Vec<CommandRecord>,
    pub output_files: Vec<WorkspacePath>,
    pub raw_provider_events: Vec<RawProviderEvent>,
}

pub enum AgentStatus {
    Completed,
    Failed { message: String },
    TimedOut,
    Cancelled,
    PolicyViolation { message: String },
    OutputContractViolation { message: String },
}
```

The session is fact, not interpretation. A failed agent session may still be
valuable evidence; the evaluator decides whether to return an assessment, and
the proposer decides whether any proposal can be parsed.

### 4.6 Fingerprint and replay law

For audit and cache policy, the stage event must record:

```text
runtime id
runtime fingerprint
request fingerprint
workspace backend fingerprint if available
tool policy fingerprint
output contract fingerprint
```

This does not make agent runs deterministic. It makes nondeterminism traceable.
Agentic stages default to no evaluation cache unless the evaluator explicitly
declares a stronger cache policy.

---

## 5. Agentic Stage Adapters

`leaven-agentic` provides reusable glue. It may depend on `leaven-engine`,
`leaven-agent`, and `leaven-workspace` because its job is to adapt agent
sessions into stage outputs. Shared render/materializer code should return only
as a behavior-bearing crate with tests and topology rows.

### 5.1 Agentic proposer

```rust
pub struct AgenticProposer<Factory, R, M, Prompt, Parse> {
    pub runtime: R,
    pub workspace_factory: Factory,
    pub workspace_config: WorkspaceConfig,
    pub materializer: M,
    pub prompt_renderer: Prompt,
    pub parser: Parse,
}
```

Canonical flow:

```text
request
  -> build stage input from scoped graph view
  -> allocate workspace with the stage-owned WorkspaceFactory
  -> materialize allowed artifact/history/traces into workspace
  -> render AgentInstructions
  -> runtime.run_session(...)
  -> parser translates session or workspace output
  -> return Metered<ProposalBatch<P>>
```

Sketch:

```rust
impl<P, R, M, Prompt, Parse> Proposer<P> for AgenticProposer<R, M, Prompt, Parse>
where
    P: OptimizationProblem,
    R: AgentRuntime,
    M: Materializer<P, AgenticProposalInput<P>>,
    Prompt: Renderer<P, AgenticProposalInput<P>, AgentPromptTarget, View = AgentInstructions>,
    Parse: ProposalParser<P>,
{
    type Request = AgenticProposalRequest;

    async fn propose(
        &self,
        request: Self::Request,
        mut ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError> {
        let input = self.build_input(&request, ctx.graph())?;

        with_workspace(
            self.workspace_factory.as_ref(),
            self.workspace_config.clone(),
            |ws| Box::pin(async move {
                let mut view = ws.view();

                let materialized = self.materializer
                    .materialize_into(&input, &mut view, ctx.materialize_context())
                    .await?;

                let instructions = self.prompt_renderer
                    .render(&input, AgentPromptTarget, ctx.render_context())
                    .await?;

                let session = self.runtime
                    .run_session(
                        &mut view,
                        AgentRunRequest {
                            instructions: instructions.value,
                            cwd: WorkspacePath::root(),
                            output_contract: OutputContract::JsonFile {
                                path: WorkspacePath::new("output/proposals.json")?,
                                schema: None,
                            },
                            env: BTreeMap::new(),
                            tool_policy: AgentToolPolicy::default(),
                            limits: AgentLimits::default(),
                        },
                        AgentRunContext::new(AgentSessionId::new(), &ctx.budget()),
                    )
                    .await?;

                let batch = self.parser
                    .parse_proposals(&mut view, &session.value, &request, ctx.graph())
                    .await?;

                Ok(Metered::new(
                    batch,
                    materialized.cost
                        .checked_add(&instructions.cost)?
                        .checked_add(&session.cost)?,
                ))
            }),
        )
        .await
        .map_err(ProposalError::from)
    }
}
```

The exact implementation may avoid holding a graph borrow across `.await` by
turning `input` into an owned snapshot before workspace allocation. That is the
preferred implementation style.

The public `with_workspace` helper remains the recommended shape for simple
scoped workspace use. Agentic adapters may inline the same acquire/result/
cleanup pattern when the async body captures stage state or graph-context
borrows that make a higher-rank closure hostile to Rust's borrow checker.

### 5.2 Agentic evaluator

```rust
pub struct AgenticEvaluator<Factory, R, M, Prompt, Parse> {
    pub runtime: R,
    pub workspace_factory: Factory,
    pub workspace_config: WorkspaceConfig,
    pub materializer: M,
    pub prompt_renderer: Prompt,
    pub parser: Parse,
}
```

Canonical flow:

```text
resolved evaluation request
  -> choose candidate/case work units
  -> allocate one workspace per unit or per batch
  -> materialize candidate artifact plus case input
  -> render AgentInstructions
  -> runtime.run_session(...)
  -> parser turns transcript/files into Evidence
  -> return Vec<Assessment<P>>
```

The evaluator is where "run the candidate agent on this task" belongs. If the
candidate includes a harness, skills, and `AGENTS.md`, the evaluator materializes
that candidate and invokes the runtime. The execution agent sees candidate-local
instructions because they were written into the workspace as part of the
candidate.

The evaluator or judge agent that parses and scores the trajectory is outside
the candidate unless it is explicitly part of the artifact. Its prompt, model,
and parser are fixed stage configuration and contribute to the evaluator
fingerprint.

### 5.3 Parser Ownership

Parsers are separate from runtimes:

```rust
pub trait ProposalParser<P: OptimizationProblem>: Send + Sync {
    async fn parse_proposals(
        &self,
        workspace: &mut WorkspaceView<'_>,
        session: &AgentSession,
        request: &AgenticProposalRequest,
        graph: RunGraphView<'_, P>,
    ) -> Result<ProposalBatch<P>, ProposalParseError>;
}

pub trait EvidenceParser<P: OptimizationProblem>: Send + Sync {
    async fn parse_evidence(
        &self,
        workspace: &mut WorkspaceView<'_>,
        session: &AgentSession,
        unit: &EvaluationWorkUnit,
    ) -> Result<Vec<Assessment<P>>, EvidenceParseError>;
}

```

The same `ProposalParser` trait covers structured-output parsers and workspace
mutation parsers. Concrete parser types should say what they interpret, for
example `JsonProposalFileParser`, `SkillBankWorkspaceProposalParser`, or
`GitSnapshotProposalParser`. Do not add a second generic workspace-readback
trait unless a later implementation exposes real polymorphism and independent
laws that `ProposalParser` cannot express.

This parser seam prevents provider-specific runtime code from knowing Leaven
optimizer types.

---

## 6. Trust Boundary

Trust is enforced by what the stage receives and what materializers write.

```text
ProposerContext read scope
  -> RenderContext / MaterializeContext inherit same read scope
  -> materializer can only write visible graph/artifact/evidence data
  -> AgentRuntime sees only workspace files and AgentRunRequest
```

This keeps clean GEPA-style validation honest:

```text
reflective proposer may see feedback/minibatch traces
reflective proposer must not see validation/test case content
candidate selector may use exposed validation scores if policy allows
execution agent may see candidate-local skills/docs because those are artifact state
```

### 6.1 Local runtime caveat

For local backends, Leaven's trust boundary prevents accidental graph-data leaks
through materializers and renderers. It does not make an untrusted local process
secure. If the runtime can run arbitrary host commands, host-level isolation is
the responsibility of the workspace backend and runtime policy.

Use E2B, Docker, K8s, Firecracker, or another isolated backend for untrusted
agents.

---

## 7. Borrow-Friendliness and Stability Laws

Agentic stages should be easy to implement without lifetime contortions.

### 7.1 Owned request law

Stage requests are owned and lightweight. They identify what to do:

```text
candidate id
case ids
parent ids
proposal count
output target
```

They should not carry borrowed graph views or borrowed artifacts across `.await`.
Stages build owned snapshots from scoped graph views before external work.

### 7.2 One mutable workspace borrow law

A stage has one mutable workspace lease. Materializers, runtimes, and parsers
borrow a `WorkspaceView<'_>` from that lease. The workspace is cleaned up by
`with_workspace` or explicit `cleanup().await` before the stage returns.

### 7.3 Cost accounting law

`AgentRuntime` returns `Metered<AgentSession>`. The owning stage combines:

```text
materialization cost
rendering cost
agent runtime cost
parsing cost if any
```

using checked cost arithmetic, then returns a metered stage result. The context
charges the authoritative budget ledger.

### 7.4 No erased renderer/materializer law

No `DynRenderer`, `DynMaterializer`, or universal `Rendered` enum is introduced
for this contract. Stage-owned typed fields are the implementation path. Add
erasure only after a real registry user exists and the erased target/view
contract is testable.

### 7.5 Runtime purity law

`AgentRuntime` may be side-effectful inside the workspace, but it is pure with
respect to Leaven's graph:

```text
same workspace state + same AgentRunRequest + same runtime config
  -> one AgentSession fact record
```

It may be nondeterministic, but nondeterminism is reported through session
status, transcript, raw provider events, fingerprinting, and cost.

---

## 8. Crate Responsibilities

```text
leaven-agent
  AgentRuntime
  AgentRunRequest
  AgentInstructions
  AgentSession
  AgentTranscript
  OutputContract
  runtime capability declarations

provider-specific runtime crates, starting with leaven-agent-codex-cli
  provider-specific AgentRuntime impls
  provider config
  transcript normalization
  provider error mapping

leaven-agentic
  AgenticProposer
  AgenticEvaluator
  ProposalParser / EvidenceParser helpers
  common agent materializer compositions
  runtime capability checks

leaven-workspace
  Workspace
  WorkspaceView
  WorkspaceFactory
  WorkspaceBackend
  WorkspacePath
  Command / CommandOutput

future render/materializer crate
  reusable Renderer and Materializer impls, if behavior-bearing
  no provider runtime logic
```

Forbidden dependencies:

```text
leaven-agent -> leaven-engine
leaven-agent -> leaven-core
leaven-agent -> leaven-gepa
leaven-workspace -> leaven-agent
leaven-workspace -> leaven-engine
leaven-core -> leaven-workspace
```

---

## 9. Implementation Order

The first implementation slice should prove the shape before adding real
provider adapters:

1. Make `leaven-workspace` backend-neutral: delegate file operations and
   `run_command` through `WorkspaceBackend`; keep `local_mount()` optional.
2. Define the narrow `leaven-agent` runtime vocabulary and a deterministic fake
   runtime for tests.
3. Implement `leaven-agentic::AgenticProposer` and
   `leaven-agentic::AgenticEvaluator` against the fake runtime.
4. Move one milestone example from handwritten deterministic plumbing to the
   agentic adapters.
5. Add the real Codex CLI provider adapter from
   `docs/specs/codex_cli_agent_runtime.md` in its own provider crate. Keep
   app-server support in `leaven-agent-codex-app-server` as a separate local
   compatibility adapter.
6. Pair provider adapters with capability tests that prove local-vs-remote
   workspace behavior fails early when unsupported.

Do not start with a provider SDK. Start with the contract and a fake runtime.

---

## 10. Rejected Designs

### 10.1 Generic `AgentRuntime<P>`

Rejected because it makes the runtime optimizer-aware. Runtime code should not
know `OptimizationProblem`, `CandidateId`, `ProposalBatch`, or `Assessment`.

### 10.2 Artifact-owned execution

Rejected because artifacts are candidate state. Running a candidate is an
evaluator/proposer concern, not a method on the artifact.

### 10.3 Engine-owned agent execution

Rejected because the engine does not know why an agent is being run. Proposers
and evaluators decide the purpose, output contract, parsing, and trust scope.

### 10.4 Runtime-owned materialization

Rejected because "these skills map to this harness" is either semantic artifact
state or materializer layout policy. Provider runtimes should not learn Leaven
artifact relationships.

### 10.5 Host-path-first workspace APIs

Rejected because E2B, K8s, Firecracker, and provider-managed sandboxes may have
no host path. `local_mount()` is an optimization or compatibility requirement,
not the primary workspace interface.

---

## 11. Final Thesis

The durable split is:

```text
Optimizer decides when.
Stage decides why.
Materializer decides what world to build.
Renderer decides what to say.
AgentRuntime decides how to execute.
Parser decides what came back.
RunContext records the truth.
```

This is the narrowest contract that supports agentic optimization over evolving
codebases, skill libraries, harnesses, and agent instruction files without
turning Leaven into an agent framework or forcing optimizer semantics into
runtime adapters.
