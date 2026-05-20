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
Parsers translate workspace/session results into proposals or assessments.
Optimizers decide rhythm.
Populations decide what survives.
Evidence preserves what happened.
```

The skill substrate must not assume GEPA, EvoSkill, Claude Code, Codex, git,
or a single skill format beyond the Agent Skills folder contract.

### 1.1 Relationship to the agentic task substrate

The end-user-facing agent workload contract lives in
`docs/specs/agentic_task_execution_substrate.md`.

This document specializes that general substrate for skills:

```text
AgentWorkload / AgentCaseEvaluator
  are generic agent execution/evaluation infrastructure.

SkillBank / SkillBankSurface / SkillUseEvent
  are skill-specific artifact, surface, and evidence vocabulary.

GEPA or another Optimizer
  consumes both through ordinary proposer/evaluator stages.
```

Skill optimization should not introduce a separate `SkillOptimizer` concept
that competes with `Gepa` or `Optimizer<P>`. For GEPA-shaped skill learning,
the composition is:

```text
P::Artifact = SkillBank or AgentKit
S = SkillBankSurface or AgentKitSurface
Evaluator = AgentCaseEvaluator<P> with a skill-aware presenter/scorer
Proposer = AgentAuthoredProposer<P> with a SkillBank proposal parser
PartSelector = skill/file selector
Population = reusable frontier/population policy
Runtime = stage dependency
```

The skill-specific product crate, if any, should provide convenience
constructors for these pieces. It must not own the optimizer rhythm, provider
runtime semantics, graph admission, or generic case execution substrate.

### 1.2 Skill-specific responsibilities

Leaven's skill-specific layer owns:

- `SkillBank` and `SkillFolder` artifacts
- Agent Skills validation
- skill edit surfaces
- skill materializers/presenters over the general `AgentCasePresenter` seam
- skill proposal parsers for agent-authored changes
- skill-use event parsing overlays for provider dialects
- `AgentSkillEvidence` helpers and attribution impls
- skill-specific tests over the general agent workload contract

It does not own:

- case suites
- generic agent workload execution
- generic run policy, retry, approval, recovery, or scoring contracts
- provider runtime traits
- GEPA candidate selection or optimizer rhythm
- graph admission or checkpoint/restore

---

## 2. Generic vs Paper-Specific

Generic Leaven/library pieces:

- skill folder and skill bank artifact types
- Agent Skills validation
- skill edit surfaces
- skill materialization layouts for agent runtimes
- provider-neutral agent runtime and agentic stage adapters
- git-backed artifact readback support
- evidence types for traces, failures, skill use, scorer output, and attribution
- population/frontier/select/admit primitives
- deterministic cache identity
- durable checkpoint/restore for long agentic runs

Paper-specific reproduction pieces:

- EvoSkill loop, prompts, OfficeQA/SealQA scorers, train/val/test splitting
- Trace2Skill analyst prompts, consolidation, merge hierarchy
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
a `SKILL.md` write. This keeps "what changed identity?" separate from "what
changed routing semantics?" while still allowing one proposal to do both.

---

## 4. Skill Bank Changes

Skill mutations should be filesystem-native. Total rewrites must be ordinary
changes.

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
proposal-stage control flow above it.

Initial rule:

```text
Artifact validation is mandatory at graph insertion.
Proposer repair is stage policy, not engine policy.
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
population, while preserving the failed graph-apply attempt as run history. It
does not repair anything by itself.

The engine should not secretly call an agent again because a proposal failed
validation. The standard repair primitive lives inside the proposer stage: the
same proposer may iterate until it can return a proposal batch that is locally
valid against the graph snapshot it was given, or until it exhausts attempts.
Repair is not a fallback to some global "fixer" unless the optimizer explicitly
chose such a proposer as the original stage.

```rust
pub struct ProposalRepairPolicy {
    pub max_attempts: NonZeroUsize,
}
```

When enabled, the same proposer receives the typed validation error, validator
output, parser error, or local apply/validate error plus the prior attempt
context and authors a revised proposal. "Same" means same proposer identity,
same configured backend/runtime, same parser, and same optimization
stage. It does not require the provider to resume the same chat/session thread.
Provider adapters may use native continuation when available, but portable
Leaven semantics are explicit prior context plus a bounded next attempt.

