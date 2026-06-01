# Leaven — Agentic Reflection and Artifact Workspaces

Date: 2026-05-16
Status: companion spec (pre-implementation roadmap), shelved. States durable
direction; active implementation is deferred to revisit at library version
v0.0.2-alpha. Near-term work does the Phase 1 routing swap only — see §0.1. A
design contract, not proof that code exists; verify current code before routing
work from it.
Governing spec: `docs/specs/initial_library.md`.
Evolves: `docs/specs/agentic_stage_materialization.md` (layer B) — see §6.
Companions: `docs/specs/agentic_stage_runtime.md`,
`docs/specs/gepa_reference_behavior.md`,
`docs/specs/gepa_reflection_evidence_visibility.md`.

## 0. Why this spec exists

Leaven's stated design pressure is agentic optimization over evolving
codebases, skill libraries, harnesses, `AGENTS.md` files, and manifests. The
GEPA reflector is the first stage where a real agent should run.

The GEPA agent-reflection slot is wired and routes, but it does not yet work as
a reflector: the agent never receives the artifact it is asked to improve. This
spec states the intended end state, records the honest current state, and
sequences the work to close the gap. It is the durable home for "what agentic
reflection is for" and "what is left to build."

### 0.1 Scope and timing

This spec is shelved as active work. It is the durable record of where agentic
reflection is going — not a current task list.

Near-term, the only change in scope is Phase 1 (§4): routing GEPA agent
reflection through a materializing agentic proposer for skill-bank artifacts.
Everything past that — receipts, the `leaven` CLI, the git/jj artifact crates,
full path convergence — is deferred and revisited at library version
v0.0.2-alpha.

The vision in §1 and the architecture in §3 stand. They are the target. They
are just not the current sprint.

The typed `ArtifactReflector` / `ReflectionWorkspace` vocabulary used by
Phase 1 is owned by `docs/specs/typed_signature_adapter_contract.md`. This
spec retains the Phase 1-N narrative; the implementation contract lives there.

## 1. Vision

A GEPA reflection step can run a real agent inside a workspace that holds the
**actual artifact** being optimized:

```text
optimizer selects a parent candidate + a part
  -> the parent artifact is materialized into a workspace
     (a skill bank as skills/*.md; a repo as a working tree)
  -> the reflective dataset (examples) is presented to the agent
  -> the agent edits the artifact in place, with its own tools
  -> the edited workspace is read back as a typed artifact Change
  -> the Change becomes a ProposalEffect::Change through RunContext
```

The agent reflector and the LM reflector are byte-identical in the data they
are *given* (`gepa_reference_behavior.md` reflective-dataset law); they differ
only in *how* that data is presented — a prompt string versus a materialized
workspace.

The artifact families this must serve, in intended order of arrival:

1. **Skill banks** — `leaven-artifact-skill`. The first real target.
2. **Git and Jujutsu repositories** — `leaven-artifact-git`,
   `leaven-artifact-jj`. The headline use case: optimizing a codebase by
   letting an agent edit a checked-out working tree.
3. **Composite agent kits** — harness + skills + manifest + `AGENTS.md`, per
   `agentic_stage_runtime.md` §2.1.

"The agent edits a repo and we read the diff back as a typed change" is the
target. Everything below is what stands between here and there.

## 2. Current state (honest)

This section is reality as of 2026-05-16, verified against code. It is not the
target.

### 2.1 The reflection slot is real

- `GepaReflector<P, S>` (`leaven-gepa/src/proposer.rs`) is the swappable
  reflector trait. Implementors: `FixedSurfaceEdit` (scaffold fixture),
  `LmBackedReflector` (real), and agentic bridge crates that materialize the
  artifact before reflection.
- `ReflectRequest` carries `parent: CandidateId`, the selected `part`, and
  `Vec<ReflectiveExample>`. `ReflectiveDatasetBuilder` builds the examples once
  per step; this is the Rust analogue of GEPA's `make_reflective_dataset`.
- The optimizer builds the request once and passes it to whichever reflector is
  configured (build-once-pass-down).

### 2.2 The agent reflector cannot see the artifact

The deleted `gepa_stage_proposer` route wired the agent reflector through
`leaven-stage`'s `AgentBacked`. Its lifecycle was bootstrap →
`setup_stage_workspace` → `StageReadAuthority::prewarm` → run → parse. Two
facts broke reflection:

1. **`AgentStagePlan` has no materializer.** Its fields are `role`, `request`,
   `directive`, `query`, `output`, `metadata`. Nothing materializes the
   artifact.
