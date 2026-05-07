# Leaven v0.2.4 - Agentic Skill Optimization Primitives

> Status: pre-implementation companion spec.  
> Date: 2026-05-07.  
> Governing spec: `docs/specs/initial_library.md`.  
> Runtime companion: `docs/specs/agentic_stage_runtime.md`.  
> Purpose: specify the generic Leaven primitives needed to reproduce current
> agentic skill-optimization papers without baking any one paper into the
> engine.

This document records the current design for making skill optimization a
first-class Leaven use case. The pressure set is:

- EvoSkill, arXiv:2603.02766
- Memento-Skills, arXiv:2603.18743
- Trace2Skill, arXiv:2603.25158
- D2Skill, arXiv:2603.28716
- SkillReducer, arXiv:2603.29919

The first reproduction target is EvoSkill. The generic substrate must also be
strong enough that Trace2Skill, Memento-Skills, D2Skill, and SkillReducer can
be implemented as paper-specific optimizer/reproduction crates rather than by
adding new engine concepts.

---

## 1. Core Thesis

Leaven should optimize agent skills as real artifact packages:

```text
skill-name/
  SKILL.md
  scripts/
  references/
  assets/
  any-other-files
```

Skills are not prompt snippets. They are folder-shaped artifacts with a
mandatory entrypoint document, optional executable code, optional reference
material, static resources, and arbitrary additional files.

The core Leaven split remains:

```text
Artifacts hold candidate state.
Surfaces expose targetable parts.
Materializers project artifacts into workspaces.
Agent runtimes execute sessions in those workspaces.
Parsers import workspace/session results into proposals or assessments.
Optimizers decide rhythm.
Populations decide what survives.
Evidence preserves what happened.
```

The skill substrate must not assume GEPA, EvoSkill, Claude Code, Codex, git,
or a single skill format beyond the Agent Skills folder contract.

---

## 2. Generic vs Paper-Specific

Generic Leaven/library pieces:

- skill folder and skill bank artifact types
- Agent Skills validation
- skill edit surfaces
- skill materialization layouts for agent runtimes
- provider-neutral agent runtime and agentic stage adapters
- git-backed artifact/finalization support
- evidence types for traces, failures, skill use, scorer output, and attribution
- population/frontier/select/admit primitives
- deterministic cache identity
- durable checkpoint/restore for long agentic runs

Paper-specific reproduction pieces:

- EvoSkill loop, prompts, OfficeQA/SealQA scorers, train/val/test splitting
- Trace2Skill analyst prompts, patch consolidation, merge hierarchy
- Memento-Skills read-write loop, router training, utility threshold policy
- D2Skill dual-granularity RL coupling, baseline/skill rollout pairing
- SkillReducer delta-debugging oracle, body classifier, faithfulness gates

The generic pieces should be useful without these papers. The paper-specific
crates should feel like ordinary library users.

---

## 3. Skill Folder Artifact

### 3.1 Required format

A valid skill is a directory containing `SKILL.md`.

`SKILL.md` must contain YAML frontmatter followed by Markdown content. The
body is required, not ornamental: if it is empty after frontmatter parsing, the
skill is not valid. A skill with a name and description but no instructions is
not agent-usable and should be rejected before evaluation.

Required frontmatter:

```yaml
name: skill-name
description: What the skill does and when to use it.
```

All other frontmatter is preserved as generic skill metadata. Leaven does not
bake optional Agent Skills fields such as `license`, `compatibility`, or
`allowed-tools` into the core skill type. If those keys exist, they are just
metadata entries:

```yaml
name: pdf-processing
description: Extracts, fills, merges, and validates PDFs. Use for PDF tasks.
license: Apache-2.0
compatibility:
  packages: [python, poppler]
allowed-tools: Bash(pdftotext:*) Read
paper_specific:
  utility_prior: 0.2
```

The metadata bag preserves arbitrary YAML/JSON-shaped values. Paper-specific
optimizers and provider adapters may unpack metadata they understand, but the
generic skill artifact validates only the required fields and structural safety
invariants.

The generic validator enforces only format and safety invariants:

- `SKILL.md` exists.
- `name` exists, is 1-64 chars, uses lowercase letters, digits, and hyphens.
- `name` does not start/end with a hyphen and does not contain `--`.
- `name` matches the parent directory name.
- `description` exists, is non-empty, and is at most 1024 chars.
- Markdown body exists and is non-empty after trimming frontmatter.
- all non-required frontmatter keys parse into `SkillMetadata`.
- paths are relative, normalized, and cannot escape the skill folder.

There are no generic restrictions on skill body content, scripts, references,
assets, or extra directories.

### 3.2 Proposed type shape

This likely deserves a dedicated crate once implemented:

```text
leaven-artifact-skill
  owns Agent Skills parsing, validation, folder artifact types, and surfaces
  depends on leaven-kernel, leaven-core, leaven-surface
  must not depend on leaven-workspace, leaven-engine, or agent runtimes
```

Core types:

```rust
pub struct SkillBank {
    pub skills: BTreeMap<SkillName, SkillFolder>,
}

pub struct SkillFolder {
    pub name: SkillName,
    pub files: FileTree,
}

pub struct FileTree {
    pub entries: BTreeMap<SkillPath, SkillFile>,
}

pub struct SkillFile {
    pub bytes: Bytes,
    pub permissions: FilePermissions,
}

pub struct FilePermissions {
    pub executable: bool,
}

pub struct SkillManifest {
    pub name: SkillName,
    pub description: SkillDescription,
    pub metadata: SkillMetadata,
}

pub struct ParsedSkillMd {
    pub manifest: SkillManifest,
    pub body: SkillBody,
}

pub struct SkillBody {
    pub markdown: String,
}

pub struct SkillMetadata {
    pub fields: BTreeMap<String, SkillMetadataValue>,
}

pub enum SkillMetadataValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<SkillMetadataValue>),
    Object(BTreeMap<String, SkillMetadataValue>),
}
```