The proposer-owned loop has one shape:

```text
request enters proposer
materialize allowed context into one workspace
render initial instructions
for attempt in 1..=max_attempts:
  run authoring agent or deterministic authoring step
  parse output/workspace changes into a ProposalBatch
  locally apply/validate proposed artifacts against the input graph snapshot
  if locally valid: return ProposalBatch
  render repair feedback back to the same proposer attempt loop
return ProposalError::repair_exhausted(...)
```

The graph remains the final admission authority. A proposal that was locally
valid inside the proposer may still fail at `RunContext::apply_batch` because
the graph changed, storage refused a write, or the proposer/parser had a bug.
That failed graph-apply attempt is recorded as `ApplyFailed`; the engine still
does not call the proposer again.

This is the key simplification:

```text
repair before returning ProposalBatch = proposer responsibility
admit returned ProposalBatch to graph = RunContext responsibility
retry after graph admission failure = optimizer-specific, not a standard primitive
```

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

The likely generic stage-local feedback shape is:

```rust
pub struct ProposalRepairFeedback<'a> {
    pub failed_attempt: NonZeroUsize,
    pub max_attempts: NonZeroUsize,
    pub parse_error: &'a AgenticParseError,
    pub previous_session: &'a AgentSession,
}

pub trait ProposalRepairPromptBuilder<I>: Send + Sync {
    fn build_repair(
        &self,
        original_input: &I,
        feedback: ProposalRepairFeedback<'_>,
    ) -> Result<AgentInstructions, AgenticRepairError>;
}
```

`leaven-agentic` provides `RepairingAgenticProposer` for this standard path. It
materializes once, keeps the same workspace across attempts, reruns the same
provider-neutral runtime with repair instructions, and only returns a
`ProposalBatch` after the parser/local validity checks succeed. It should not
require optimizers to own the repair loop unless an optimizer wants a
non-standard policy.

### 4.3 Validator reports

Validator reports should be intentionally small. The report exists for two
callers:

```text
the same proposer, which needs enough detail to repair
the run recorder, which needs enough detail to audit what happened
```

Minimum shape:

```rust
pub struct ValidatorReport {
    pub checks: Vec<ValidatorCheckReport>,
}

pub struct ValidatorCheckReport {
    pub name: String,
    pub cwd: WorkspacePath,
    pub argv: Vec<String>,
    pub exit_code: Option<i32>,
    pub stdout: DiagnosticText,
    pub stderr: DiagnosticText,
}

pub struct DiagnosticText {
    pub text: String,
    pub truncated: bool,
}
```

Laws:

- validator commands run inside the stage workspace through `WorkspaceView`, not
  through host paths
- a failed validator check is repair feedback, not evaluator evidence and not
  candidate quality evidence
- truncation must be explicit when output is shortened
- validators must not read hidden partitions unless the proposer has read scope
  for them
- validator success means only "the configured checks passed"; graph admission
  and artifact validation still remain authoritative

Permission portability is not a `SkillBank` validation error. A `SkillFile`
can represent `executable: true`; if a specific workspace backend cannot
materialize or preserve that bit, the failure belongs to materialization or
workspace readback, with the backend and path preserved in that error.

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

### 5.4 Retrieval/index surface

`SkillCard` starts as a manifest-derived catalog entry:

```rust
pub struct SkillCard {
    name: SkillName,
    description: SkillDescription,
    metadata: SkillMetadata,
}
```

The base card is recomputable from `SkillBank` alone and is useful for router
catalogs that should see names, descriptions, and generic frontmatter without
reading full skill bodies or files. Utility scores, trigger counts, learned
retrieval keys, and use stats are not part of the standard `SkillBank` card;
they require run/evidence state or a deliberate registry artifact overlay.

Open question: whether Leaven should ship a separate `SkillRegistryArtifact`
for systems like D2Skill that maintain utility, retrieval keys, and eviction
state as optimized memory rather than derived bookkeeping.

---

## 6. Materialization and Progressive Disclosure

Skill folders materialize differently for different agent runtimes. This spec
only defines the generic layout choice; provider-specific paths belong in
provider adapter specs.

Common generic layouts:

```text
Provider-neutral workspace:
  skills/<skill-name>/SKILL.md
  agent.toml maps provider layout to skill root

Provider-native workspace:
  provider adapter chooses the runtime-specific skill root
```