2. **`StageReadAuthority` renders graph metadata, not artifact content.** A
   `StageQuery::Candidate` is rendered as JSON of `{id, origin, identity,
   created_at}`. No `StageQuery` variant returns artifact body; every kind is an
   `EntryProjection::Summary`.

Consequence: that agent reflector received the reflective examples and candidate
*metadata*, but never the current artifact — the `<curr_param>` half of GEPA's
own reflection prompt. The LM reflector, by contrast, reads
`ctx.graph().artifact(parent)` directly. The old `agent_stage_routing` test did
not catch this: examples were not the artifact, and the `FakeAgentRuntime` was
scripted to emit output regardless of input.

### 2.3 The interactive query path has no transport

`StageQueryPolicy` distinguishes `prewarm` queries (adapter-run, before the
session) from agent-issued `leaven_query` calls (gated by `allowed` /
`max_queries`). Only prewarm fires today: `AgentBacked::run_attempt` never calls
`StageReadAuthority::query` during the session, and the agent subprocess has no
way to reach the authority. Agent-issued `leaven_query` requires a `leaven` CLI
(or query server) in the workspace that does not exist.

So `AgentBacked`'s interactive-context machinery — its main justification over a
simpler proposer — is currently unreachable. In practice `AgentBacked` reduces
to "write metadata files, run, parse."

### 2.4 A second agentic proposer path exists, with the missing half

`leaven-agentic`'s `AgenticProposer<Factory, Runtime, Materialize, Render,
Parse, Input>` is a `Proposer` whose request *carries the artifact value*. It
calls `Materializer::materialize_into` — it materializes the real artifact —
then renders, runs, and parses. It lacks `StageAttemptReceipt` recording and the
graph-scoped read authority.

The two agentic proposer paths each hold half of a correct agent reflector:

```text
AgentBacked      graph-native request, read authority, receipts   | NO materializer
AgenticProposer  real Materializer, read-back parser              | NO receipts, NO authority
```

### 2.5 Artifact crate maturity gates the repo use case

- `leaven-artifact-skill` — **real.** `impl Artifact for SkillBank`, three
  `EditSurface` impls, proof-anchor test. `leaven-agentic-skill` ships
  `SkillBankMaterializer` and `SkillBankWorkspaceProposalParser`.
- `leaven-artifact-git` — **behavior-bearing artifact vocabulary.** It owns
  normalized paths, refs, object ids, diffs, ref lineage, and Git program
  artifact records. It still has no edit-surface implementation, workspace
  backend, or Git command execution.
- `leaven-artifact-jj` — **behavior-bearing materializable snapshot
  vocabulary.** `JjArtifact` writes its file map into a workspace slot, derives
  content/cache identity from the file map, and reads back
  `.leaven/jj/change.patch` as `JjChange::Patch`. It still has no operation-log
  handling, conflict parser, JJ surface projection, or workspace execution.

Repo integration testing is therefore blocked on the missing edit surfaces,
workspace execution, receipt/read-authority convergence, and reflection wiring,
not on empty artifact placeholder crates.

## 3. Target architecture

### 3.1 One agentic stage proposer

The optimizer agentic stage proposer must carry all four properties:

```text
graph-native request   parent referenced by CandidateId, resolved by the optimizer
real materialization    the artifact is written into the workspace via a Materializer
typed read-back         the edited workspace is parsed into a typed artifact Change
durable receipts        each attempt records a StageAttemptReceipt for resume
```

`AgentBacked` and `AgenticProposer` converge into this single path. Neither
half-path survives as a parallel product surface (§7 governs the interim).

### 3.2 Reflection as a typed `ArtifactReflector`

The reflector is handed a typed value, not just graph ids. The shape is
expressed as an `ArtifactReflector` impl (see
`docs/specs/typed_signature_adapter_contract.md` §2):

```rust
pub trait ArtifactReflector: Send + Sync {
    type Input: Send + Sync;
    type Change: Send + Sync;
    type Error: std::error::Error + Send + Sync + 'static;