`SkillManifest` is parsed from `SKILL.md`. It is not separate source of truth.
`SkillMetadata` is skill-frontmatter metadata, not Leaven's operational
`MetadataBag`; optimizer logic should depend on typed evidence or explicit
registry state, not ad hoc metadata parsing, unless the paper-specific optimizer
has deliberately made those metadata keys part of its artifact contract.

The executable bit is not semantically special. It is one filesystem
permission bit the artifact preserves because scripts are first-class and git
records executable-bit changes.

### 3.3 Skill identity

`SkillName` is the stable in-bank identifier for a valid skill folder.

Changing a skill's `name` is a rename of the skill identity, not just a text
edit. Rename should be first-class because removal plus creation loses exactly
the continuity that matters for skill optimization: retrieval stats, utility
history, failure attribution, and lineage.

`RenameSkill` therefore means:

```text
old folder name changes
SKILL.md frontmatter name changes to match
skill identity changes
continuity is preserved across the rename event
```

If the rename also changes the description, model that as an atomic rename plus
`SKILL.md` write/patch. This keeps "what changed identity?" separate from
"what changed routing semantics?" while still allowing one proposal to do both.

---

## 4. Skill Bank Changes

Skill mutations should be filesystem-native but not patch-only. Total rewrites
must be ordinary changes.

```rust
pub enum SkillBankChange {
    CreateSkill {
        folder: SkillFolder,
    },

    /// Replace the whole skill folder. This is a hard cutover for one skill,
    /// not a rewrite of only SKILL.md.
    ReplaceSkill {
        name: SkillName,
        folder: SkillFolder,
    },

    RemoveSkill {
        name: SkillName,
    },

    /// Rename a skill identity while preserving continuity for lineage,
    /// attribution, retrieval stats, and utility state.
    RenameSkill {
        from: SkillName,
        to: SkillName,
    },

    WriteFile {
        skill: SkillName,
        path: SkillPath,
        file: SkillFile,
    },

    PatchFile {
        skill: SkillName,
        path: SkillPath,
        patch: TextPatch,
    },

    RemoveFile {
        skill: SkillName,
        path: SkillPath,
    },

    MoveFile {
        skill: SkillName,
        from: SkillPath,
        to: SkillPath,
    },

    SetFilePermissions {
        skill: SkillName,
        path: SkillPath,
        permissions: FilePermissions,
    },

    Atomic(Vec<SkillBankChange>),
}
```

Important naming decisions:

- `ReplaceSkill` means replace the entire skill folder.
- `RenameSkill` means identity rename and must update folder name plus
  `SKILL.md` frontmatter `name` together.
- Rewriting `SKILL.md` is `WriteFile { path: "SKILL.md", ... }`.
- Patching `SKILL.md` is `PatchFile { path: "SKILL.md", ... }`.
- Changing `description` normally means editing `SKILL.md` frontmatter.
- `SkillCard` or retrieval index records are derived from the folder, not
  separate canonical artifact truth.

Every `apply_change` validates the resulting `SkillBank`. A change may replace
or delete any file, but the final artifact must still satisfy the required
Agent Skills format for every remaining skill:

```text
directory exists
SKILL.md exists
SKILL.md frontmatter parses
name is valid and matches folder
description is valid
body is non-empty
paths are normalized
file permissions are normalized into Leaven's preserved permission model
```

This is not a mutability constraint. A proposal may totally rewrite a skill
folder, delete scripts, replace references, add generated files, or rewrite the
entire `SKILL.md`. The library only rejects final states that are not valid
skill artifacts.

### 4.1 Validation and reproposal seam

Leaven already has the right low-level hook: `Artifact::validate` and
`Artifact::apply_change` share an error type, and failed apply/validate must not
mutate the source artifact. `SkillBank` should use that hook directly.

For skills, validation errors should be typed enough to feed back to an agentic
proposer:

```rust
pub enum SkillBankError {
    MissingSkillMd { skill: SkillName },
    InvalidSkillMdUtf8 { skill: SkillName, source: Utf8Error },
    InvalidSkillMdFrontmatter { skill: SkillName, source: FrontmatterError },
    MissingName { skill: SkillName },
    InvalidName { skill: SkillName, reason: SkillNameError },
    NameDoesNotMatchFolder {
        folder: SkillName,
        manifest_name: SkillName,
    },
    MissingDescription { skill: SkillName },
    EmptyDescription { skill: SkillName },
    EmptyBody { skill: SkillName },
    DuplicateSkillName { name: SkillName },
    MissingSkill { name: SkillName },
    EscapingPath { path: SkillPath },
}
```

The open library gap is not validation itself. The gap is the standard
reproposal control flow above it.

Initial rule:

```text
Artifact validation is mandatory at graph insertion.
Automatic repair/reproposal is stage policy, not engine policy.
```

Current engine behavior is deliberately minimal:

```text
proposer returns ProposalBatch
optimizer records the batch through RunContext
optimizer applies proposals through RunContext
artifact apply/validate failure records ApplyFailed
no candidate is created for that proposal
the ApplyReport tells the optimizer which proposals failed
the engine does not call the proposer again
```