The materializer owns layout. `SkillBank` does not.

```rust
pub struct SkillBankMaterializer {
    pub layout: SkillLayout,
    pub selection: SkillSelectionPolicy,
}

pub enum SkillLayout {
    ProviderNeutral,
    ProviderNative { name: String },
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

### 7.3 Checkout and readback

Checkout strategy is operational, not semantic:

```rust
pub enum GitCheckoutStrategy {
    TempClone,
    Worktree,
    BareCheckout,
    Bundle,
    SnapshotReadback,
}

pub enum GitReadbackPolicy {
    Commit,
    DiffOnly,
    Snapshot,
}
```

`SnapshotReadback` is important for containers: materialize files into a sandbox,
let the agent mutate them, read changed files back, and create the git commit
outside the sandbox.

Agentic stage flow:

```text
parent candidate = GitArtifact { revision: abc }
materialize checkout/snapshot from abc
agent edits workspace
parser reads workspace result and creates GitChange::AdvanceTo { expected_parent: abc, child: def }
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
Parser turns files/transcript/workspace state into typed result.
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
  owns skill-layout materializers, skill workspace proposal parsers,
  provider-specific skill layout helpers, and common skill parsers
  depends on leaven-agentic, leaven-artifact-skill, leaven-workspace
```

This crate is optional. Do not add it until the helpers are large enough to
hide a real decision.

### 8.1 Workspace proposal parsing

Agentic proposers often do not produce `output/proposals.json`. The natural
shape for code and skill agents is:

```text
materialize parent artifact into a workspace
agent edits files
stage parses the edited workspace back into typed Leaven proposals
```

That readback step is proposal parsing. It is stage-owned parsing logic, not
runtime logic and not graph mutation. Do not add a second generic workspace
readback trait for this unless a later implementation exposes real polymorphism
and independent laws that `ProposalParser` cannot express.

```rust
pub trait ProposalParser<P, I>: Send + Sync
where
    P: OptimizationProblem,
{
    async fn parse_proposals(
        &self,
        workspace: &mut WorkspaceView<'_>,
        session: &AgentSession,
        input: &I,
        graph: RunGraphView<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, AgenticParseError>;
}
```

For `SkillBank`, a concrete `SkillBankWorkspaceProposalParser` reads the
materialized skill directory, parses and validates every `SKILL.md`, compares
the resulting `SkillBank` to the parent artifact, and emits one of the standard
`SkillBankChange` proposal forms. For git-backed artifacts, a concrete
`GitSnapshotProposalParser` reads a stable revision, snapshot, or diff summary
and emits the corresponding git artifact change. In both cases, the parser
converts workspace side effects into immutable proposal data.

The standard `AgenticProposer` has one proposal parsing seam with multiple
concrete parser implementations:

```text
structured output:
  runtime satisfies OutputContract::JsonFile or OutputContract::Files
  JsonProposalFileParser reads those files into ProposalBatch

workspace readback:
  runtime satisfies OutputContract::WorkspaceDiff or stage-owned equivalent
  SkillBankWorkspaceProposalParser / GitSnapshotProposalParser reads changed
  workspace state into ProposalBatch
```

Both modes feed the same proposer-owned validity loop in §4. The proposer should
not return a batch until the parsed proposal has passed local validation
against the graph snapshot visible to that proposer.

For agentic GEPA reflection, the SkillBank consumer is a
`SkillBankReflector: ArtifactReflector` (see
`docs/specs/typed_signature_adapter_contract.md` §4). Its `project`
materializes the SkillBank into `target/current/<skill-name>/<path>`; its
`read_back` diffs the tree, validates the resulting bank, and returns a typed
`SkillBankChange`, or `ReadbackResult::Invalid` with diagnostics when the agent
broke the contract. The thin `GepaSkillBankAgenticReflector`, which implements
`GepaReflector`, wraps the `ReflectionWorkspace::run` call; the bespoke
`renderer.rs` / `materializer.rs` / `parser.rs` triplet in
`leaven-gepa-agentic-skill` is superseded.

Parser laws for workspace readback:

- **No graph mutation.** A parser returns `ProposalBatch`; only `RunContext`
  records and applies it.
- **Typed proposal data.** A parser must return artifact-native changes or fresh
  artifacts, not opaque workspace paths.
- **Workspace paths only.** A parser uses `WorkspacePath` and `WorkspaceView`;
  host `PathBuf` access is backend-specific and belongs behind a workspace
  backend or adapter.
- **Local validity check.** A parser or wrapping proposer validates the
  resulting proposal against the relevant parent artifact before returning it.
- **Graph admission remains authoritative.** Local validity is a preflight, not
  a substitute for `RunContext::apply_batch`.
- **Identity discipline.** Workspace paths, branch names, and mutable refs are
  not artifact identity or cache identity.
- **Read-scope discipline.** Parsers may read only data the proposer was
  allowed to materialize or observe.
- **Deterministic parsing.** Given the same parent artifact, parser config,
  session output, and workspace bytes, parsing should produce the same
  proposal batch or the same error.

Minimum error shape:

```rust
pub enum AgenticParseError {
    MissingExpectedPath { path: WorkspacePath },
    ReadFailed { path: WorkspacePath, source: WorkspaceError },
    ParseFailed { path: WorkspacePath, source: DurableErrorRecord },
    InvalidArtifact { source: DurableErrorRecord },
    NoChanges,
    UnsupportedWorkspace { reason: String },
}
```

`NoChanges` is not automatically an error in every optimizer. A proposer that is
allowed to return "no proposal" may map it to an empty batch; a proposer that
was explicitly asked to author a mutation should surface it as repair feedback
or `ProposalError`.

---

## 9. Evidence for Skill Optimization

Skill papers need more than scalar scores, but Leaven should not make
"trajectory" a mandatory cold-core concept. The base rule is:

```text
runtime/session crates preserve raw transcripts and provider events
evaluator/parser code derives typed evidence when it can
skill-use telemetry is an optional evidence capability
absence of telemetry means unknown, not false
```

Standard evidence should be able to preserve:

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

Proposed standard shape for agent-skill evaluations:

```rust
pub struct AgentSkillEvidence {
    pub outcome: CaseOutcome,
    pub scores: ScoreVector,
    pub transcript: TraceRef,
    pub scorer_output: ScorerOutput,
    pub skill_events: Vec<SkillUseEvent>,
    pub failures: Vec<FailureRecord>,
    pub attributions: AttributionSet,
}

pub struct SkillUseEvent {
    pub skill: SkillName,
    pub kind: SkillUseKind,
    pub source: SkillUseSource,
    pub confidence: SkillUseConfidence,
    pub evidence: Option<TraceRef>,
}

pub enum SkillUseKind {
    Available,
    Retrieved,
    Loaded,
    ReferencedFile { path: SkillPath },
    RanScript { path: SkillPath },
    CitedOrParaphrased,
}

pub enum SkillUseSource {
    RuntimeTelemetry,
    TranscriptParser,
    EvaluatorInstrumentation,
    PaperSpecificInference,
}

pub enum SkillUseConfidence {
    Observed,
    Inferred,
}
```

Expected attribution impls:

```rust
impl AttributableEvidence<CaseId> for AgentSkillEvidence
impl AttributableEvidence<SkillName> for AgentSkillEvidence
impl AttributableEvidence<SkillFilePart> for AgentSkillEvidence
impl AttributableEvidence<FailureClusterId> for AgentSkillEvidence
```

Optional capability trait:

```rust
pub trait SkillUseEvidence {
    fn skill_events(&self) -> &[SkillUseEvent];
}
```

Why this matters:

- EvoSkill needs failures below threshold and active-skill context.
- Trace2Skill needs success/failure memories and consolidation support counts.
- Memento-Skills needs skill-level failure attribution.
- D2Skill needs utility updates based on retrieved skills.
- SkillReducer needs route-trigger evidence and task-retention evidence.

Telemetry laws:

- `Observed` means the runtime, instrumentation, or evaluator directly observed
  the event.
- `Inferred` means a parser or paper-specific analysis inferred the event from a
  transcript, final answer, filesystem output, or scorer result.
- an implementation must not convert "not observed" into "did not happen"
- evidence should keep a `TraceRef` or other durable pointer when the event is
  derived from a larger transcript
- generic retrieval and utility policies may consume `SkillUseEvidence`, but
  paper-specific inference stays in the evaluator or reproduction crate

For agentic reflection, `AgentSkillEvidence.transcript: TraceRef` flows into a
typed `Attachment::Transcript(TraceRef)` on the corresponding `ReflectiveRun`
(see `docs/specs/typed_signature_adapter_contract.md` §3 and
`gepa_reflection_evidence_visibility.md` §3). `skill_events:
Vec<SkillUseEvent>` flows into `Attachment::Json` named `skill_events`; there
is no dedicated `ToolCalls` variant. Artifact-specific evidence stays out of
the generic `ReflectiveRun` shape; it arrives as an attachment contributed by
the SkillBank evidence projection.

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
RoundRobinCandidate
BestCandidate
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
  candidate selectors that depend only on population evidence

leaven-gepa
  GEPA step rhythm
  GEPA-specific part selectors
  GEPA-specific batch/gate/validation wiring

leaven-agentic-skill or paper crates
  skill-utility selectors that depend on skill telemetry or registry state
```

This split keeps `ParetoFrequencyWeighted`, `BestCandidate`, `RoundRobinCandidate`,
and `TopKByPreference` reusable for EvoSkill, GEPA, beam baselines, and
non-GEPA skill optimizers.

### 10.1 Utility state ownership

Skill utility is learned run state by default. It should not be baked into
`SkillBank` just because several papers maintain utility tables.

Ownership rule:

```text
skill content and routing description = Artifact
frontier/admission/retrieval utility = Population or optimizer private state
deployed registry visible to the candidate agent = Artifact
fixed evaluator-side retrieval model = Evaluator config/fingerprint
```

Start with population-owned utility state:

```rust
pub struct SkillUtilityState {
    pub utilities: BTreeMap<SkillName, SkillUtility>,
    pub observations: BTreeMap<SkillName, SkillUtilityStats>,
}
```

Promote utility state into an artifact only when changing that utility state
changes the candidate being evaluated. Examples:

- the agent receives a materialized `skill_registry.json` containing utility
  scores and uses it at task time
- the optimizer mutates a router policy as part of the candidate
- the paper explicitly treats the learned registry as the deployed artifact

Keep utility outside the artifact when it is only optimizer bookkeeping:

- parent selection weights
- admission/pruning thresholds
- retrospective analysis of which skills helped
- evaluator-side routing held fixed across candidates

Laws:

- if utility changes candidate behavior, it must participate in artifact and
  cache identity
- if utility is population/private state, checkpoint/restore must preserve it
- if utility belongs to evaluator config, evaluator fingerprint/cache key must
  include it
- `RenameSkill` must either transfer utility state or record an explicit
  discontinuity; remove/create must not silently preserve utility

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

Agentic skill runs are long and expensive. Checkpointing is not just graph
serialization: the graph records public optimization facts, while optimizers,
populations, selectors, repair loops, and caches may hold private state needed
to continue without changing behavior.

Minimum envelope:

```rust
pub struct RunCheckpoint {
    pub format_version: u32,
    pub run_id: RunId,
    pub created_at: Timestamp,
    pub graph_snapshot: GraphSnapshotRef,
    pub optimizer_state: Option<OptimizerStateSnapshot>,
    pub population_states: BTreeMap<PopulationId, PopulationStateSnapshot>,
    pub selector_states: BTreeMap<StageId, StageStateSnapshot>,
    pub admission_states: BTreeMap<StageId, StageStateSnapshot>,
    pub budget_ledger: BudgetLedgerSnapshot,
    pub cache_index: Option<CacheIndexSnapshot>,
    pub artifact_refs: Vec<ArtifactRef>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub stage_journal: StageJournalSnapshot,
    pub workspace_journal: WorkspaceJournalSnapshot,
}
```

Checkpoint boundaries:

```text
after seed insertion
after proposal batch recording
after proposal application
after assessment recording
after population update
after cache write
after repair attempt completes
```

Do not require mid-session checkpointing inside an agent turn. If a process
crashes mid-agent-run, restore may mark that workspace abandoned and rerun the
stage from the last clean boundary.

Required restore behavior:

- resume frontier/population membership
- avoid re-running cached completed evaluations
- preserve failed proposals and discarded candidates
- preserve feedback history used by agentic proposers
- preserve artifact revisions or content blobs
- mark abandoned workspaces and run janitors where possible
- preserve private counters, RNG state, round-robin cursors, utility tables, and
  repair-attempt state when those are not graph-derived
- never replay committed graph mutations
- never charge budget twice for restored completed stage outputs
- detect schema/fingerprint mismatches before continuing

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

The same pattern applies to populations, selectors, admission policies, and
long-lived repair state when they are not reconstructible from graph events. An
optimizer may choose "derived from graph only," but that should be an explicit
state schema, not an accident.

```rust
pub enum PrivateStatePolicy {
    DerivedFromGraph,
    ExplicitSnapshot {
        schema: Fingerprint,
        format: StateFormat,
    },
}
```

Restore laws:

- **Graph truth first.** The restored graph is the source of public candidate,
  proposal, assessment, event, and cost truth.
- **Private state must line up.** Restored optimizer/population/private state
  must reference candidates and assessments that exist in the restored graph.
- **No hidden replay.** Restore must not replay proposal application,
  assessment recording, cache writes, or population events that were already
  committed before the checkpoint.
- **No silent state loss.** If an optimizer declares explicit private state and
  it is missing or schema-incompatible, restore fails.
- **Derived means checked.** If state is declared `DerivedFromGraph`, restore
  recomputes it deterministically from graph events and should test that this
  matches the live state in checkpoint round-trip tests.
- **In-flight sessions are abandoned.** Workspaces or agent sessions that did
  not reach a clean checkpoint boundary are not resumed as if completed; they
  are abandoned, cleaned up when possible, and may be rerun by the owning stage.

Resume must be able to answer:

```text
which candidate/frontier was active?
which private counters/RNG seeds/router weights/utility tables existed?
which validation/reproposal attempts already happened?
which cached evaluations are safe to reuse?
which agent workspaces were abandoned or consumed by a parser?
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
SkillEventDialect    = optional provider dialect overlay for skill telemetry
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
- `SkillCard` is recomputable from `SkillBank`; utility/retrieval overlays are
  recomputable from cards plus evidence or registry stats.
- `CacheIdentity` is derived from normalized content, never from `CandidateId`.
- skill-use telemetry is optional and provider-derived; absence of telemetry is
  unknown, not false.

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
- `leaven-agentic`: bounded proposer-owned repair adapter over existing
  proposer/runtime/parser seams.
- `leaven-agentic-skill`: skill-specific presenters, proposal parsers,
  skill-event dialect overlays, and convenience constructors over
  `leaven-agentic`; no private fake graph, fake case model, or fake skill model.
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

SkillWorkspaceParseError
  tells stage callers whether workspace changes could not be parsed back into
  immutable artifact state.

ReproposalError
  tells proposer/stage callers whether repair exhausted attempts, validation
  kept failing, runtime failed, or parsing failed.

SkillDialectError
  tells dialect callers whether skill-use telemetry could not be parsed or a
  valid SkillBank could not be projected into a provider-specific skill layout.
```

Important split:

- missing `SKILL.md`, invalid frontmatter, empty body, name mismatch, duplicate
  names, and escaping paths are artifact validation errors
- backend inability to preserve executable bits is a materialization or
  workspace parse error
- script execution failures are evaluator evidence or runtime/session errors,
  not skill artifact errors
- low score, bad behavior, or irrelevant routing is evidence/preference, not
  validation failure
- inability to parse skill-use telemetry is dialect evidence quality loss or a
  dialect error, not proof that no skill was used

Every public error should preserve:

- skill name or folder where known
- path where relevant
- backend/dialect id where relevant
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

Skill dialect overlay tests:

- dialect materialization of the same valid bank is deterministic for the same
  backend config and workspace capability set
- a dialect incompatibility does not become `SkillBankError`
- transcript parser preserves raw provider evidence through a trace reference
- missing telemetry produces unknown evidence, not a negative skill-use event

General case, objective, retry, approval, and recovery law tests live in
`docs/specs/agentic_task_execution_substrate.md`; skill-specific tests should
reuse those suites instead of redefining local task/evaluator types.

Scenario tests:

- local materializer writes provider-neutral skill layout
- provider-specific layout adapters map the same artifact into provider-specific
  paths without changing artifact state
- proposer-owned repair loop feeds typed validation/workspace-parse feedback to a
  fake runtime and records failed attempts in stage output or events
- GEPA can run one fake-runtime iteration from `SkillBank` seed through
  proposal, validation, agent-case evaluation, evidence, population update, and
  checkpoint using the general agentic task substrate
- selector/admission primitives can be reused by GEPA and a non-GEPA skill
  optimizer without dependency inversion
- checkpoint/restore resumes frontier membership, selector state, and cached
  evaluations without rerunning completed assessments

Paper reproduction gates:

- EvoSkill fake-runtime smoke proves create/edit/validate/select/admit without
  paid agent calls
- EvoSkill workspace-parse smoke proves materialization/readback without
  provider-specific runtime assumptions
- Trace2Skill second gate pressures many-trace consolidation and file-level
  skill parsing
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
- Git snapshot proposal parsing is not implemented.
- Top-k frontier/candidate selectors are not fully implemented.
- OfficeQA reproduction crate is not implemented.

### 14.2 Trace2Skill

Generic primitives used:

- file-level skill surface
- workspace parsing into valid skill folders
- many-to-one lesson consolidation
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

- Leaven needs the basic skill artifact and workspace parsing primitives
  first.
- Structure-aware document editing is deliberately deferred until the file-level
  artifact path is implemented and pressure-tested.
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
- Real skill activation telemetry is runtime-dependent and remains a risk.

---

## 15. Implementation Gaps That Matter

Detailed enough to implement now:

- `SkillFolder` required format and validation
- `SkillBankError` validation family
- `SkillBankChange` folder/file/permission changes
- first-class `RenameSkill`
- derived `SkillManifest` / `SkillCard`
- skill-specific presenters over the general `AgentCasePresenter` seam
- skill-specific proposal parsers over the general `AgentAuthoredProposer` seam
- skill-event dialect overlays over the general `AgentProviderDialect` seam
- proposer-owned validation/reproposal loop
- `ValidatorReport` minimum shape
- workspace proposal parser contract and laws
- utility state ownership rule
- checkpoint envelope and restore laws
- provider-neutral runtime split
- agentic proposer/evaluator adapter shape
- cache identity law
- git artifact identity law

Needs design tightening before implementation:

- executable permission portability across local, git, and remote sandboxes
- security policy for scripts and provider-specific metadata keys such as
  `allowed-tools`
- exact private-state checkpoint trait placement
- crate placement for reusable selection/admission policies so GEPA does not
  own generic candidate selection

Known implementation blockers for paper reproduction:

- real `leaven-agentic` agent workload substrate from
  `docs/specs/agentic_task_execution_substrate.md`
- real `leaven-artifact-skill`
- real `leaven-agentic-skill` skill-specific adapters over that substrate
- real `leaven-artifact-git`
- workspace proposal parser / snapshot parser
- cache identity code cutover
- top-k frontier and standalone candidate selectors
- skill evidence types
- durable checkpoint/restore with explicit private optimizer/population state
- bounded proposer-owned validation/reproposal loop for agentic proposers
- EvoSkill reproduction crate
- Codex provider adapter implementation from
  `docs/specs/codex_app_server_agent_runtime.md`

---

## 16. Recommended Build Ladder

1. Implement `leaven-artifact-skill` with validation, content identity, and
   law tests.
2. Implement skill folder/file/manifest surfaces.
3. Implement the general agent workload substrate in `leaven-agentic`:
   `CaseSuite`, `AgentCase`, deterministic sampling, `AgentCaseEvaluator`,
   `AgentCasePresenter`, `AgentCaseScorer`, run policy, and case-run records.
4. Implement skill-specific presenters, proposal parsers, and skill-event
   dialect overlays in `leaven-agentic-skill`.
5. Implement a Codex-native skill layout overlay using the Codex runtime and
   the general provider dialect/event model.
6. Implement typed skill validation errors and a bounded reproposal adapter in
   `leaven-agentic`.
7. Implement local skill materializer and workspace proposal-parser smoke tests.
8. Implement fake-runtime agentic proposer/evaluator tests over real
   `SkillBank`.
9. Implement `TopKFrontier`, `KeepIfAboveWeakest`, and candidate selectors outside
   `leaven-gepa`.
10. Implement `AgentSkillEvidence` and attribution traits.
11. Prove GEPA consumes the generic agent evaluator/proposer plus `SkillBank`
    without a skill-specific optimizer facade, using a fake runtime first.
12. Finish cache identity cutover.
13. Add minimal checkpoint/restore for graph + explicit optimizer/population
   state.
14. Implement git `GitArtifact` and `GitChange::AdvanceTo`.
15. Implement snapshot proposal parsing for local workspaces.
16. Reproduce one EvoSkill-shaped iteration with a real `SkillBank`, real
    workspace materialization/readback, stored evidence, checkpoint/resume, and
    a fake runtime.
17. Implement the Codex provider adapter from
    `docs/specs/codex_app_server_agent_runtime.md`.
18. Run the same one-iteration EvoSkill gate through live Codex app-server with
    `gpt-5.4-mini` low. This is the first product proof that the generic Leaven
    substrate can drive a real agentic skill mutation.
19. Scale EvoSkill from the fixture to the paper OfficeQA/SealQA setup after
    the provider adapter and one-iteration gate are proven.
20. Use Trace2Skill as the second reproduction to pressure-test many-trace
    consolidation and file-level skill parsing.

The ladder intentionally starts with artifact law and fake-runtime tests. The
goal is to make the skill substrate correct before paying for real agent runs.

---

## 17. Paper Reproduction Acceptance Contract

The paper reproduction crates are the forcing function for whether Leaven's
primitives are properly shaped. A reproduction is only accepted as a Leaven
pressure test when paper-specific code uses real library primitives for the
generic substrate.

Paper-specific crates may own:

- prompts and reflection templates
- dataset loaders and train/validation/test split definitions
- scorers, judges, environment adapters, and task harnesses
- paper-specific equations, thresholds, ablations, and merge policies
- paper-specific renderers, materializers, and parsers when the
  paper's presentation format is genuinely unique

Paper-specific crates must not define substitutes for these generic primitives:

- fake `SkillRegistry` / fake skill folder types
- fake skill validation
- fake candidate graph or proposal application
- fake workspace proposal parsing
- fake frontier/admission/parent selection when a standard primitive exists
- fake cache identity
- fake checkpoint/restore substrate for long runs
- fake skill-use evidence when the standard optional capability applies

Done-done for EvoSkill means:

```text
EvoSkill reproduction uses real SkillBank.
EvoSkill reproduction creates and edits real skill folders with SKILL.md.
EvoSkill reproduction uses real materializer/workspace-parser paths.
EvoSkill reproduction uses real proposer-owned validation/reproposal.
EvoSkill reproduction uses real selection/admission primitives.
EvoSkill reproduction records real evidence and costs.
EvoSkill reproduction can checkpoint and resume without changing the result.
```

The first accepted EvoSkill gate may use a small fixture, but it must run the
real proposer/build/evaluate/admit loop through Leaven and at least one live
Codex app-server session. That gate proves product wiring. It is not the full
paper reproduction until the paper's task loaders, splits, frontier loop,
feedback history, graders, and ablations are present.

If a paper reproduction needs a new public Leaven trait to express its generic
mechanism, that is a design gap. If it needs only paper-specific prompts,
datasets, scorers, or adapters, that is expected user code.

---

## 18. Non-Goals

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

## 19. Short Form

```text
Skill = valid folder with mandatory SKILL.md.
Valid SKILL.md = name + description + non-empty body.
Extra frontmatter = generic skill metadata, not typed core fields.
SkillBank = collection of valid skill folders.
SkillCard = derived retrieval/index state.
ReplaceSkill = replace the whole folder.
RenameSkill = preserve identity continuity across folder/name changes.
Rewrite SKILL.md = WriteFile at SKILL.md.
Scripts/references/assets/other files are first-class.
Artifact validation is mandatory.
Proposer-owned repair may iterate before returning ProposalBatch.
Git is first-class revision support, not the default artifact model.
Workspaces are temporary execution state.
Workspace proposal parsers turn workspace mutations into immutable artifact changes.
Evidence records scores, failures, trace refs, optional skill use, and attribution.
Populations/frontiers/selectors decide what survives and what gets tried next.
Checkpointing must preserve explicit private optimizer/population state.
Optimizers decide the paper-specific rhythm.
```