    fn reflection_id(&self) -> &'static str;
    fn project<'a>(&'a self, input: &'a Self::Input, view: &'a mut WorkspaceView<'_>)
        -> impl Future<Output = Result<(), Self::Error>> + Send + 'a;
    fn read_back<'a>(&'a self, input: &'a Self::Input, view: &'a WorkspaceView<'_>, session: &'a AgentSession)
        -> impl Future<Output = Result<ReadbackResult<Self::Change>, Self::Error>> + Send + 'a;
}
```

Per-artifact crates own `Input` / `Change` types and the impl. The generic
`ReflectionWorkspace` runner (also in
`docs/specs/typed_signature_adapter_contract.md` §2) owns the workspace
transaction: write `TASK.md` / `MANIFEST.json` / `AGENTS.md`, call `project`,
lay out `cases/`/`cross_case/` per layout config, run the agent session via
`AgentRuntime`, call `read_back`, harvest session attachments, clean up.

The optimizer loop is graph-connected; resolving `parent -> artifact` and
packing the typed `Input` is the same move it already makes to build the
reflective dataset.

### 3.3 Materialization is the artifact's decision

How an artifact lands in the workspace is owned by the artifact, not the stage:

- A skill bank materializes as `skills/*.md` + manifest.
- A git/jj repo materializes as a checked-out working tree.
- A composite agent kit materializes its harness/skills/manifest layout.

The stage supplies a workspace and a `Materializer`; the artifact crate owns
the projection and the inverse read-back (`read_back_change`). Leaven currently
has two materialization traits — `leaven_engine::Materializer<P, T>` and
`leaven_stage::MaterializableArtifact`. Convergence must pick one seam; this
spec does not yet mandate which (§8).

The `ReflectionWorkspace` runner composes over the artifact's `project` and
`read_back` methods. The artifact crate continues to own how its content lands
on disk under `target/current/` and how the diff reads back into a typed
`Change`. The runner owns the rest: `MANIFEST.json` writing,
`cases/`/`cross_case/` layout, transcript capture, workspace cleanup, and
progressive trace disclosure.

### 3.4 Trust boundary

For reflection, hidden-data filtering happens at **dataset-build time** — the
`ReflectiveCaseInput` projection in `gepa_reflection_evidence_visibility.md`
already guarantees reflection never sees hidden targets. The agent then sees
exactly the materialized footprint and nothing else. Static materialized
context *is* the trust boundary; it does not require a query-time authority.

Interactive `leaven_query` is an **additive** capability for stages that want
the agent to explore graph state. It is not required for GEPA-faithful
reflection and must not be on reflection's critical path.

## 4. Work breakdown

Phases are ordered. Earlier phases do not depend on later ones.

### Phase 1 — The swap (skill banks) — near-term

Route GEPA agent reflection through a materializing agentic proposer.

- Add `ArtifactReflector` trait + `ReflectionWorkspace` runner +
  `ReadbackResult` + `ReflectionLayoutConfig` + `ReflectionError` to
  `leaven-agentic` (see `docs/specs/typed_signature_adapter_contract.md` §2
  for the public surface).
- Implement `SkillBankReflector: ArtifactReflector` in
  `leaven-gepa-agentic-skill`. `project` materializes the skill bank under
  `target/current/<skill-name>/<path>`. `read_back` diffs the tree, validates
  the resulting `SkillBank`, returns a typed `SkillBankChange` inside
  `ReadbackResult::Valid` or `ReadbackResult::Invalid` with diagnostics when
  the agent's edits break the contract.
- Promote `ReflectiveExample` to `ReflectiveCase { runs: Vec<ReflectiveRun> }`
  + `ReflectiveValue` + `Checks` in `leaven-gepa::reflection` (see
  `gepa_reflection_evidence_visibility.md` §3). Add `Attachment` +
  `AttachmentKind` to `leaven-evidence` per
  `docs/specs/typed_signature_adapter_contract.md` §3.2; `leaven-gepa`
  re-exports as `leaven_gepa::Attachment` for ergonomics.
- Collapse `leaven-gepa-agentic-skill`'s bespoke `renderer.rs` /
  `materializer.rs` / `parser.rs` into the single `ArtifactReflector` impl.
- The resulting proposal is `ProposalEffect::Change` with `informed_by`
  computed by Leaven, not by the agent, from the reflect-request's
  `source_refs` plus per-case / per-run refs carried in the reflective dataset.
- Tests: an end-to-end agentic reflection test over a skill bank, plus the
  `leaven doctor proposal-roundtrip --json` byte-stable gate.
- The earlier `gepa_stage_proposer` / `AgentBacked` path has been deleted
  rather than preserved as scaffold (§7).

Outcome: agentic GEPA reflection works for skill-bank artifacts.

### Phase 2 — Receipts on the converged path — deferred to v0.0.2-alpha

The converged proposer records `StageAttemptReceipt`s. Decide whether GEPA's
`GepaCheckpointState` already covers reflection-stage resume or whether the
receipt is independently load-bearing (§8). Until decided, the missing receipt
is an explicit TODO, not a silent gap.

### Phase 3 — `leaven` CLI (priority P2) — deferred to v0.0.2-alpha

A `leaven` CLI (or in-workspace query server) exposes `leaven_query` to a
running agent. Only with this transport does `AgentBacked`'s interactive-context
machinery become real and earn its complexity. Until then, interactive queries
stay out of the reflection critical path.

### Phase 4 — Git and Jujutsu artifact crates — deferred to v0.0.2-alpha

Build the remaining repo artifact behavior beyond the current vocabulary.
`leaven-artifact-git` needs a materialization/readback owner outside the cold
artifact vocabulary, and `leaven-artifact-jj` needs real JJ operation-log and
diff readback semantics beyond its file-snapshot materialization. Each repo
path still needs an `EditSurface` so GEPA can select a part. This phase unblocks
repo integration testing — the headline use case — and nothing before it does.

### Phase 5 — Full convergence — deferred to v0.0.2-alpha

Collapse `AgentBacked` and `AgenticProposer` into the single stage proposer of
§3.1 and remove the scaffolding left from Phase 1. Pick the single
materialization seam (§3.3). This is the hard cutover the interim scaffolding
defers.

### Orthogonal — Multi-part reflection

`leaven-338` (multi-part / multi-component reflection) is independent of this
spec but must target the converged proposer of §3.1, not `AgentBacked`. A repo
artifact reflected on by an agent is exactly where single-part selection stops
being meaningful.

## 5. Definition of done

- A GEPA run can use an agent as its reflector, and the agent receives the
  current artifact materialized into its workspace.
- Skill-bank agentic reflection has an end-to-end test that fails if the
  artifact is not delivered.
- Git/jj repositories can be materialized, edited by an agent, and read back as
  typed changes, with an integration test.
- A single agentic stage proposer; no parallel half-paths in the product
  surface.

## 6. Reconciliation with `agentic_stage_materialization.md`

`agentic_stage_materialization.md` v0.4 defines three layers and places the
optimizer agentic stage (layer B) on `AgentBacked` / `leaven_query` /
`StageAttemptReceipt`. That layering intent stands. This spec corrects one
thing: layer B as implemented delivers graph metadata, not artifacts, and its
interactive-query premise has no transport. Layer B must gain real
materialization, and the `AgenticProposer` (currently adjacent to layer A)
folds into it rather than persisting as a parallel proposer.

Layer A (candidate-evaluation workload: `AgentCase` / `AgentCaseEvaluator`) and
layer C (workspace substrate) are unchanged. When Phase 5 lands, v0.4's layer B
section must be updated to the converged proposer or this spec promoted to
govern layer B outright.

## 7. Scaffolding policy

Repo policy is hard cutover with no parallel old/new paths. The old
GEPA-specific `gepa_stage_proposer` scaffold is not retained. Generic
`AgentBacked`, `StageReadAuthority`, and `StageQuery` machinery remain in
`leaven-stage` as stage substrate, but they are not GEPA product reflection
proof until a materializing route exists. Every retained scaffold surface must:

- carry a docstring stating it is pre-convergence scaffolding and pointing at
  this spec;
- carry a `TODO` naming the phase that removes or absorbs it;
- be classified as explicit scaffold, not ordinary public contract, wherever it
  is exported.

Phase 5 is the hard cutover that ends the exception.

## 8. Open questions

- **Materialization seam.** `leaven_engine::Materializer<P, T>` versus
  `leaven_stage::MaterializableArtifact` — convergence must pick one. Undecided.
- **Receipts versus checkpoint.** Whether reflection-stage `StageAttemptReceipt`
  is load-bearing for resume given `GepaCheckpointState`. Decided in Phase 2.
- **This spec's status.** Written as a companion evolving layer B. May be
  promoted to formally supersede `agentic_stage_materialization.md` layer B
  after Phase 5.
- **Composite agent kits.** First slice is a repo-backed `AgentKit` view over
  `GitProgramArtifact`, with Codex as the first materialization profile. The
  slice includes `manifest.toml`, `system_prompt.md`, `AGENTS.md`, and
  `skills/`. This proves deterministic Codex workspace projection, not live
  Codex consumption; promotion to an ordinary default route requires the opt-in
  live Codex conformance gate for the target surface to prove consumption of
  the projected system prompt, `AGENTS.md`, and skills. `hooks/` is scaffold
  only and is not materialized by the default Codex profile; `harness/` is
  optional and not required for simple Codex kits.