That behavior is necessary but not sufficient for agentic skill optimization.
It prevents invalid artifacts from entering the candidate graph, cache, or
population, while preserving the failed attempt as run evidence. It does not
repair anything by itself.

The engine should not secretly call an agent again because a proposal failed
validation. A concrete `AgenticProposer`, optimizer, or optimizer helper can
choose a bounded repair loop, but the repair target is the same proposer stage
that authored the invalid proposal. Repair is not a fallback to some global
"fixer" unless the optimizer explicitly chose such a proposer as the original
stage.

```rust
pub struct ReproposalPolicy {
    pub max_attempts: NonZeroUsize,
    pub include_validation_error: bool,
    pub preserve_failed_attempts: bool,
    pub validate_in_workspace: Option<ValidationCommand>,
}

pub struct ValidationCommand {
    pub cwd: WorkspacePath,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub max_attempt_output_bytes: u64,
}
```

When enabled, the same proposer receives the typed validation error plus the
prior candidate/proposal context and authors a revised proposal. "Same" means
same proposer identity, same configured backend/runtime, same parser/finalizer,
and same optimization stage. It does not require the provider to resume the same
chat/session thread. Provider adapters may use native continuation when
available, but portable Leaven semantics are explicit prior context plus a new
bounded attempt.

Failed graph-apply attempts should still be recordable as failed proposal
applications so the run graph preserves what happened. This is core enough to
standardize as proposal-stage orchestration, but it should remain opt-in stage
composition rather than an engine-global retry loop.

There are two useful repair-loop shapes:

```text
Parse/apply validation loop:
  agent proposes a typed change or output file
  parser imports it into ProposalBatch
  optimizer records and applies the batch
  artifact apply/validate fails
  run graph records ApplyFailed
  optimizer calls the same proposer again with the error and prior attempt

Workspace submit validation loop:
  agent edits files in workspace
  proposer stage runs a validator command before parsing/finalizing
  on validator failure, route stdout/stderr back to the same proposer adapter
  on validator success, parser/finalizer imports proposals
```

The second shape covers "validate before submit" workflows: a paper-specific
proposer or generic `AgenticProposerWithRepair` can write a validation report
into the workspace, render it back into the next agent prompt, and let the
agent repair before anything is admitted to the optimizer as a candidate.

Provider-neutral Leaven should not require "same provider thread" semantics for
this loop. `AgentRuntime` currently executes one session in one workspace.
Reproposal can be modeled portably as another session over the same materialized
workspace or an equivalent rematerialized workspace plus explicit context:

```text
previous transcript
previous output/proposal
typed validation error or validator log
repair instructions
```

A provider adapter may use a native thread/session continuation internally if
it has one, but the Leaven contract should not depend on that. The portable
semantic is "bounded repair attempts with explicit prior context," not "resume
the same chat thread."

### 4.2 Proposal-stage scope

The reusable primitive should be proposal-stage scoped. Its job is to keep
invalid authored artifacts out of the graph while giving the same authoring
stage a chance to repair before the optimizer gives up on that proposal.

It should not become a generic "retry anything during running" mechanism.
During evaluation, a candidate agent may fail a task, call the wrong tool, or
produce bad output; that is usually evidence about candidate quality, not a
reason for Leaven to re-enter the proposer. Evaluators may have their own
runtime reliability retries or self-correction loops, but those are evaluator
implementation details and should return evidence/cost, not mutate the
candidate graph.

The likely generic helper shape is:

```rust
pub struct ProposalRepairFeedback<'a, P: OptimizationProblem> {
    pub attempt_index: NonZeroUsize,
    pub batch_id: ProposalBatchId,
    pub outcomes: Vec<ApplyOneReport>,
    pub validator_report: Option<ValidatorReport>,
    pub graph: RunGraphView<'a, P>,
}

pub trait ReproposalPlanner<P, Prop>
where
    P: OptimizationProblem,
    Prop: Proposer<P>,
{
    fn next_request(
        &mut self,
        original: &Prop::Request,
        feedback: &ProposalRepairFeedback<'_, P>,
    ) -> Option<Prop::Request>;
}
```

An optimizer can then use a standard loop:

```text
request = original request
for attempt in 1..=max_attempts:
  batch = ctx.propose(&same_proposer, request).await?
  apply = ctx.apply_batch(batch.batch_id)?
  if any acceptable candidate exists: return candidates
  request = planner.next_request(original, apply feedback)?
return record/expose exhausted repair
```

This helper is general enough for skill folders, code-editing agents, harness
generation, config synthesis, and any artifact whose proposal can fail local
structural validation. It is intentionally not a replacement for GEPA's
selection/gate/validation policy; it is the "make the proposed artifact
well-formed enough to enter the graph" loop.

Permission portability is not a `SkillBank` validation error. A `SkillFile`
can represent `executable: true`; if a specific workspace backend cannot
materialize or preserve that bit, the failure belongs to materialization or
workspace finalization, with the backend and path preserved in that error.

---

## 5. Skill Surfaces

Surfaces are lenses over `SkillBank`; they are not intrinsic to the artifact.

The standard surfaces should include at least:

### 5.1 Folder surface

One part per skill:

```rust
pub enum SkillFolderPart {
    Skill(SkillName),
}
```

Use for EvoSkill and Memento-style create/edit/restructure loops where the
optimizer decides at skill granularity.

### 5.2 File surface

One part per file:

```rust
pub enum SkillFilePart {
    File {
        skill: SkillName,
        path: SkillPath,
    },
}
```

Use for Trace2Skill, Memento file-level rewriting, scripts, references, and
git-diff-shaped proposals.

### 5.3 Manifest surface

One part per important frontmatter field:

```rust
pub enum SkillManifestPart {
    Name(SkillName),
    Description(SkillName),
    Metadata {
        skill: SkillName,
        path: Vec<String>,
    },
}
```

Use for SkillReducer and routing optimization. `description` is load-bearing:
it is the startup retrieval signal for common agent runtimes. Optional
frontmatter fields are not individual generic parts; they are metadata paths.
Paper-specific crates can define typed views over their own metadata keys when
those keys become semantic.

### 5.4 Section surface

Potential later surface over Markdown headings inside `SKILL.md`:

```rust
pub enum SkillDocPart {
    Section {
        skill: SkillName,
        heading_path: Vec<String>,
    },
}
```

This is useful for Trace2Skill-style "insert after section" patches and
SkillReducer body restructuring. It is more parser-sensitive and can be
deferred until file-level patches become too blunt.

### 5.5 Retrieval/index surface

`SkillCard` is a derived index:

```rust
pub struct SkillCard {
    pub name: SkillName,
    pub description: SkillDescription,
    pub tags: Vec<String>,
    pub retrieval_keys: Vec<String>,
    pub stats: SkillUseStats,
}
```

The card is not canonical artifact state unless the user explicitly makes a
registry artifact. The standard `SkillBank` derives it from `SKILL.md` plus
run/evidence stats.

Open question: whether Leaven should ship a separate `SkillRegistryArtifact`
for systems like D2Skill that maintain utility, retrieval keys, and eviction
state as optimized memory rather than derived bookkeeping.

---

## 6. Materialization and Progressive Disclosure

Skill folders materialize differently for different agent runtimes.

Common layouts:

```text
Codex:
  .agents/skills/<skill-name>/SKILL.md

Claude Code:
  .claude/skills/<skill-name>/SKILL.md

Provider-neutral workspace:
  skills/<skill-name>/SKILL.md
  agent.toml maps provider layout to skill root
```

The materializer owns layout. `SkillBank` does not.

```rust
pub struct SkillBankMaterializer {
    pub layout: SkillLayout,
    pub selection: SkillSelectionPolicy,
}

pub enum SkillLayout {
    CodexAgents,
    ClaudeCode,
    ProviderNeutral,
    Custom(SkillLayoutSpec),
}
```

Progressive disclosure matters:

```text
startup:
  agent runtime sees name + description for all materialized skills

activation:
  runtime loads SKILL.md body for selected skill

on demand:
  runtime or agent reads scripts/references/assets as needed
```

Leaven should preserve this shape. A renderer may inline selected skill text for
simple LM calls, but agentic stages should prefer materializing real folders.

Trust rule: materializers must respect read scope. Hidden evaluation cases,
oracle answers, test traces, and hidden partitions must not be written into an
agent workspace.

---

## 7. Git Support

Git must be first-class but not default.

Skill folders and skill banks can be pure in-memory/file-tree artifacts. Git is
one durable revision substrate for repo-shaped or codebase-shaped artifacts.

### 7.1 Graph DAG vs Git DAG

Leaven graph:

```text
candidates
proposals
failed attempts
evaluations
evidence
informed_by edges
population/frontier membership
```

Git graph:

```text
commits
trees
parents
merge commits
diffs
```

They should be linked, not collapsed.

```text
Git stores artifact state.
Leaven stores optimization causality.
```

A proposal inspired by another branch but applied to one parent should be a
single-parent git commit and a Leaven proposal with `informed_by`, not a fake
git merge.

### 7.2 Git artifact shape

```rust
pub struct GitArtifact {
    pub repo: RepoRef,
    pub revision: GitRevision,
    pub subpath: Option<RepoPath>,
    pub identity_mode: GitArtifactIdentityMode,
}

pub enum GitArtifactIdentityMode {
    Commit,
    Tree,
}

pub enum GitChange {
    AdvanceTo {
        expected_parent: GitRevision,
        child: GitRevision,
        summary: GitDiffSummary,
    },
    ApplyPatch {
        expected_parent: GitRevision,
        patch: GitPatch,
    },
}
```

Mutable refs are never graph truth. Branch names, worktree paths, and checkout
directories are labels or workspace state, not artifact identity and not cache
identity.

### 7.3 Checkout and finalization

Checkout strategy is operational, not semantic:

```rust
pub enum GitCheckoutStrategy {
    TempClone,
    Worktree,
    BareCheckout,
    Bundle,
    SnapshotImport,
}

pub enum FinalizePolicy {
    Commit,
    DiffOnly,
    Snapshot,
}
```

`SnapshotImport` is important for containers: materialize files into a sandbox,
let the agent mutate them, read changed files back, and create the git commit
outside the sandbox.

Agentic stage flow:

```text
parent candidate = GitArtifact { revision: abc }
materialize checkout/snapshot from abc
agent edits workspace
finalizer imports result and creates commit def
parser returns GitChange::AdvanceTo { expected_parent: abc, child: def }
RunContext applies ProposalEffect::Change
child candidate = GitArtifact { revision: def }
```

---

## 8. Agentic Stage Substrate

`docs/specs/agentic_stage_runtime.md` owns the runtime contract. This skill
spec depends on that split:

```text
Materializer writes the skill world.
Renderer builds prompt/config.
AgentRuntime runs one session.
Parser imports files/transcript into typed result.
```

Generic adapters already belong in `leaven-agentic`:

```text
AgenticProposer
AgenticEvaluator
AgenticRunInput
AgentOutputParser
```

Skill-specific materializer/parser helpers should not live in `leaven-core` or
`leaven-engine`. If they grow beyond small standard helpers, prefer a sibling
adapter crate rather than fattening `leaven-agentic`:

```text
leaven-agentic-skill
  owns skill-layout materializers, skill workspace diff import,
  provider-specific skill layout helpers, and common skill parsers
  depends on leaven-agentic, leaven-artifact-skill, leaven-workspace
```

This crate is optional. Do not add it until the helpers are large enough to
hide a real decision.

---

## 9. Evidence for Skill Optimization

Skill papers need more than scalar scores. Standard evidence should preserve:

- case id / task id
- partition and evaluation purpose
- score vector or preference outcome
- predicted answer / output artifact ref
- scorer output and judge rationale
- execution transcript ref
- tool calls and command records
- active skills
- retrieved skills
- files read from each skill, when observable
- failure records
- attribution records
- cost

Proposed standard shape:

```rust
pub struct AgentSkillEvidence {
    pub outcome: CaseOutcome,
    pub scores: ScoreVector,
    pub transcript: TraceRef,
    pub scorer_output: ScorerOutput,
    pub active_skills: Vec<SkillName>,
    pub retrieved_skills: Vec<SkillName>,
    pub file_reads: Vec<SkillFileRead>,
    pub failures: Vec<FailureRecord>,
    pub attributions: AttributionSet,
}
```

Expected attribution impls:

```rust
impl AttributableEvidence<CaseId> for AgentSkillEvidence
impl AttributableEvidence<SkillName> for AgentSkillEvidence
impl AttributableEvidence<SkillFilePart> for AgentSkillEvidence
impl AttributableEvidence<FailureClusterId> for AgentSkillEvidence
```

Why this matters:

- EvoSkill needs failures below threshold and active-skill context.
- Trace2Skill needs success/failure memories and patch support counts.
- Memento-Skills needs skill-level failure attribution.
- D2Skill needs utility updates based on retrieved skills.
- SkillReducer needs route-trigger evidence and task-retention evidence.

Open question: how much skill-use telemetry can be observed provider-neutrally.
Some runtimes expose skill activation and file reads; others may only expose
transcripts and final files. Evidence must represent both precise telemetry and
best-effort inferred telemetry without lying.

---

## 10. Population, Frontier, and Selection Primitives

The skill papers use several survival/selection patterns:

```text
top-k validation frontier
round-robin parent selection
best parent selection
random parent selection
utility-ranked skill bank
capacity-bounded pruning
patch-set consolidation
train/validation/test partition filtering
```

Generic pieces:

```rust
pub struct TopKFrontier<P> { /* validation score ordered */ }
pub struct BeamPopulation<P> { /* bounded candidate set */ }
pub struct SkillUtilityBank { /* utility/retrieval stats by skill */ }

pub trait CandidateSelector<P> {
    fn select(
        &mut self,
        population: &dyn PopulationView<P>,
        ctx: SelectionContext<'_, P>,
    ) -> Option<CandidateId>;
}

pub trait AdmissionPolicy<P> {
    fn admit(
        &mut self,
        candidate: CandidateId,
        assessments: &[Assessment<P>],
        ctx: AdmissionContext<'_, P>,
    ) -> AdmissionDecision;
}
```

Standard selectors:

```text
RoundRobinParent
BestParent
RandomParent
WeightedParent
ParetoFrequencyWeighted
UtilityWeightedSkill
```

Standard admission/pruning policies:

```text
TopKByPreference
StrictImprovement
KeepIfAboveWeakest
CapacityPruneByUtility
ProtectedWindowPrune
NoRegression
```

Do not bake candidate selection into GEPA. Selection is load-bearing for the
literature and must stay swappable.

This also means Leaven should avoid duplicating "GEPA selectors" and
"skill-library selectors" when the selection logic only depends on population
views, graph views, assessment summaries, or skill utility state. The generic
selector/admission vocabulary should live outside `leaven-gepa`; GEPA should
reuse those pieces and add only GEPA-specific adapters where its request shape
requires a surface part or minibatch context.

Practical placement:

```text
leaven-population
  population/frontier state
  population views
  admission policies
  candidate/parent selectors that depend only on population evidence

leaven-gepa
  GEPA step rhythm
  GEPA-specific part selectors
  GEPA-specific batch/gate/validation wiring

leaven-agentic-skill or paper crates
  skill-utility selectors that depend on skill telemetry or registry state
```

This split keeps `ParetoFrequencyWeighted`, `BestParent`, `RoundRobinParent`,
and `TopKByPreference` reusable for EvoSkill, GEPA, beam baselines, and
non-GEPA skill optimizers.

---

## 11. Cache Identity

The spec already separates graph identity from cache identity. Implementation
must finish that cutover.

```rust
pub trait CacheIdentified: Artifact {
    fn cache_identity(&self) -> Option<CacheIdentity>;
}

pub enum CacheIdentity {
    Content(ContentId),
    ExternalContent(ExternalRef),
    User(Fingerprint),
}
```

Evaluation cache keys should include:

```rust
pub struct EvaluationCacheKey {
    pub evaluator: Fingerprint,
    pub request: EvaluationRequestFingerprint,
    pub candidates: Vec<CacheIdentity>,
    pub case_set: EvaluationSetFingerprint,
    pub runtime: Option<Fingerprint>,
    pub materializer: Option<Fingerprint>,
}
```

For skill artifacts:

- pure `SkillBank` can use `Content(ContentId)` over normalized file tree and
  permissions.
- `GitArtifact` can use `ExternalContent(git commit/tree)` when immutable.
- branch names, workspace paths, and unversioned directories cannot be cache
  identity.

The cache must never key deterministic evaluation on `CandidateId`.

---

## 12. Durable Checkpoint and Restore

Agentic skill runs are long and expensive. Minimum resume support:

```rust
pub struct RunCheckpoint {
    pub run_id: RunId,
    pub graph_snapshot: GraphSnapshotRef,
    pub optimizer_state: OptimizerStateRef,
    pub population_state: PopulationStateRef,
    pub budget_ledger: BudgetLedgerSnapshot,
    pub cache_index: CacheIndexSnapshot,
    pub artifact_refs: Vec<ArtifactRef>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub workspace_journal: WorkspaceJournalRef,
}
```

Required restore behavior:

- resume frontier/population membership
- avoid re-running cached completed evaluations
- preserve failed proposals and discarded candidates
- preserve feedback history used by agentic proposers
- preserve artifact revisions or content blobs
- mark abandoned workspaces and run janitors where possible

Open question: exact serialization boundary for optimizer state. Some
optimizers can derive state from the graph; others need explicit private state.
The trait should make this explicit rather than relying on `serde` magic.

JSON is acceptable as the first durable format, but the boundary must not be
"whatever serde happens to find." The checkpoint contract needs explicit
private-state participation:

```rust
pub struct OptimizerStateSnapshot {
    pub optimizer: Fingerprint,
    pub schema: Fingerprint,
    pub format: StateFormat,
    pub bytes: BlobRef,
}

pub enum StateFormat {
    Json,
    Postcard,
    Custom(String),
}

pub trait CheckpointableOptimizer<P>: Optimizer<P>
where
    P: OptimizationProblem,
{
    type State: Serialize + DeserializeOwned;

    fn checkpoint_state(
        &self,
        ctx: CheckpointContext<'_, P>,
    ) -> Result<Self::State, CheckpointError>;

    fn restore_state(
        &mut self,
        state: Self::State,
        ctx: RestoreContext<'_, P>,
    ) -> Result<(), CheckpointError>;
}
```

The same pattern applies to populations and long-lived selector/admission state
when they are not reconstructible from graph events. An optimizer may choose
"derived from graph only," but that should be an explicit state schema, not an
accident. Resume must be able to answer:

```text
which candidate/frontier was active?
which private counters/RNG seeds/router weights/utility tables existed?
which validation/reproposal attempts already happened?
which cached evaluations are safe to reuse?
which agent workspaces were abandoned or finalized?
```

---

## 13. Design-Tightening Checklist

This section is the sanity check before implementation. The skill substrate is
only net-good for Leaven if it preserves the same type, trait, error, and test
standards as the rest of the library.

### 13.1 Type design

Types should preserve the actual domain distinctions:

```text
SkillBank            = normalized collection of valid skill folders
SkillFolder          = one folder-shaped skill artifact
SkillFile            = bytes + filesystem metadata
ParsedSkillMd        = parsed SKILL.md frontmatter + non-empty body
SkillManifest        = required name/description + generic frontmatter metadata
SkillMetadata        = uninterpreted extra frontmatter tree
SkillBankChange      = filesystem-native artifact mutation
RenameSkill          = identity rename with continuity
SkillCard            = derived retrieval/index view, not source truth
SkillRegistryArtifact = optional future semantic registry state
```

Essential type invariants:

- `SkillName` is validated at construction.
- `SkillDescription` is non-empty and bounded.
- `SkillBody` is non-empty after frontmatter removal.
- `SkillPath` is relative, normalized, and cannot escape the skill root.
- `SkillManifest.name` matches the skill folder name after validation.
- `SkillMetadata` preserves unknown frontmatter without giving it generic
  semantics.
- `SkillFile.permissions.executable` is preserved as artifact state, not as an
  executable-file semantic promise.
- `SkillBank` has no duplicate skill names.
- `SkillCard` is recomputable from `SkillBank` plus evidence/registry stats.
- `CacheIdentity` is derived from normalized content, never from `CandidateId`.

Things to resist:

- typed optional fields for `license`, `compatibility`, `allowed-tools`, or
  other provider-specific metadata in the generic skill artifact
- treating `SKILL.md` text and parsed manifest as two independent sources of
  truth
- representing rename as remove/create when continuity matters
- letting workspace/backend facts leak into cold artifact validity

### 13.2 Trait design

The cold trait surface should stay small:

```text
SkillBank implements Artifact.
Skill folder/file/manifest/section surfaces implement EditSurface.
Materializers live outside the artifact crate.
Agent runtimes know only workspace sessions, not skills.
Reproposal is agentic stage policy, not an engine trait.
Selection/admission traits are reusable population policy, not GEPA internals.
```

Trait laws that must be documented and tested:

- `Artifact::apply_change` is functional: same input state plus same change
  yields the same success or error.
- failed `apply_change` leaves the source artifact unchanged.
- successful `apply_change` returns a valid `SkillBank`.
- `Artifact::validate` and `apply_change` use the same structured error family.
- `EditSurface` part IDs are stable for the same validated artifact state.
- file/folder/manifest surfaces are lenses; they do not own semantic truth.
- materialization is deterministic for the same artifact, layout, task, and
  materializer fingerprint.
- selectors are synchronous, side-effect-light policies over population/graph
  views; remote or LLM work belongs in stages.
- checkpoint-capable optimizers/populations explicitly serialize private state
  or explicitly declare that state is graph-derived.

Likely trait additions or moves:

- `leaven-artifact-skill`: no new public runtime trait; just artifact,
  validation, parser, and surface implementations.
- `leaven-population`: reusable selector/admission traits and implementations
  that do not depend on GEPA.
- `leaven-agentic`: bounded reproposal adapter over existing proposer/runtime
  seams.
- `leaven-store` or `leaven-engine`: checkpoint traits and snapshot references,
  with concrete formats kept behind store/backend boundaries.

### 13.3 Error design

Errors should match caller decisions:

```text
SkillBankError
  tells apply/validate callers whether the artifact is structurally invalid
  and what an agentic proposer should repair.

SkillParseError / FrontmatterError
  tells parser callers which part of SKILL.md failed.

SkillMaterializeError
  tells stage callers whether a valid artifact could not be projected into a
  specific workspace/provider layout.

SkillFinalizeError
  tells stage callers whether workspace changes could not be imported back
  into immutable artifact state.

ReproposalError
  tells optimizer/stage callers whether repair exhausted attempts, validation
  kept failing, runtime failed, or parsing failed.
```

Important split:

- missing `SKILL.md`, invalid frontmatter, empty body, name mismatch, duplicate
  names, and escaping paths are artifact validation errors
- backend inability to preserve executable bits is a materialization or
  finalization error
- script execution failures are evaluator evidence or runtime/session errors,
  not skill artifact errors
- low score, bad behavior, or irrelevant routing is evidence/preference, not
  validation failure

Every public error should preserve:

- skill name or folder where known
- path where relevant
- attempted change when small enough, or blob reference when large
- source parser/backend/runtime error
- retryability or a caller-visible classification where it changes policy

### 13.4 Test design

Implementation should start with contract/law tests before real agent runs.

Artifact law tests:

- valid minimal skill parses and validates
- missing `SKILL.md` fails with typed error
- missing name fails
- invalid name fails
- name/folder mismatch fails
- missing or empty description fails
- empty body fails
- path traversal fails
- arbitrary extra files and directories are accepted
- arbitrary nested metadata is preserved
- executable bit is preserved in content identity
- total folder rewrite is accepted when the final skill is valid
- failed apply leaves source `SkillBank` unchanged
- applying the same valid change twice is deterministic

Surface contract tests:

- folder surface returns one part per skill
- file surface returns one part per file
- manifest surface exposes only name, description, and metadata paths
- part IDs are stable across equivalent content
- surface-lowered edits validate through `SkillBank::apply_change`

Scenario tests:

- local materializer writes provider-neutral skill layout
- Codex/Claude layout adapters map the same artifact into provider-specific
  paths without changing artifact state
- reproposal loop feeds typed validation error to a fake agent and records
  failed attempts
- selector/admission primitives can be reused by GEPA and a non-GEPA skill
  optimizer without dependency inversion
- checkpoint/restore resumes frontier membership, selector state, and cached
  evaluations without rerunning completed assessments

Paper reproduction gates:

- EvoSkill fake-runtime smoke proves create/edit/validate/select/admit without
  paid agent calls
- EvoSkill real-runtime sample proves materialization/runtime/finalization
- Trace2Skill second gate pressures patch consolidation and section surfaces
- Memento/D2Skill gates pressure utility state and retrieval telemetry
- SkillReducer gate pressures description/body split and route-retention
  evidence

---

## 14. Paper Pressure Map

### 14.1 EvoSkill

Generic primitives used:

- `SkillBank` / `SkillFolder`
- create/edit skill proposals
- skill materialization into agent runtime layout
- agentic evaluator and proposer
- git-backed reproducible revisions
- top-k frontier and parent selection
- train/validation/test partitioning
- failure sampling below threshold
- feedback history as proposer input
- cache/checkpoint for expensive runs

Paper-specific pieces:

- EvoSkill proposer prompt
- skill-builder prompt
- OfficeQA/SealQA loaders and scorers
- branch/tag compatibility if reproducing their exact codebase UX
- skill-merge experiment across independent runs

Implementation status:

- Runtime/stage boundary is mostly specced.
- Skill artifact/surface/materializer is not implemented.
- Git finalization is not implemented.
- Top-k frontier/parent selectors are not fully implemented.
- OfficeQA reproduction crate is not implemented.

### 14.2 Trace2Skill

Generic primitives used:

- file-level skill surface
- patch import and application
- many-to-one patch consolidation
- support counts from success/failure memories
- references file creation plus `SKILL.md` link edits
- final skill directory materialization

Paper-specific pieces:

- analyst prompts
- memory item extraction
- merge hierarchy
- conflict/deduplication policy
- SpreadsheetBench/DocVQA task setup

Implementation status:

- Leaven needs `SkillPatchSet` or equivalent helper vocabulary.
- Section-level Markdown surface is probably needed for ergonomic patches.
- Consolidation can be a custom proposer before it becomes standard library.

### 14.3 Memento-Skills

Generic primitives used:

- skill library as writable memory
- router/retrieval over skill descriptions
- skill-level failure attribution
- targeted file-level rewrites
- create-on-miss
- restructure existing skill folder
- unit-test gate and rollback
- utility stats per skill

Paper-specific pieces:

- router training data generation
- behavior-aligned retrieval model
- read-write retry/reproposal loop
- GAIA/HLE setup

Implementation status:

- Leaven has the right artifact/stage split conceptually.
- The router/retrieval model is not a generic Leaven primitive yet.
- Utility state may need a `SkillRegistryArtifact` or population-owned state.

### 14.4 D2Skill

Generic primitives used:

- skill bank entries with retrieval keys
- task-skill and step-skill granularity
- utility updates
- utility-aware retrieval
- capacity-bounded pruning
- protected window for new skills

Paper-specific pieces:

- RL policy training
- paired baseline/skill rollouts
- hindsight utility equations
- ALFWorld/WebShop environments

Implementation status:

- Skill-bank membership and utility state are clear.
- Coupling to RL training is outside the first Leaven reproduction path.
- Need to decide whether dual-granularity skill banks are standard artifact
  shape or paper-specific registry state.

### 14.5 SkillReducer

Generic primitives used:

- manifest/description surface
- body/reference file surface
- route-trigger evidence
- task-based retention evidence
- validation gates
- progressive disclosure materialization

Paper-specific pieces:

- semantic clause segmentation
- ddmin oracle
- adversarial distractor generation
- body taxonomy classifier
- faithfulness verifier
- task generator and feedback loop

Implementation status:

- Description and body mutation should be easy once `SkillBank` exists.
- A section-level Markdown surface would make body restructuring cleaner.
- Real skill activation telemetry is runtime-dependent and remains a risk.

---

## 15. Implementation Gaps That Matter

Detailed enough to implement now:

- `SkillFolder` required format and validation
- `SkillBankChange` folder/file/permission changes
- first-class `RenameSkill`
- derived `SkillManifest` / `SkillCard`
- provider-neutral runtime split
- agentic proposer/evaluator adapter shape
- cache identity law
- git artifact identity law

Needs design tightening before implementation:

- exact `SkillBankError` variants and validation-report shape
- reusable validation/reproposal policy in `leaven-agentic`
- whether utility/retrieval state lives in population state, derived registry,
  or a separate `SkillRegistryArtifact`
- exact `TextPatch` representation
- exact Markdown section parser/surface contract
- provider-neutral skill activation telemetry model
- executable permission portability across local, git, and remote sandboxes
- security policy for scripts and provider-specific metadata keys such as
  `allowed-tools`
- standard patch-consolidation vocabulary for Trace2Skill
- optimizer/population/selector-state checkpoint traits and schema policy
- crate placement for reusable selection/admission policies so GEPA does not
  own generic candidate selection

Known implementation blockers for paper reproduction:

- real `leaven-artifact-skill`
- real `leaven-artifact-git`
- git finalizer / snapshot importer
- cache identity code cutover
- top-k frontier and standalone parent selectors
- skill evidence types
- durable checkpoint/restore with explicit private optimizer/population state
- bounded validation/reproposal loop for agentic proposers
- EvoSkill reproduction crate
- real provider adapter for Codex/Claude/OpenCode

---

## 16. Recommended Build Ladder

1. Implement `leaven-artifact-skill` with validation, content identity, and
   law tests.
2. Implement skill folder/file/manifest surfaces.
3. Implement typed skill validation errors and a bounded reproposal adapter in
   `leaven-agentic`.
4. Implement local skill materializer and fake-runtime parser smoke tests.
5. Implement `TopKFrontier`, `KeepIfAboveWeakest`, and parent selectors outside
   `leaven-gepa`.
6. Implement `AgentSkillEvidence` and attribution traits.
7. Finish cache identity cutover.
8. Add minimal checkpoint/restore for graph + explicit optimizer/population
   state.
9. Implement git `GitArtifact` and `GitChange::AdvanceTo`.
10. Implement snapshot-import finalization for local workspaces.
11. Reproduce EvoSkill OfficeQA sample with fake runtime.
12. Reproduce EvoSkill OfficeQA sample with real Codex runtime.
13. Scale to full OfficeQA split.
14. Use Trace2Skill as the second reproduction to pressure-test patch
    consolidation and section-level surfaces.

The ladder intentionally starts with artifact law and fake-runtime tests. The
goal is to make the skill substrate correct before paying for real agent runs.

---

## 17. Non-Goals

- Do not make git the default artifact model.
- Do not make worktrees the semantic primitive.
- Do not make `SkillCard` separate source of truth for standard skill folders.
- Do not put workspace/materializer code in cold artifact crates.
- Do not make `AgentRuntime` know about skills, candidates, proposals, or
  assessments.
- Do not constrain arbitrary files inside a skill beyond path safety and the
  required `SKILL.md` format.
- Do not bake EvoSkill's loop into the engine.

---

## 18. Short Form

```text
Skill = valid folder with mandatory SKILL.md.
Valid SKILL.md = name + description + non-empty body.
Extra frontmatter = generic skill metadata, not typed core fields.
SkillBank = collection of valid skill folders.
SkillCard = derived retrieval/index state.
ReplaceSkill = replace the whole folder.
RenameSkill = preserve identity continuity across folder/name changes.
Rewrite SKILL.md = WriteFile/PatchFile at SKILL.md.
Scripts/references/assets/other files are first-class.
Artifact validation is mandatory; automatic reproposal is stage policy.
Git is first-class revision support, not the default artifact model.
Workspaces are temporary execution state.
Finalizers import workspace mutations into immutable artifact changes.
Evidence records traces, failures, scores, skill use, and attribution.
Populations/frontiers/selectors decide what survives and what gets tried next.
Checkpointing must preserve explicit private optimizer/population state.
Optimizers decide the paper-specific rhythm.
```
