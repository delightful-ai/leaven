# leaven

*Optimize anything in Rust.*

Project name: **leaven**. Crate: `leaven` (umbrella) + `leaven-core`, `leaven-engine`, `leaven-std`, `leaven-workspace`, `leaven-derive`. Metaphor: the small biological culture you mix into a substrate, walk away from, and come back to find transformed.

> Status: v0.2.7, pre-implementation command-backed runtime bump.  
> Date: 2026-05-07.  
> Supersedes the v0.2.1a spec and folds in the v0.2.1b topology cutover, v0.2.1c surface/evidence/cache cleanup, v0.2.2 renderer/workspace/GEPA selector clarification, v0.2.3 agentic stage runtime contract, v0.2.4 agentic skill optimization primitive spec, v0.2.5 Codex app-server provider adapter spec, v0.2.6 live EvoSkill iteration proof, and v0.2.7 command-backed provider runtime cutover. The architecture is unchanged: cold core stays shape-neutral, surfaces are explicit optimizer/stage choices, and GEPA remains one optimizer.  
> This is still not an API lock — but it is now ready to be coded against.

---

## 0. What Changed in This Pass

This pass keeps the main architectural direction:

> **Engine runs an Optimizer. Optimizer owns algorithm rhythm. RunContext provides shared services. RunGraph records truth. Populations/frontiers maintain live strategy state. Preference relations interpret evidence.**

It tightens several places where the first v0.1 draft was still ambiguous or under-specified:

1. **Cost is now infrastructure, not proposal metadata.**  
   Every stage invocation is metered. Proposal batches, evaluations, renderers, agent runtimes, cache misses, and custom optimizer work can all charge the central `BudgetLedger`.

2. **`ProposalAnnotations` remains typed; `MetadataBag` remains operational.**  
   There is no generic “note” field. Semantic proposal payloads live in `ProposalAnnotations`; debug/operational extras live in `MetadataBag`.

3. **Evaluation distinguishes independent, pairwise, and listwise requests.**  
   Pairwise comparison is one assessment over two candidates. Independent scoring of two candidates is two assessments. The request shape says which one is intended.

4. **Assessment granularity is explicit.**  
   GEPA needs per-case scores to build instance-wise Pareto frontiers. Some evaluators return only aggregate assessments. `AssessmentGranularity` makes this explicit.

5. **`EvaluationSet` and `Niche` are the names.**  
   `Cohort` and `Cell` are removed from the user vocabulary.

6. **The engine has an explicit shape.**  
   The engine owns graph, budget, evaluator registry, cache, callbacks, stoppers, trust policy, and run store. Stage-owned renderers/materializers are the v0.2.2 path. The optimizer owns the algorithm rhythm.

7. **Evaluator registry replaces single evaluator.**  
   Simple users configure one evaluator. Advanced optimizers may call multiple evaluators by ID: task scorer, pairwise judge, human judge, verifier, etc.

8. **Callback event surface is real.**  
   The spec includes a concrete `RunEvent` shape.

9. **Caching policy is explicit.**  
   Engine-owned cache; evaluator-declared cache policy; default no-cache.

10. **Async/dyn policy is explicit.**  
    Optimizers are static-first. Stages intended for registries use object-safe `Dyn*` wrappers returning boxed futures.

11. **Evidence and annotations are run-wide types.**  
    If a run mixes evidence or annotation shapes, the user defines an enum. This is deliberate and Rust-native.

---

## 0.1 What Changed in v0.2

The v0.1 second pass survived the conceptual stress tests. The corrections in this pass are local refinements that emerged when implementations were walked through end-to-end.

12. **`parents` moves from `ProposalBatch` to `Proposal`.**  
    Sibling proposals in a single batch can have different causal parents (cross-branch synthesis case). The batch carries `semantics + metadata`; each proposal carries its own `parents`.

13. **`CausalInputs::None`, `ProposalEffect::Create`, and `Arity::None` are first-class.**  
    Brand-new authored artifacts (Meta-Harness pattern: agent writes a fresh harness from scratch each iteration) have no causal predecessor. Creation is represented by `ProposalEffect::Create`; causal lineage is `CausalInputs::None`; bibliographic influence still flows through `informed_by`.

14. **`Renderer<P, T, Target>` and `Materializer<P, T>` are split trait families.**  
    Value-returning rendering (LM prompt context, JSON blob, debug HTML) and agentic/sandbox workspace population (write files into a workspace for an agent or subprocess to read) have different shapes. Conflating them was awkward. Resolves open question 27.1.

15. **Fitted preference relations live on `Population` impls.**  
    Stateless preferences (cardinal-pareto, scalar, lexicographic, copeland) implement `PreferenceRelation`. Stateful/fitted preferences (Bradley-Terry over accumulated pairwise judgments) are owned by `TournamentPopulation` which fits its model in `observe_assessment`. The `PreferenceRelation` trait stays simple. Resolves open question 27.6.

16. **`ParetoFrontier::partition_filter` is a builder method.**  
    Frontiers can declaratively ignore observations from specific case-set partitions (e.g. only update from `SEARCH`, never from `TEST`). Replaces ad-hoc skip logic in optimizer step bodies.

17. **`informed_by` is a typed graph relation.**  
    Promoted from string-keyed `MetadataBag` access to a first-class graph query. Avoids the python-gepa stringly-typed metadata-parsing failure mode. Stored as a structured `Vec<InfoRef>` in `ProposalProvenance`; exposed via `graph.informed_by(c)` and `graph.informed(c)`.

18. **Merge canonicalization is documented.**  
    `apply(&self, change) -> Self` only sees one artifact, so for `Parents::Pair(a, b)` the change must canonicalize to one parent and embed cross-parent content. Spelled out in §5.5 and §20.

19. **`ContentId` collision-resistance is a hard trait law.**  
    Strengthened from "observational identity" to "MUST be a cryptographic hash of all observationally-relevant state" with a derive macro for safe-by-default impls.

20. **Workspace lifecycle has its own section (§16.6).**  
    `WorkspaceFactory`, `WorkspaceBackend`, and `Workspace` are explicit. Standard backends (Local, E2B, Docker, K8s, Git-worktree) are sketched. Agent runtimes are kept separate from workspaces — they take a workspace and run commands in it.

21. **Implementation plan reorders prototypes 2 and 3.**  
    Pairwise tournament (formerly P3) runs before GEPA parity (formerly P2). Pairwise stresses what is *new* in this design (Pairwise eval requests, fitted preference relations, tournament populations) and is therefore the more informative early test.

22. **Two coding-agent worked examples.**  
    `gskill` and Meta-Harness are spelled out end-to-end in the worked-example section to demonstrate the abstractions on real research workloads. v0.2.1c adds a pairwise tournament example before them.

---

## 0.2 What Changed in v0.2.1

v0.2 retained shapes from v0.1 that became lies once the new capabilities (fresh authored proposals, agentic proposers, typed provenance) were added. v0.2.1 fixes those without changing the architecture.

23. **`Proposal` carries `ProposalEffect`, not a bare `Change`.**  
    `effect: ProposalEffect::{ Create { artifact } | Change { target, change } }`. A brand-new authored artifact is honestly `Create`, not a `Change` with no apply target. Kills the v0.2 awkwardness around Meta-Harness-style fresh authoring.

24. **`ProposalProvenance { causal, informed_by }` is typed.**  
    `informed_by` is no longer "metadata under the hood" — it's a structured field of typed `InfoRef`s (candidates, assessments, proposals, external refs). Graph queries derive from this directly. Removes the python-gepa-stringly-typed failure mode v0.2 was sliding back toward.

25. **`Proposer::Request` is an associated type.**  
    GEPA reflective mutation, merge, Meta-Harness, ComBE, and MIPRO acquisition all need different request shapes. A single universal `ProposalRequest<P>` would collapse to an enum or a metadata bag. Associated type matches the static-first proposer story already chosen in v0.1.

26. **`RunContext::apply_batch` and `apply_proposal`** replace `apply(parents, batch)`.  
    Per-proposal effects subsume the parents argument. Context just routes the proposal through.

27. **`ProposalBatchSemantics::Ordered` is removed.**  
    Multi-batch optimizer rhythm covers ordered-dependency cases. Re-add if a real prototype forces it.

28. **`Materializable` moves out of cold core.**  
    Conflicts with the rendering-separation principle. Now a stdlib convenience trait used by default `Materializer` impls. Custom layouts always go through `Materializer`.

29. **`RendererRegistry` is deferred; stage-owned renderers are the default.**  
    Most stages should hold their renderers/materializers as fields. Add a registry only after the erased value/target/view contract is real.

30. **Historical note: v0.2.1 kept mandatory `content_id`; v0.2.1c supersedes this.**  
    The earlier draft kept cache identity on `Artifact`. v0.2.1c splits graph identity from cache identity because external graph identities can be mutable while some external content references are cache-safe.

31. **`Arity` is a request hint, not a law.**  
    Describes what the optimizer should provide as input when the optimizer drives candidate selection. Proposers may emit fewer or more proposals than `Arity` suggests, and may set causal inputs differently per-proposal.

32. **Constructor sugar for `Proposal`.**  
    `Proposal::mutate(target, change)`, `Proposal::merge(a, b, change)`, `Proposal::create(artifact)` builders cover the common cases in one call. Users rarely construct the full struct directly.

---

## 0.3 What Changed in v0.2.1a

A pre-implementation review flagged real Rust-mechanics issues and residual wording inconsistencies in v0.2.1. Fixed before P0/P1 coding.

33. **`Proposer::Request` is no longer required to be `'static`.**  
    The v0.2.1 spec said `type Request: Send + Sync + 'static`, but the Meta-Harness example wanted `HistoryProposalRequest<'a>` borrowing from the run graph. Resolved: requests should be owned/lightweight (just identify what to do — a `Vec<CandidateId>` plus a `k`, etc.); proposers construct rich views (`HistorySnapshot`) internally from `ctx.graph()`. The bound on `Request` is relaxed, and the worked examples are updated to construct their snapshots inside `propose`.

34. **`<P::Artifact as Artifact>::Change` is the canonical change type.**  
    `P::Change` was used as shorthand in some signatures but `OptimizationProblem` doesn't define a `Change` associated type — the change lives on `Artifact`. Signatures fixed throughout. No new associated type added (would duplicate).

35. **Report types defined explicitly.**  
    `ProposalBatchReport`, `ApplyReport`, `ApplyOneReport`, `EvaluationReport` were referenced but undefined. Now spelled out in §8.4. They return IDs and graph-backed views, not graph-owned values — the graph is the durable truth.

36. **`EvaluationSet` resolution boundary explicit.**  
    `RunContext::evaluate` accepts an `EvaluationRequest` containing an unresolved `EvaluationSet`; the context resolves it and passes a `ResolvedEvaluationRequest` to the evaluator. Cache keys use the resolved set ID + case-set version. The graph records both the original expression and the resolution.

37. **`informed_by` wording cleanup (§10.2).**  
    Stale text saying graph queries are "backed by typed metadata recorded at proposal time" was replaced. They're derived from `ProposalProvenance::informed_by` directly, which is the v0.2.1 win.

38. **`BradleyTerryPreference` renamed to `BradleyTerryFit`.**  
    The stdlib list still listed Bradley-Terry under stateless `PreferenceRelation`s, contradicting §15.1 which placed fitted models on `Population` impls. Fixed: `BradleyTerryFit` is a model object owned by `TournamentPopulation<BradleyTerryFit>`. Stateless graph aggregators (`CopelandPreference`, `BordaPreference`) stay where they were.

39. **`Workspace::cleanup()` is explicit, not Drop-driven.**  
    Async cleanup cannot be reliably awaited in `Drop`. The trait now has an explicit `async fn cleanup(self)`. `Drop` does best-effort local cleanup or marks the workspace abandoned; remote cleanup (E2B sandbox destroy, K8s container delete, git worktree removal) goes through `cleanup()`. Factories may run janitors for abandoned workspaces.

40. **`BudgetHandle<'a>` is the single budget access type.**  
    Multiple `&mut BudgetLedger` references across `ProposalContext`, `EvalHandle`, etc. would be borrow-hostile. Stages now receive `BudgetHandle<'a> { ledger: &'a mut BudgetLedger, stage: StageId }` — one type, one mutable borrow path, stage tag baked in.

41. **Proposal validation laws (§24).**  
    Cheap correctness checks before graph insertion: `Create + None` ok; `Create + NAry` ok (aggregate); `Create + Single` invalid; `Change + Single` requires `target == single parent`; `Change + Pair` requires `target ∈ pair`; `Change + None` invalid. These prevent bad lineage data from entering the graph.

---

## 0.4 What Changed in v0.2.1c

The v0.2.1b topology cutover made surfaces explicit, but the long-form spec still carried v0.2.1a examples and a few underspecified seams. This pass tightens those seams before `leaven-gepa` work.

42. **Artifact identity and cache identity are separate.**  
    `ArtifactIdentity` is graph identity. `CacheIdentity` is the stronger evaluator-cache promise. Deterministic cache keys use `CacheIdentity`, not whatever identity an artifact happens to expose.

43. **Artifact-intrinsic decomposition is removed.**  
    `Decomposable` is gone from the main spec. Parts are exposed by `EditSurface<A>`, which is selected by an optimizer, proposer, renderer, or adapter. Surface part IDs are scoped to a `SurfaceFingerprint`.

44. **Evidence splits measurement from attribution.**  
    `CasewiseEvidence` expresses per-case outcomes. `AttributableEvidence<K>` expresses blame/credit/routing over a key space such as surface parts, tools, agents, files, or changesets. GEPA instance frontiers use casewise measurement; trace-aware selectors use attribution.

45. **GEPA owns its edit surface and lowers surface edits.**  
    The canonical shape is `Gepa<P, S, Pop>` where `S: EditSurface<P::Artifact>`. GEPA proposers may emit surface edits; GEPA lowers them through `S` into artifact-native changes before recording `ProposalEffect::Change`.

46. **Cost arithmetic is checked by default.**  
    `Cost` does not silently saturate through `+`. Stages use `checked_add` / `checked_add_assign` and propagate `CostOverflow`; explicit `saturating_add` exists only for non-authoritative reporting.

47. **Population observation is optimizer-driven.**  
    The engine records assessments into the graph. The optimizer decides which population or fitted model observes each assessment and records resulting population events.

48. **Pairwise tournament is now a worked example.**  
    The example exercises pairwise evaluation requests, fitted preference state, optimizer-driven observation, and a non-GEPA rhythm.

---

## 0.5 What Changed in v0.2.2

This is a minor spec bump because it changes public vocabulary and algorithm
extension seams before implementation.

49. **`WorkspaceRenderer` is renamed to `Materializer`.**  
    Value rendering and workspace materialization are different operations.
    `Renderer` returns values for ordinary LM calls, debug views, and typed
    blobs. `Materializer` is the v0.2.2 workspace bridge for agents, sandboxed
    evaluators, and subprocess tools. It is not deferred; only erased
    registry/dyn dispatch for renderers/materializers is deferred. There is no
    compatibility alias.

50. **Workspace paths are backend-neutral.**  
    Public workspace APIs use `WorkspacePath`, not host `PathBuf` or raw
    strings. Local and E2B-style remote backends share the same file and command
    surface; `local_mount()` is optional and never required by examples.

51. **Trust and capability scopes are enumerated by actor.**  
    The spec now states exactly what optimizers, proposers, evaluators,
    renderers, materializers, agent runtimes, and callbacks can read or do.

52. **GEPA candidate selection is explicitly swappable.**
    `Population` is archive/admission/update state. `CandidateSelector` is the
    policy that chooses which candidate to mutate next from a typed `PopulationView`. This is
    required by GEPA variants, MAP-Elites/quality-diversity, skill-library
    evolution, and tournament optimizers.

53. **Future skill-library optimizers get a named direction.**  
    The bottom of the spec records the likely `leaven-skill` extension slots:
    skill routing, hard-case selection, skill-target selection, and skill
    admission.

---

## 0.6 What Changed in v0.2.3

This is a minor spec bump because it fixes the public meaning of
`leaven-agent` and `leaven-agentic` before provider adapters land.

54. **Agent runtimes execute sessions, not optimizers.**  
    `AgentRuntime` is provider-neutral execution vocabulary over a workspace:
    request, output contract, transcript, status, cost, and capability
    declaration. It does not know `OptimizationProblem`, `CandidateId`,
    `ProposalBatch`, `Assessment`, `RunGraph`, or GEPA.

55. **Agentic stages own the adapter semantics.**  
    Agentic proposers and evaluators compose materializers, renderers, runtimes,
    and parsers. They convert session outputs into typed `ProposalBatch` or
    `Assessment` values; runtimes only report what happened.

56. **Semantic agent wiring belongs in artifacts when optimized.**  
    Harness code, skills, enabled skill lists, skill mounts, tool policy names,
    `AGENTS.md`, and manifests are artifact state if changing them changes the
    candidate. Workspace path layout, commands, and output contracts are
    materializer/stage configuration.

57. **Runtime workspace requirements are explicit.**  
    Backend-neutral runtimes use `WorkspacePath`, file APIs, and `run_command`.
    Runtimes that require `local_mount()` must declare that capability and fail
    early on pure-remote backends such as E2B.

The full companion contract is
`docs/specs/agentic_stage_runtime.md`.

The optimizer-stage workspace materialization companion is
`docs/specs/agentic_stage_materialization.md`. It separates
`AgentStagePlan` / `AgentBacked` / `StageAttemptReceipt` from the
candidate-evaluation `AgentCase` / `AgentWorkload` substrate.

---

## 0.7 What Changed in v0.2.4

This is a minor spec bump because agentic skill optimization needs real
folder-shaped artifacts, validation, and workspace parsing contracts before paper
reproduction work can be meaningful.

58. **Agent Skills folders are first-class artifacts.**  
    A skill is a directory with mandatory `SKILL.md` frontmatter (`name`,
    `description`) and non-empty Markdown body. Optional and provider-specific
    frontmatter lives in a generic metadata bag rather than baked-in fields.

59. **Skill mutations are filesystem-native.**  
    Skills may contain scripts, references, assets, and arbitrary files.
    Rewrites are allowed; `RenameSkill` and `ReplaceSkill` are explicit
    changes; executable bits are preserved as file metadata without making
    executable files a separate semantic type.

60. **Invalid proposals do not create candidates.**  
    Apply/validation failure records an attempt and returns a typed error.
    Bounded repair/reproposal is same-proposer stage policy before a
    `ProposalBatch` is returned, not hidden engine behavior.

61. **Workspace proposal parsing is stage-owned.**  
    Agentic proposers may parse edited workspaces into typed proposals through
    their `ProposalParser`; parsers do not mutate the graph.

The full companion contract is
`docs/specs/agentic_skill_optimization_primitives.md`.

---

## 0.8 What Changed in v0.2.5

This is a minor spec bump because the first real provider adapter needs a
precise boundary before implementation.

62. **Codex app-server is a concrete provider runtime, not an engine concept.**  
    `leaven-agent-codex-app-server` implements `AgentRuntime` over an already
    materialized workspace. `leaven-agent-codex` is only the Codex provider
    facade. Neither crate knows optimizer, graph, proposal, assessment, skill,
    or git vocabulary.

63. **Codex app-server dependencies are leaf-only.**  
    `codex-app-server-protocol` is confined to
    `leaven-agent-codex-app-server` and is feature-gated from
    umbrella/facade crates.

64. **The stdio Codex connector requires a local mount.**  
    `StdioCodexAppServerConnector` fails early on pure-remote workspaces.
    Backend-neutral remote Codex execution waits for a real connector that can
    run app-server inside the backend or read back a provider-managed snapshot.

65. **Transcript and output-contract mapping are specified.**  
    Assistant messages, commands, tool calls, raw provider events, output
    files, status, and provider errors now have a concrete mapping into
    `AgentSession`.

66. **Codex skill layout has a provider owner.**  
    Codex-specific workspace layout and `UserInput::Skill` references are
    provider ABI owned by `leaven-agent-codex-app-server`; `SkillBank`,
    `SKILL.md` validation, and skill mutations remain outside the runtime.

The full companion contract is
`docs/specs/codex_app_server_agent_runtime.md`.

---

## 0.9 What Changed in v0.2.6

This is a minor spec bump because the agentic-skill substrate now has a live
EvoSkill-shaped proof path, not only a design contract.

67. **P5 is a live EvoSkill iteration, not a toy paper map.**  
    `examples/p5_evoskill_iteration` must run one real create/edit skill
    iteration over `SkillBank`, materialized Agent Skills folders, workspace
    proposal parsing, `RunContext` proposal application, evaluator evidence,
    population update, and checkpoint/resume.

68. **Live Codex is part of the P5 gate.**  
    `just milestone-p5` runs Codex CLI with `gpt-5.4-mini` and low reasoning
    when `LEAVEN_CODEX_LIVE=1` is set. The gate must include
    developer instructions and stored session evidence.

69. **Source-prompt fidelity is explicit.**  
    Paper role prompts may be copied from source and wrapped with a small
    Leaven/Codex ABI contract. That is source-prompt faithful; it is not a
    claim of full paper reproduction until the paper datasets, splits, frontier
    loop, feedback history, graders, and ablations are present.

70. **Provider no-shell stages are valid.**  
    Codex-backed stages may use `OutputContract::FinalMessage` and no shell
    tools when the stage contract is typed proposal/evidence output. Tool and
    command evidence are required only for stages that actually grant and expect
    tool use.

The live proof and remaining paper gaps are tracked in
`docs/plans/2026-05-07-milestone-5-skill-paper-reproductions.md`.

---

## 0.10 What Changed in v0.2.7

This is a minor spec bump because backend-neutral agent execution must be the
default product path before paper reproduction work can scale beyond local
host-mounted app-server experiments.

71. **Provider CLIs run through the workspace backend.**  
    The default product path is materialize workspace, write provider-native
    setup files, run the provider CLI through `WorkspaceView::run_command`,
    capture native logs/session files, validate the output contract, and let a
    stage parser translate the result into proposal or evidence.

72. **Codex app-server stdio is not the container default.**  
    `leaven-agent-codex-app-server` remains a leaf provider adapter. Its stdio
    connector requires a host-local mount and is therefore a local
    compatibility path, not the backend-neutral Codex path for E2B, K8s,
    Firkin, Firecracker, or other remote/container backends.

73. **Codex CLI is the implemented backend-neutral Codex path.**  
    `leaven-agent-codex-cli` delegates to `leaven-agent-command`, invokes
    `codex exec` through `WorkspaceView::run_command`, captures
    `--output-last-message` as the normalized final response, and retains JSONL
    stdout/stderr as raw provider events. It does not own skill layout.

74. **Workspace command execution is now a first-class law surface.**  
    Commands carry workspace cwd, env, stdin, timeout/output limits, optional
    user identity, exit status, duration, and truncation facts. Backends must
    either honor requested semantics or refuse with typed errors.

74. **Runtime setup files are not artifact state by default.**  
    Provider homes, skill registrations, MCP config, stdout/stderr logs, and
    native session files are operational presentation unless materialized from
    candidate state or parsed back into typed proposals.

75. **Durable session data is required for resume.**  
    `AgentSession`, transcript, command records, output paths, artifact files,
    and raw provider event counts must be serializable enough for stored
    evidence and checkpoints. Examples should stop inventing bespoke session
    persistence once this lands.

The implementation plan is
`docs/plans/2026-05-07-harbor-style-agent-runtime.md`.

---

## 1. Executive Summary

We are building a Rust library for writing optimizers over arbitrary artifacts whose behavior can be assessed.

The library should support GEPA-style reflective prompt evolution, but must not be a GEPA-only engine. It should also support MIPRO-like surrogate optimizers, TextGrad/Trace-style feedback propagation, MAP-Elites, island evolutionary code search, pairwise-tournament preference optimization, skill-library evolution, agentic proposers, recursive meta-optimization, and future optimizers we have not read yet.

The library’s cold core should not assume:

```text
candidates are text dictionaries
evaluation returns scalar scores
selection is Pareto
proposals are one-shot LLM calls
evidence is an agent trajectory
"accept/reject" is a universal candidate lifecycle
a frontier is always maintained
train/validation exists
rendering is precomputed
every optimizer has GEPA's loop shape
```

The core should provide:

```text
typed artifacts and fallible typed changes
graph-local candidates
proposal batches
independent / pairwise / listwise evaluation requests
opaque evidence
preference relations over evidence
populations/frontiers as optimizer-owned live state
explicit rendering as the bridge from opaque values to consumers
budget/cost accounting across all stages
callbacks/events
caching hooks
trust/capability boundaries for agentic stages
a first-class Optimizer trait for algorithm authors
```

GEPA is one optimizer value, not the engine. It is composed from smaller GEPA-specific strategies: candidate selector, part selector, batch sampler, reflector/proposer, acceptance policy, validation policy, population/frontier, and optional merge proposer.

---

## 2. Design Philosophy

### 2.1 The consumer model

The library has three first-class consumer groups.

#### End users optimizing something

They want a short, obvious path:

```rust
let result = optimize(seed)
    .train(train_cases)
    .validation(validation_cases)
    .score(my_scoring_function)
    .using(Gepa::default().with_reflection_lm(reflection_lm))
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

They should not have to understand every internal trait.

#### GEPA customizers

They want to replace one part of GEPA:

```rust
let gepa = Gepa::default()
    .candidate_selector(ParetoFrequencyWeighted)
    .surface(PartMapSurface::default())
    .part_selector(InvokedAndFailingPart::default())
    .batch_sampler(EpochShuffled::new(4))
    .acceptance(StrictImprovement)
    .population(ParetoFrontier::by_case())
    .merge(SystemAwareMerge::adaptive());
```

They should not have to write a new optimizer.

#### Optimizer authors

They want to implement a new optimizer from a paper or idea:

```rust
impl Optimizer<MyProblem> for MyOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, MyProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        // own the algorithm rhythm
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, MyProblem>,
    ) -> Option<CandidateId> {
        // choose final answer
    }
}
```

They must be first-class. If implementing TextGrad, MIPRO, pairwise tournaments, or AlphaEvolve requires contorting the algorithm into GEPA’s sequence, the design failed.

### 2.2 Rust-library design standard

The library should feel like a serious Rust crate:

```text
precise names
honest types
explicit failure
typed capability boundaries
typed events
minimal magic
clear object-safety policy
async by default
ergonomic builders
well-documented trait laws
examples that show how to implement real optimizers
```

The core should be small, but not artificially tiny. A few sharp concepts are better than fewer overloaded ones.

### 2.3 Model-legibility

A competent model should be able to read a paper and map its concepts to library concepts:

```text
candidate selection -> CandidateSelector
Pareto frontier -> ParetoFrontier
niche -> NicheDescriptor / MapElites
pairwise judge -> EvaluationRequest::Pairwise
Bradley-Terry -> BradleyTerryFit (model object on TournamentPopulation)
train/val split -> CaseSet partitions + EvaluationSet
```

Naming is not polish; it is infrastructure.

---

## 3. Nomenclature

| Domain concept | Use this name | Avoid / demote | Reason |
|---|---|---|---|
| Thing being optimized | `Artifact` | `Candidate` | Candidate is a run-local artifact state. Artifact is the domain value. |
| Typed modification | `Change` | `Rewrite` | “Rewrite” is text-biased. `Change` works for files, code, configs, weights, prompts. |
| Artifact state inside a run | `Candidate` | `Snapshot`, `Node` | Optimizer literature says candidate. Graph storage may call it node internally. |
| Content identity | `ContentId` | `SnapshotId` | Same content can be reached by different causal paths. |
| Graph-local candidate identity | `CandidateId` | `SnapshotId` | A candidate is an occurrence in a run. |
| Attempted change | `Proposal` | `Edge` | Proposal exists before apply. Edge exists only after apply succeeds. |
| Multiple proposals from one call | `ProposalBatch` | `Parallel proposals` | Preserves sibling alternatives from one context. |
| Evaluation request | `EvaluationRequest` | `Metric call` | Evaluation may be scalar, pairwise, listwise, mixed. |
| Evaluation result | `Assessment` | `Score` | Assessment can contain any evidence shape. |
| Granularity of assessment | `AssessmentGranularity` | implicit per-case/per-set behavior | GEPA needs per-case; some optimizers need aggregate. |
| Opaque evaluation payload | `Evidence` | `Trace`, `SideInfo`, `Feedback` | Those are specific evidence shapes or renderings. |
| “Which is better?” logic | `PreferenceRelation` | `Comparator`, `Score` | Evidence is not preference. Preference consumes evidence. |
| Live optimizer state | `Population` | `Archive` | Population matches evolutionary/search literature and is intuitive. |
| Non-dominated live set | `Frontier`, `ParetoFrontier` | generic `ArchivePolicy` | If it is a Pareto frontier, say so. |
| Frontier partition | `Niche` | `Cell`, `Slice::Niche` | Niche is the MAP-Elites / quality-diversity term. |
| Where to evaluate | `EvaluationSet` | `Slice`, `Cohort` | EvaluationSet is direct. Cohort is removed. |
| Chooses candidates to evolve | `CandidateSelector` | old parent-framed naming | Matches upstream GEPA (`CandidateSelector` / `select_candidate_idx`). Framing the selected candidate only as a parent presumes the next stage produces a child, which is a property of what the proposer does, not of selection itself. |
| Acceptance/admission decision | `Acceptance` | `Gate`, core `Decision` | Acceptance is optimizer-local and says whether the child is good enough to keep or validate. |
| Full algorithm value | `Optimizer` | `SearchStrategy` | Optimizer is the domain word. |
| Opaque-to-visible bridge | `Renderer` / `Materializer` | `make_reflective_dataset`, global `RenderedView` | Rendering/materialization is consumer-specific, not GEPA-specific. |
| Typed proposal payload | `ProposalAnnotations` | `Meta` / `Claims` split | One typed semantic payload. Claims are a capability on annotations. |
| Debug/operational extras | `MetadataBag` | `Note` | Metadata is non-semantic, extensible, and not read by algorithms by default. |

---

## 4. Architecture Overview

### 4.1 One-sentence architecture

> The engine runs an optimizer over typed artifacts; the optimizer uses a context to apply proposals, request evaluations, compare candidates, render views, update populations, and record events into an append-only run graph.

### 4.2 Ownership split

#### Engine owns infrastructure

```text
RunGraph
BudgetLedger
EvaluatorRegistry
EvaluationCache
Callback list
Stopper list
RunStore / checkpointing
TrustPolicy
iteration envelope
external stoppers
RNG seed / run identity
```

#### Optimizer owns algorithm rhythm

```text
which candidates to mutate
whether to evaluate before proposing
whether to propose one or many candidates
which evaluation requests to issue
when to update a population/frontier
when to call merge/crossover
whether to use a gate
when it considers itself done
which candidate is best
```

#### Stages own side-effectful work

```text
evaluator runs artifact(s) against the world
proposer produces changes
renderer creates views for consumers
agent runtime operates in a workspace/sandbox
preference relation interprets graph evidence
```

### 4.3 Engine policy: structured envelope, flexible step

The engine has a structured lifecycle:

```text
optimization started
optimizer initialized
while not stopped:
    check external stoppers
    iteration started
    optimizer.step(ctx)
    iteration ended
optimization ended
```

Inside `optimizer.step(ctx)`, the optimizer may call context methods in any order:

```text
ctx.propose(...)
ctx.apply(...)
ctx.evaluate(...)
ctx.render(...)
ctx.compare(...)
ctx.record_population_events(...)
ctx.emit(...)
ctx.charge(...)
```

The engine provides the envelope. The optimizer drives the algorithm. Context methods centralize graph, budget, cache, trust, and callback correctness.

---

## 5. Core Concepts

### 5.1 `Artifact`

An artifact is the domain value being optimized.

```rust
pub trait Artifact: Clone + Send + Sync + 'static {
    type Change: Clone + Send + Sync + 'static;
    type ApplyError: std::error::Error + Send + Sync + 'static;

    /// Stable identity of this artifact state for graph storage and lineage.
    /// This is not automatically an evaluation-cache key.
    fn identity(&self) -> ArtifactIdentity;

    /// Apply a typed change. Must be functional: same artifact + same change
    /// either fails the same way or produces the same artifact identity.
    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError>;
}
```

`Artifact` does not know about scores, evidence, traces, rationale, claims, cases, or rendering.

Artifact identity is graph identity:

```rust
pub enum ArtifactIdentity {
    /// Collision-resistant content identity.
    Content(ContentId),

    /// Stable external identity used for graph lineage. This may or may not
    /// be safe for evaluation caching.
    External(ExternalRef),
}
```

Evaluation cache identity is separate:

```rust
pub trait CacheIdentified: Artifact {
    /// Identity that the deterministic evaluator cache may trust.
    fn cache_identity(&self) -> Option<CacheIdentity>;
}

pub enum CacheIdentity {
    /// The artifact state is content-addressed by this digest.
    Content(ContentId),

    /// The external reference is immutable by law for this artifact type.
    /// Examples: git commit hash, IPFS CID, OCI image digest.
    ExternalContent(ExternalRef),

    /// Caller-supplied cache fingerprint. Use only when the user has supplied
    /// an explicit stable key for the evaluated state.
    User(Fingerprint),
}
```

`ArtifactIdentity` answers "what state did the graph record?" `CacheIdentity`
answers "what may an evaluator cache reuse without re-running?" Mutable external
handles such as branch names, filesystem paths, database row IDs, or S3 keys
without versioning are valid graph identities but must return `None` for
`cache_identity()` unless wrapped in an immutable snapshot or explicit user key.

Artifact parts are not intrinsic. Use `leaven-surface::EditSurface<A>` for any
chosen projection over an artifact:

```rust
pub trait EditSurface<A: Artifact>: Send + Sync {
    type PartId: Eq + Hash + Clone + Send + Sync + 'static;
    type Address: Eq + Hash + Clone + Send + Sync + 'static;
    type View<'a>: Send + Sync where A: 'a;
    type Edit: Clone + Send + Sync + 'static;

    fn fingerprint(&self) -> SurfaceFingerprint;
    fn parts<'a>(&self, artifact: &'a A) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError>;
    fn change_part(&self, artifact: &A, id: Self::PartId, edit: Self::Edit) -> Result<A::Change, SurfaceError>;
}
```

Surface laws:

- Surface identity is scoped to `SurfaceFingerprint`, not to the artifact type.
- `S::PartId` is meaningful only for the surface fingerprint that produced it.
- Path-based surfaces preserve path identity only; rename is remove + add.
- Logical-ID surfaces may preserve identity across rename if their extraction rule says so.
- Borrowed surface views must not be held across `.await`; async stages should turn them into owned request/rendering data before awaiting.

Workspace materialization is separate from value rendering. Use
`Materializer<P, ArtifactType>` or a stdlib materializer helper; do not put
workspace layout on `Artifact`.

### 5.2 `ContentId` and `CandidateId`

These are distinct.

```rust
pub struct ContentId([u8; 32]);
pub struct CandidateId(Uuid);
```

`ContentId` means artifact content identity.  
`CandidateId` means occurrence in this run graph.

The same content can appear multiple times in the graph via different proposals. That preserves causal history.

### 5.3 `Candidate`

A candidate is a graph-local artifact state.

```rust
pub struct Candidate<A: Artifact> {
    pub id: CandidateId,
    pub identity: ArtifactIdentity,
    pub artifact: A,
}
```

Candidates are created by successful proposal application or by seeding the run.

### 5.4 `OptimizationProblem`

Use one bundle for run-associated types.

```rust
pub trait OptimizationProblem {
    type Artifact: Artifact;
    type Case: Send + Sync + 'static;
    type Evidence: Evidence;
    type ProposalAnnotations: Clone + Send + Sync + 'static;
}
```

This keeps strategy signatures legible:

```rust
impl Optimizer<MyProblem> for MyOptimizer { ... }
```

Mixed evidence or annotations are represented with user-defined enums:

```rust
pub enum MyEvidence {
    Score(ScoreVectorEvidence),
    Pairwise(PairwiseJudgmentEvidence),
    AgentTrace(AgentTrajectory),
}

pub enum MyAnnotations {
    None,
    Reflection(ReflectionAnnotations),
    Edit(EditAnnotations),
    Merge(MergeAnnotations),
}
```

This is deliberate. The run-wide types tell the truth about all shapes that may occur in the run.

### 5.5 `Proposal`

A proposal is one record of "do this thing, with this lineage and this rationale." It separates *what to do* (effect) from *what informed it* (provenance) from *how to interpret it* (annotations) from *operational extras* (metadata).

```rust
pub struct Proposal<P: OptimizationProblem> {
    pub effect: ProposalEffect<P>,
    pub provenance: ProposalProvenance,
    pub annotations: P::ProposalAnnotations,
    pub metadata: MetadataBag,
}
```

#### Effect: what this proposal does

```rust
pub enum ProposalEffect<P: OptimizationProblem> {
    /// Brand-new authored artifact, no apply target.
    /// Used when the proposer constructs the artifact directly rather than
    /// transforming an existing candidate.
    /// Examples: Meta-Harness fresh harness each iteration; MIPRO initial
    /// surrogate sampling; ensemble aggregates that combine N → 1.
    Create {
        artifact: P::Artifact,
    },

    /// Mutation applied to an existing candidate.
    /// Examples: GEPA reflective mutation, TextGrad per-variable updates,
    /// AlphaEvolve code edits, MuF/Edit, merge (canonicalized — see below).
    Change {
        target: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    },
}
```

`Change` covers the dominant case. `Create` exists because faking "no parent" as a `Change` requires inventing a null artifact value and pretending the apply step is `null.apply(replace_with: x) -> x`, which is a lie.

#### Provenance: causal lineage and bibliographic influence

```rust
pub struct ProposalProvenance {
    pub causal: CausalInputs,
    pub informed_by: Vec<InfoRef>,
}

pub enum CausalInputs {
    /// No causal predecessor. The proposal is `Create`.
    None,

    /// One causal parent. Standard mutation.
    Single(CandidateId),

    /// Two causal parents. Merge/crossover.
    /// One is the apply target (recorded in ProposalEffect::Change.target);
    /// the change embeds content sourced from the other.
    Pair(CandidateId, CandidateId),

    /// N-ary causal inputs. Uncommon; for ensemble aggregates and similar.
    NAry(Vec<CandidateId>),
}

pub enum InfoRef {
    Candidate(CandidateId),
    Assessment(AssessmentId),
    Proposal(ProposalId),
    External(ExternalRef),
}
```

`causal` records what the proposal was *derived from* — these contributed to the new candidate's artifact state. `informed_by` records what the proposer *read while deciding* — these did not become causal inputs. The distinction matters for cache correctness (informed-by candidates do not become candidate cache identities for the child) and for graph queries ("what learnings are incorporated into this candidate's lineage" vs "what was used to construct it").

`informed_by` is a typed structured field, not metadata. Graph queries `graph.informed_by(c)` and `graph.informed(c)` derive directly from this. Implementations are expected to populate it honestly — agentic proposers that read prior candidates must record those reads.

#### Constructor sugar

Users rarely construct the full `Proposal` struct. The common cases are covered by builders:

```rust
impl<P: OptimizationProblem> Proposal<P> {
    /// Standard mutation. Sets effect = Change, causal = Single(target).
    pub fn mutate(
        target: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    ) -> ProposalBuilder<P>;

    /// Merge of two candidates. Sets effect = Change { target: a, change },
    /// causal = Pair(a, b). The change must already embed content from b.
    pub fn merge(
        a: CandidateId,
        b: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    ) -> ProposalBuilder<P>;

    /// Brand-new authored artifact. Sets effect = Create, causal = None.
    pub fn create(artifact: P::Artifact) -> ProposalBuilder<P>;

    /// Aggregate of N candidates into a new artifact. Sets effect = Create,
    /// causal = NAry(parents).
    pub fn aggregate(parents: Vec<CandidateId>, artifact: P::Artifact) -> ProposalBuilder<P>;
}

pub struct ProposalBuilder<P: OptimizationProblem> { /* … */ }

impl<P: OptimizationProblem> ProposalBuilder<P> {
    pub fn informed_by<I: IntoIterator<Item = InfoRef>>(self, refs: I) -> Self;
    pub fn annotations(self, ann: P::ProposalAnnotations) -> Self;
    pub fn metadata(self, bag: MetadataBag) -> Self;
    pub fn build(self) -> Proposal<P>;
}
```

Typical usage:

```rust
// GEPA reflective mutation
Proposal::mutate(parent, change)
    .informed_by([InfoRef::Candidate(parent)])
    .annotations(reflection_notes)
    .build()

// GEPA merge
Proposal::merge(a, b, change_with_content_from_b)
    .informed_by([InfoRef::Candidate(a), InfoRef::Candidate(b)])
    .build()

// Meta-Harness fresh harness
Proposal::create(new_harness_artifact)
    .informed_by(referenced_candidates.iter().map(|&c| InfoRef::Candidate(c)))
    .annotations(proposer_notes)
    .build()
```

#### Cost is not on the proposal

No cost field is required here. Costs are recorded through stage invocations and `BudgetLedger`. A proposal may optionally include cost allocation metadata for analysis, but cost truth lives in the ledger.

#### Merge canonicalization

`Artifact::apply(&self, change) -> Self` only sees one artifact. So `Proposal::merge(a, b, change)` produces:

- `effect: ProposalEffect::Change { target: a, change }` — applied to `a` only
- `causal: CausalInputs::Pair(a, b)` — both contributed

The change must already embed any content the merge proposer wanted to import from `b`. The merge proposer reads `b` via the run graph during proposal generation, extracts the relevant components, and packages their content into the change. The framework records `Pair(a, b)` for lineage queries; the apply step ratifies what the merge proposer constructed.

#### Annotations are typed

The core does not distinguish `Meta` from `Claims`. If annotations have predictions or behavioral claims, they implement capability traits:

```rust
pub trait HasPredictions<P: OptimizationProblem> {
    fn predictions(&self) -> &[Prediction<P>];
}
```

```rust
pub trait HasBehavioralClaims {
    fn should_fix(&self) -> &str;
    fn should_not_break(&self) -> &str;
    fn confidence(&self) -> Confidence;
}
```

MuF/Edit-style annotations:

```rust
pub struct EditAnnotations {
    pub rationale: String,
    pub rhetorical_strategy: String,
    pub should_fix: String,
    pub should_not_break: String,
    pub rollback_note: String,
    pub confidence: Confidence,
}
```

### 5.6 `MetadataBag`

Operational metadata is separate from typed annotations.

```rust
pub struct MetadataBag {
    pub fields: BTreeMap<MetadataKey, MetadataValue>,
}
```

Floating-point metadata values use `FiniteF64`, not raw `f64`, so operational
debug data cannot inject `NaN` or infinity into serialization, reporting, or
cache-adjacent tooling. Use `Amount` for non-negative cost/budget quantities;
use `FiniteF64` only where the sign is domain-specific.

Use metadata for:

```text
raw response refs
worker IDs
stdout/stderr blob refs
rendered prompt blob refs
human comments
hostnames
trace file locations
diagnostic breadcrumbs
```

Metadata is for debugging and observability. Optimizer logic should depend on typed annotations, not ad hoc metadata, unless explicitly designed otherwise.

### 5.7 `ProposalBatch`

Proposal batches are first-class. A batch groups proposals that came from one reflection context — one `propose()` call.

```rust
pub struct ProposalBatch<P: OptimizationProblem> {
    pub proposals: Vec<Proposal<P>>,
    pub semantics: ProposalBatchSemantics,
    pub metadata: MetadataBag,
}
```

Each proposal carries its own `effect` and `provenance`. The batch does not carry causal inputs; sibling proposals from one reflection context may have entirely different causal lineages (or none at all). The batch only records *that they came from one context* and how they should be evaluated relative to each other.

The **cost of creating the batch** is recorded as a stage cost by `ctx.propose(...)` or `ctx.charge(...)`.

```rust
pub enum ProposalBatchSemantics {
    /// Sibling alternatives from one context.
    /// All alternatives are evaluated independently if applied successfully.
    /// Cost is N×eval, not amortized — the framework does not deduplicate.
    Alternatives,

    /// Candidate pool; optimizer/engine may evaluate only a subset by budget.
    CandidatePool,
}
```

`Ordered` (sibling proposals where later ones depend on earlier ones) was considered but removed in v0.2.1. Multi-batch optimizer rhythm covers ordered-dependency cases — the optimizer issues one batch, applies, then issues another batch using the new candidates as parents. Re-introducing `Ordered` would require the framework to interleave application with proposal generation, which is the optimizer's responsibility, not the engine's.

Important distinction:

```text
Alternatives = multiple independent proposals from one call
atomic multi-edit = one proposal whose Change contains multiple operations
```

Example user change type:

```rust
pub enum AgentChange {
    Single(AgentPatch),
    PatchSet {
        patches: Vec<AgentPatch>,
        atomic: bool,
    },
}
```

### 5.8 `CaseSet`

Generalization mode is represented by explicit partitions.

```rust
pub struct CaseSet<C> {
    pub cases: IndexMap<CaseId, C>,
    pub partitions: BTreeMap<PartitionId, Vec<CaseId>>,
    pub tags: BTreeMap<Tag, Vec<CaseId>>,
    pub version: CaseSetVersion,
}
```

Reserved partitions:

```rust
PartitionId::TRAIN
PartitionId::VALIDATION
PartitionId::TEST
```

Modes:

```text
single-task: no case set, singleton case set, or EvaluationSet::Unscoped
multi-task: TRAIN partition only
generalization: TRAIN and VALIDATION, with trust boundaries controlling proposer access
true test: TEST, usually evaluator-only and not visible to proposer
```

### 5.9 `EvaluationSet`

An evaluation set is where/what to evaluate.

```rust
pub enum EvaluationSet {
    /// No dataset scope. Useful for single-task or evaluator-internal tasks.
    Unscoped,

    All,

    Partition(PartitionId),

    Cases(Vec<CaseId>),

    Tagged(Tag),

    Recent {
        window: Window,
    },

    Sample {
        of: Box<Self>,
        n: usize,
        seed: u64,
    },

    Stratified {
        of: Box<Self>,
        by: Tag,
        k: usize,
        seed: u64,
    },

    Union(Vec<Self>),

    Intersect(Vec<Self>),

    Difference(Box<Self>, Box<Self>),
}
```

Evaluation sets are resolved before reaching the evaluator:

```rust
pub struct ResolvedEvaluationSet {
    pub id: ResolvedEvaluationSetId,
    pub expr: EvaluationSet,
    pub case_ids: Vec<CaseId>,
    pub resolved_at: DateTime<Utc>,
    pub case_set_version: CaseSetVersion,
}
```

#### Resolution boundary

The boundary between unresolved and resolved sets is sharp:

```text
Optimizer constructs EvaluationRequest with EvaluationSet (possibly dynamic).
RunContext::evaluate resolves the set:
  - Static variants (All, Partition, Cases, Tagged) resolve trivially.
  - Dynamic variants (Recent, Sample, Stratified) compute case_ids ONCE
    against the current case-set version, then freeze into ResolvedEvaluationSet.
  - Compositional variants (Union, Intersect, Difference) resolve recursively.
RunContext records the ResolvedEvaluationSet in the graph alongside the
  original EvaluationSet expression — both are queryable.
RunContext passes a ResolvedEvaluationRequest to the evaluator. Evaluators
  do not see EvaluationSet expressions; they see resolved case_ids.
Cache key uses (evaluator_fingerprint, ResolvedEvaluationSetId, case_set_version,
  candidate cache identities). Dynamic sets at different times resolve to different
  ResolvedEvaluationSetIds and therefore different cache entries.
```

```rust
pub struct ResolvedEvaluationRequest<'a> {
    pub kind: ResolvedRequestKind,
    pub resolved_set: &'a ResolvedEvaluationSet,
    pub granularity: AssessmentGranularity,
    pub purpose: EvaluationPurpose,
}

pub enum ResolvedRequestKind {
    Independent { candidates: Vec<CandidateId> },
    Pairwise { left: CandidateId, right: CandidateId, order: PairOrder },
    Listwise { candidates: Vec<CandidateId> },
}
```

This means the same dynamic `EvaluationSet::Recent { window: Duration::hours(1) }` issued at iteration 5 and iteration 6 produces two different `ResolvedEvaluationSet`s — they're snapshots, not lazy queries. The graph remembers both expressions and both resolutions. Cache hits across iterations are possible only if the resolution actually matches.

### 5.10 `AssessmentGranularity`

GEPA needs per-case assessments to build instance-wise Pareto frontiers. Some evaluators only produce aggregate assessments. The request must say what is wanted.

```rust
pub enum AssessmentGranularity {
    /// One assessment for the whole resolved evaluation set.
    Aggregate,

    /// One assessment per case in the resolved evaluation set.
    PerCase,

    /// Return both aggregate and per-case assessments when possible.
    Both,
}
```

If an evaluator cannot provide the requested granularity, it returns an explicit `EvaluationError::UnsupportedGranularity`.

### 5.11 `EvaluationRequest`

Evaluation can be independent, pairwise, or listwise.

```rust
pub enum EvaluationRequest {
    Independent {
        candidates: Vec<CandidateId>,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    },

    Pairwise {
        left: CandidateId,
        right: CandidateId,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
        order: PairOrder,
    },

    Listwise {
        candidates: Vec<CandidateId>,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    },
}
```

This avoids ambiguity:

```text
Independent over [A, B] returns independent assessments for A and B.
Pairwise over (A, B) returns comparison assessment(s).
Listwise over [A, B, C] returns ranking/listwise assessment(s).
```

### 5.12 `AssessmentTarget`

An assessment has a target: unscoped, aggregate set, or case-specific.

```rust
pub enum AssessmentTarget {
    Unscoped,
    EvaluationSet(EvaluationSetId),
    Case {
        set: EvaluationSetId,
        case: CaseId,
    },
}
```

### 5.13 `Assessment`

An assessment is evaluation output.

```rust
pub enum Assessment<P: OptimizationProblem> {
    Independent {
        candidate: CandidateId,
        target: AssessmentTarget,
        evidence: P::Evidence,
        cost: Cost,
        metadata: MetadataBag,
    },

    Pairwise {
        left: CandidateId,
        right: CandidateId,
        target: AssessmentTarget,
        evidence: P::Evidence,
        cost: Cost,
        metadata: MetadataBag,
    },

    Listwise {
        candidates: Vec<CandidateId>,
        target: AssessmentTarget,
        evidence: P::Evidence,
        cost: Cost,
        metadata: MetadataBag,
    },
}
```

This model supports:

```text
aggregate scalar evaluation
per-case scalar evaluation
pairwise comparison per case
pairwise comparison across a set
listwise ranking
mixed evidence
human judge rationales
agent traces
compiler logs
```

### 5.14 `Evidence`

Evidence is opaque to core.

```rust
pub trait Evidence: Send + Sync + 'static {}
```

Evidence does not render itself in cold core. Consumers use `Renderer` or
`Materializer` implementations at the stage boundary.

```rust
pub trait CasewiseEvidence: Evidence {
    fn case_outcome(&self, case: CaseId) -> Option<CaseOutcome>;
    fn case_outcomes(&self) -> Vec<(CaseId, CaseOutcome)>;
}
```

```rust
pub trait AttributableEvidence<K>: Evidence {
    /// The key space this attribution was produced under.
    /// For surface-part attribution this must be
    /// `AttributionDomain::Surface(surface.fingerprint().0)`.
    fn attribution_domain(&self) -> AttributionDomain;

    fn attributions(&self) -> Vec<Attribution<K>>;
    fn evidence_for(&self, key: &K) -> Option<AttributionEvidence<'_>>;
}

pub enum AttributionDomain {
    Surface(Fingerprint),
    ToolCalls,
    Agents,
    Changesets,
    User(Fingerprint),
}

pub struct Attribution<K> {
    pub key: K,
    pub weight: Option<FiniteF64>,
    pub note: Option<Arc<str>>,
}
```

```rust
pub trait CommandEvidence: Evidence {
    fn commands(&self) -> &[CommandRecord];
}
```

```rust
pub trait DiffEvidence: Evidence {
    fn diff_summary(&self) -> Option<RenderedDiff>;
}
```

The core does not require these. Strategies bind what they need.

Casewise measurement and attribution are deliberately separate:

- `CasewiseEvidence` answers "what happened on this case?"
- `AttributableEvidence<K>` answers "which key was responsible or relevant?"

GEPA instance frontiers consume casewise outcomes. Trace-aware part selectors,
TextGrad routing, and multi-agent credit assignment consume attribution. A
single evidence type may implement both, but the traits say different things.
For `AttributableEvidence<S::PartId>`, consumers must check that
`attribution_domain() == AttributionDomain::Surface(surface.fingerprint().0)`
before using the part IDs.

### 5.15 `PreferenceRelation`

Evidence is not preference. A relation interprets evidence.

```rust
pub trait PreferenceRelation<P: OptimizationProblem>: Send + Sync {
    fn compare(
        &self,
        left: CandidateId,
        right: CandidateId,
        scope: PreferenceScope,
        graph: RunGraphView<'_, P>,
    ) -> Preference;
}
```

```rust
pub enum PreferenceScope {
    /// Compare using all visible assessments.
    All,

    /// Compare using assessments from one evaluation set.
    EvaluationSet(EvaluationSetId),

    /// Compare using one case inside one evaluation set.
    Case {
        set: EvaluationSetId,
        case: CaseId,
    },

    /// Compare using assessments recorded for one optimizer purpose.
    Purpose(EvaluationPurpose),
}
```

Composition can be added later as a separate scoped-query type if a prototype
needs it. v0.2.1c keeps `PreferenceScope` to the cases already used by the
spec and avoids a generic filter language inside cold preference APIs.

```rust
pub enum Preference {
    LeftBetter,
    RightBetter,
    Equivalent,
    Incomparable,
}
```

Standard relations:

```text
HigherScoreIsBetter
LowerScoreIsBetter
ParetoPreference
LexicographicPreference
CopelandPreference
BordaPreference
CondorcetPreference
UserDefinedPreference
```

(Fitted preference *models* like `BradleyTerryFit` are not in this list — they live on `Population` impls. See §15.1.)

`Score` is not a cold primitive. Scores are one evidence shape plus one preference relation.

### 5.16 `Population`

A population is live optimizer state: archive membership, admission/update laws,
fitted model state, and strategy events. It is not the policy for choosing what
to try next; selectors own that.

```rust
pub trait Population<P: OptimizationProblem>: Send {
    fn insert_seed(
        &mut self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent>;

    fn observe_candidate(
        &mut self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn observe_assessment(
        &mut self,
        assessment: AssessmentId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn best(
        &self,
        graph: RunGraphView<'_, P>,
    ) -> Option<CandidateId>;

    fn view<'a>(&'a self, graph: RunGraphView<'a, P>) -> PopulationView<'a, P>;
}
```

This supports:

```text
candidate-scored optimizers, where a population observes a candidate after validation
tournament optimizers, where a population observes pairwise/listwise assessments
streaming optimizers, where population changes as fresh assessments arrive
quality-diversity optimizers, where archive state and selection policy are separate
```

Standard populations:

```text
KeepBest
ParetoFrontier
MapElites
BeamPopulation
IslandsPopulation
TournamentPopulation
LenientParetoFrontier
NoveltyPopulation
NoPopulation
```

### 5.17 `Niche`

A niche is a frontier/population partition.

```rust
pub trait NicheDescriptor<P: OptimizationProblem>: Send + Sync {
    type Niche: Eq + Hash + Clone + Send + Sync + 'static;

    fn niche(
        &self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Self::Niche;
}
```

MAP-Elites and related methods use niches. GEPA’s instance Pareto can be represented as a frontier keyed by case ID; that is a frontier partition, not an evaluation set.

### 5.18 Rendering and materialization

Rendering converts opaque core types into consumer-specific values — strings for
prompts, JSON for typed signatures, HTML for human inspection. Materialization
writes structured workspace trees for agents, sandboxed evaluators, and
subprocess-backed tools. Both may be async and costful.

Vanilla LM stages do not need a `Materializer`; they should use `Renderer` to
produce `LmMessages`, prompt strings, typed signature inputs, or other
provider-facing values. Reach for `Materializer` only when the consumer's native
interface is a workspace/filesystem plus commands.

The library splits rendering into two trait families:

- **`Renderer<P, T, Target>`** — value-returning. Used for prompts, summaries, JSON blobs, debug HTML.
- **`Materializer<P, T>`** — side-effecting. Populates a workspace by writing files. Used for materializing artifacts, lineage history, traces, and any structured filesystem layout an agentic or sandboxed stage will read.

Full trait definitions, examples, and trait laws live in §13. The framework does not pre-render anything; consuming stages choose their renderings.

---

## 6. Cost and Budget

### 6.1 Cost is infrastructure

Cost is not proposal metadata. Cost is tracked across all side-effectful stages.

```rust
pub struct Cost {
    pub axes: BTreeMap<CostUnit, Amount>,
}
```

`Amount` values are finite and non-negative by construction. Constructors for
costs and budgets must reject `NaN`, infinities, and negative values before
they can enter budget comparison or durable cost snapshots.

Cost arithmetic is checked by default. Authoritative accounting must not
silently saturate or wrap:

```rust
impl Cost {
    pub fn zero() -> Self;

    pub fn checked_add(&self, other: &Self) -> Result<Self, CostOverflow>;
    pub fn checked_add_assign(&mut self, other: &Self) -> Result<(), CostOverflow>;

    /// For lossy summaries only. Do not use for budget ledger truth.
    pub fn saturating_add(&self, other: &Self) -> Self;
}
```

There is no default `Add<Output = Cost>` implementation in the core surface.
Examples use `checked_add` / `checked_add_assign` so overflow remains visible to
the stage that produced it.

Standard units:

```text
usd
wall_time_ms
cpu_time_ms
input_tokens
output_tokens
cached_input_tokens
llm_calls
tool_calls
metric_calls
subprocesses
```

User-defined units:

```rust
pub struct CostUnit(SmolStr);
```

### 6.2 Budget ledger

```rust
pub struct BudgetLedger {
    // internal
}

impl BudgetLedger {
    pub fn remaining(&self, unit: CostUnit) -> Option<Amount>;

    pub fn charge(
        &mut self,
        stage: StageId,
        cost: Cost,
    ) -> Result<(), BudgetExceeded>;

    pub fn snapshot(&self) -> BudgetSnapshot;
}
```

Every context operation that can spend cost records it.

### 6.3 Metered values

```rust
pub struct Metered<T> {
    pub value: T,
    pub cost: Cost,
}
```

Examples:

```text
Proposer produces Metered<ProposalBatch>
Evaluator produces Metered<Vec<Assessment>>
Renderer produces Metered<R::View>
Materializer produces Metered<MaterializationReport>
AgentRuntime produces Metered<AgentTranscript>
```

Even if the public type stores cost on `Assessment`, the graph also records a `BudgetCharged` event for the stage invocation.

### 6.4 Cost truth

There are three levels:

1. **Stage invocation cost** — authoritative cost charged to the ledger.
2. **Assessment cost** — cost attributable to a returned assessment.
3. **Optional cost allocation** — approximate per-proposal or per-candidate attribution for analysis.

Only the ledger is authoritative.

---

## 7. Engine

### 7.1 Engine shape

```rust
pub struct Engine<P, O>
where
    P: OptimizationProblem,
    O: Optimizer<P>,
{
    problem: P,
    optimizer: O,

    evaluators: EvaluatorRegistry<P>,
    graph: RunGraph<P>,
    budget: BudgetLedger,
    cache: EvaluationCache<P>,

    stoppers: Vec<Box<dyn DynStopper<P>>>,
    callbacks: Vec<Box<dyn DynCallback<P>>>,

    rng: StdRng,
    trust: TrustPolicy,
    store: RunStore<P>,
}
```

Simple builders install one primary evaluator:

```rust
optimize(seed)
    .evaluator(my_evaluator)
```

Advanced users install multiple evaluators:

```rust
optimize(seed)
    .evaluator(EvaluatorId::PRIMARY, task_evaluator)
    .evaluator(EvaluatorId::PAIRWISE_JUDGE, pairwise_judge)
    .evaluator(EvaluatorId::HUMAN_REVIEW, human_review)
```

### 7.2 Engine run loop

```rust
impl<P, O> Engine<P, O>
where
    P: OptimizationProblem,
    O: Optimizer<P>,
{
    pub async fn run(mut self) -> Result<RunResult<P>, EngineError> {
        self.emit(RunEvent::OptimizationStarted { /* ... */ });

        {
            let mut ctx = self.context();
            self.optimizer.initialize(&mut ctx).await?;
        }

        loop {
            if let Some(reason) = self.check_stoppers() {
                self.emit(RunEvent::OptimizationStopping { reason });
                break;
            }

            let iteration = self.graph.next_iteration();
            self.emit(RunEvent::IterationStarted { iteration });

            let step_status = {
                let mut ctx = self.context_for_iteration(iteration);
                self.optimizer.step(&mut ctx).await?
            };

            self.emit(RunEvent::IterationEnded {
                iteration,
                status: step_status.clone(),
            });

            match step_status {
                StepStatus::Continue => {}
                StepStatus::Stop { reason } => {
                    self.emit(RunEvent::OptimizationStopping { reason });
                    break;
                }
            }
        }

        let best = self.optimizer.best_candidate(self.graph.view());
        self.emit(RunEvent::OptimizationEnded { best });

        Ok(RunResult {
            graph: self.graph,
            best,
            budget: self.budget.snapshot(),
        })
    }
}
```

### 7.3 Stop policy

External stoppers are checked before each optimizer step. The optimizer may also stop itself. Budget exhaustion may stop during any context operation.

```rust
pub enum StopReason {
    Stopper {
        id: StopperId,
        message: String,
    },

    Optimizer {
        message: String,
    },

    BudgetExceeded {
        unit: CostUnit,
        requested: Amount,
        remaining: Amount,
    },

    ErrorPolicy {
        message: String,
    },

    UserRequested,

    Composite(Vec<StopReason>),
}
```

The graph records all stop reasons.

---

## 8. Optimizer Author Surface

### 8.1 `Optimizer`

```rust
pub trait Optimizer<P: OptimizationProblem>: Send {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, P>,
    ) -> Result<(), OptimizerError> {
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, P>,
    ) -> Result<StepStatus, OptimizerError>;

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, P>,
    ) -> Option<CandidateId>;
}
```

```rust
pub enum StepStatus {
    Continue,
    Stop { reason: StopReason },
}
```

The optimizer owns algorithm rhythm.

### 8.2 `RunContext`

```rust
impl<'a, P: OptimizationProblem> RunContext<'a, P> {
    pub fn graph(&self) -> RunGraphView<'_, P>;

    pub fn budget(&self) -> BudgetSnapshot;

    pub async fn propose<Pr>(
        &mut self,
        proposer: &Pr,
        request: Pr::Request,
    ) -> Result<ProposalBatchReport<P>, ProposalError>
    where
        Pr: Proposer<P>;

    /// Apply every proposal in a batch. Returns per-proposal apply outcomes.
    /// Per-proposal effects (Create vs Change) and provenance are read from
    /// each Proposal directly; the batch is not parameterized by parents.
    pub async fn apply_batch(
        &mut self,
        batch: ProposalBatch<P>,
    ) -> Result<ApplyReport<P>, ApplyError>;

    /// Apply a single proposal. Convenience for optimizers that don't batch.
    pub async fn apply_proposal(
        &mut self,
        proposal: Proposal<P>,
    ) -> Result<ApplyOneReport<P>, ApplyError>;

    pub async fn evaluate(
        &mut self,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport<P>, EvaluationError>;

    pub async fn evaluate_with(
        &mut self,
        evaluator: EvaluatorId,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport<P>, EvaluationError>;

    pub fn compare(
        &self,
        left: CandidateId,
        right: CandidateId,
        scope: PreferenceScope,
        relation: &dyn DynPreferenceRelation<P>,
    ) -> Preference;

    pub async fn render<R, T, Target>(
        &mut self,
        renderer: &R,
        value: &T,
        target: Target,
    ) -> Result<Metered<R::View>, RenderError>
    where
        R: Renderer<P, T, Target>;

    pub fn record_population_events(
        &mut self,
        population: PopulationId,
        events: Vec<PopulationEvent>,
    );

    pub fn selection_context(&mut self, arity: Arity) -> SelectionContext<'_>;

    pub fn emit(&mut self, event: RunEvent<P>);

    pub fn charge(
        &mut self,
        stage: StageId,
        cost: Cost,
    ) -> Result<(), BudgetExceeded>;
}
```

Context methods handle:

```text
graph writes
budget charges
cache lookup
callback emission
trust policy enforcement
error normalization
event metadata
persistence hooks
```

This is what makes `Optimizer` first-class without making optimizer authors reimplement the engine.

### 8.3 Context capability table

The context types are distinct because their trait signatures should reveal
which powers a stage receives. They are not aliases for one god context.

| Capability | `RunContext` | `ProposalContext` | `EvaluationContext` | `RenderContext` | `MaterializeContext` |
|---|---|---|---|---|---|
| Read graph | yes | scoped | scoped | scoped | scoped |
| Mutate graph | yes | no | no | no | no |
| Apply proposals | yes | no | no | no | no |
| Request normal evaluations | yes | no | no | no | no |
| Request probe evaluations | no | if granted by `EvalHandle` | no | no | no |
| Charge budget | yes | yes | yes | yes | yes |
| Allocate workspace | no | if granted | if granted | no | no |
| Touch workspace files | no | through allocated `Workspace` | through allocated `Workspace` | no | only the provided `WorkspaceView` |
| Run workspace commands | no | via `AgentRuntime` or explicit workspace call | via explicit workspace call | no | no by default |
| Render values | yes | yes | yes | yes | yes |
| Materialize workspace trees | no | yes, if a workspace was allocated | yes, if a workspace was allocated | no | yes |
| Read hidden partitions | run policy | proposer read scope | evaluator read scope | inherited scope | inherited scope |

Borrow-hostility rule: no context method should force a stage to hold borrowed
graph/surface views across `.await`. Requests passed into async stage calls are
owned and lightweight; rich borrowed views are constructed inside the call and
converted to owned renderings before awaiting external work.

### 8.4 Report types

Context methods return small report structs. The reports carry IDs and graph-backed views — never graph-owned values — because the run graph is the durable truth and reports are read-only summaries of what was just recorded.

```rust
/// Returned by RunContext::propose. The batch was already recorded in the graph;
/// this report exposes IDs and the freshly-built batch for the caller's loop.
pub struct ProposalBatchReport<P: OptimizationProblem> {
    pub batch_id: ProposalBatchId,
    pub batch: ProposalBatch<P>,
    pub cost: Cost,
}

/// Returned by RunContext::apply_batch. Per-proposal apply outcomes.
/// Successful candidates are queryable via successful_candidates();
/// failed proposals' errors are in failed.
pub struct ApplyReport<P: OptimizationProblem> {
    pub batch_id: ProposalBatchId,
    pub outcomes: Vec<ApplyOneReport<P>>,
}

impl<P: OptimizationProblem> ApplyReport<P> {
    pub fn successful_candidates(&self) -> impl Iterator<Item = CandidateId> + '_;
    pub fn failed(&self) -> impl Iterator<Item = (ProposalId, &ErrorRecord)> + '_;
}

pub struct ApplyOneReport<P: OptimizationProblem> {
    pub proposal_id: ProposalId,
    pub outcome: ApplyOutcome<P>,
}

pub enum ApplyOutcome<P: OptimizationProblem> {
    Success {
        candidate: CandidateId,
        identity: ArtifactIdentity,
    },
    Failure {
        error: ErrorRecord,
    },
}

/// Returned by RunContext::evaluate. Assessment IDs (graph-owned) plus a
/// borrowed view for the caller's immediate use.
pub struct EvaluationReport<'a, P: OptimizationProblem> {
    pub request_id: EvaluationRequestId,
    pub resolved_set: ResolvedEvaluationSetId,
    pub assessment_ids: Vec<AssessmentId>,
    pub assessments: Vec<AssessmentView<'a, P>>,
    pub cost: Cost,
    pub cache: CacheStatus,
}
```

The principle: **reports point at the graph, they do not duplicate it.** If a caller wants persistent access to an assessment, they hold the `AssessmentId` and re-query via `ctx.graph().assessment(id)`. The borrowed views in reports are convenience for the immediate loop body.

---

## 9. Async and Dynamic Dispatch Policy

### 9.1 Static-first optimizer

`Optimizer` is static by default:

```rust
Engine<P, O: Optimizer<P>>
```

Optimizers are usually configured values, not registry items.

### 9.2 Dyn-friendly stages

Stages likely to live in registries get object-safe erased traits:

```rust
pub trait DynEvaluator<P: OptimizationProblem>: Send + Sync {
    fn evaluate_boxed<'a>(
        &'a self,
        request: EvaluationRequest,
        ctx: EvaluationContext<'a, P>,
    ) -> BoxFuture<'a, Result<Vec<Assessment<P>>, EvaluationError>>;
}
```

Equivalent wrappers:

```text
DynProposer
DynPreferenceRelation
DynCallback
DynStopper
```

No `DynRenderer` or `DynMaterializer` ships in v0.2.2. Rendering erasure waits
until a real registry use case defines the erased value/target/view contract and
has stage trait contract tests. Do not expose empty marker traits as placeholders.

Adapters exist from static traits to dyn traits.

Core should not require `async_trait`, but an ergonomic adapter crate may use it.

### 9.3 Static traits may use async fn

Static traits may use `async fn` where they are not intended for dyn dispatch. Dyn wrappers use boxed futures.

---

## 10. Run Graph

### 10.1 Graph role

The graph is durable truth. It records what happened. It does not decide what is good.

It records:

```text
candidates
proposal batches
apply attempts
assessments
population events
budget charges
stage errors
cache hits/misses
callbacks/checkpoints
stop events
```

### 10.2 Required graph queries

```rust
impl<P: OptimizationProblem> RunGraphView<'_, P> {
    fn candidate(&self, id: CandidateId) -> Option<CandidateView<'_, P>>;

    fn artifact(&self, id: CandidateId) -> Option<&P::Artifact>;

    fn parents(&self, id: CandidateId) -> Vec<CandidateId>;

    fn children(&self, id: CandidateId) -> Vec<CandidateId>;

    fn lineage(&self, id: CandidateId) -> Lineage<'_, P>;

    fn siblings(&self, id: CandidateId) -> Vec<CandidateId>;

    /// Candidates this proposal read from during reflection.
    /// Distinct from causal parents: these candidates contributed to the proposer's
    /// decision but did not contribute to the new candidate's artifact identity.
    /// Derived from ProposalProvenance::informed_by recorded at proposal time;
    /// not from MetadataBag.
    fn informed_by(&self, id: CandidateId) -> Vec<CandidateId>;

    /// Inverse of informed_by: candidates whose proposers read from `id`.
    fn informed(&self, id: CandidateId) -> Vec<CandidateId>;

    fn proposal_batch(&self, id: ProposalBatchId) -> Option<ProposalBatchView<'_, P>>;

    fn proposal_that_created(&self, id: CandidateId) -> Option<ProposalView<'_, P>>;

    fn assessments(&self, id: CandidateId) -> AssessmentQuery<'_, P>;

    fn assessments_for_target(
        &self,
        id: CandidateId,
        target: AssessmentTarget,
    ) -> AssessmentQuery<'_, P>;

    fn pairwise_assessments(
        &self,
        left: CandidateId,
        right: CandidateId,
    ) -> AssessmentQuery<'_, P>;

    fn population_events(&self, population: PopulationId) -> Vec<PopulationEvent>;

    fn recent_failures(&self, window: Window) -> Vec<FailureRef>;

    fn costs(&self) -> CostSummary;

    fn candidate_tree(&self) -> CandidateTree<'_, P>;
}
```

Strategy authors should navigate by optimizer concepts, not raw storage maps.

---

## 11. Evaluator

```rust
pub trait Evaluator<P: OptimizationProblem>: Send + Sync {
    fn id(&self) -> EvaluatorId;

    fn fingerprint(&self) -> Fingerprint;

    fn cache_policy(
        &self,
        request: &EvaluationRequest,
    ) -> CachePolicy {
        CachePolicy::Never
    }

    fn pair_order_policy(&self) -> PairOrderPolicy {
        PairOrderPolicy::Ordered
    }

    async fn evaluate(
        &self,
        request: EvaluationRequest,
        ctx: EvaluationContext<'_, P>,
    ) -> Result<Metered<Vec<Assessment<P>>>, EvaluationError>;
}
```

Evaluators can be:

```text
deterministic functions
LLM judges
human judges
subprocess runners
agentic sandboxes
compiler/profiler harnesses
pairwise tournament judges
listwise rankers
```

A closure adapter should exist for simple cardinal evaluation.

---

## 12. Proposer

A proposer emits proposal batches. It is a stage used by optimizers such as GEPA, but not required by the engine.

```rust
pub trait Proposer<P: OptimizationProblem>: Send + Sync {
    /// The shape of input this proposer expects.
    /// GEPA reflective mutation, merge, Meta-Harness, ComBE, MIPRO acquisition,
    /// and human editors all need different request shapes — they are not the
    /// same data and should not be smashed into a universal enum.
    ///
    /// Convention: requests are owned and lightweight. They identify *what to do*
    /// (a candidate id, a list of case ids, a `k`), not *what data to use*.
    /// Rich views (HistorySnapshot, lineage walks, evidence aggregations) are
    /// constructed inside `propose` from `ctx.graph()`. This avoids lifetime
    /// gymnastics on the trait while keeping requests type-safe at the call site.
    type Request: Send + Sync;

    fn id(&self) -> ProposerId;
    fn arity(&self) -> Arity;

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError>;
}

/// A hint for what causal-input shape this proposer expects when the optimizer
/// is responsible for candidate selection. NOT a hard law — proposers may emit
/// proposals with different causal shapes than their declared arity (e.g.
/// fail and emit zero proposals; emit alternatives with different parents).
pub enum Arity {
    /// No causal parents required. Proposer authors brand-new artifacts.
    /// Examples: Meta-Harness, MIPRO initial sampling.
    None,

    /// One causal parent. Standard mutation case.
    Single,

    /// Two causal parents. Merge/crossover.
    Pair,

    /// Variable count; proposer decides at request time.
    Variadic,
}
```

Proposers can be:

```text
one-shot LLM calls
multi-stage typed pipelines
agentic workspace processes
merge/crossover algorithms
surrogate acquisition samplers
human editors
ensemble reducers
parallel scan aggregators
```

Use `ctx.propose(&proposer, request)` when possible so stage events and costs are recorded uniformly. The associated `Request` type means the call is fully type-checked: passing a `MergeRequest` to a `ReflectiveMutation` proposer is a compile error.

#### Dyn dispatch for proposers in registries

When proposers must live in a registry (rare; usually proposers are stage fields), the type-erased wrapper hides the request type:

```rust
pub trait DynProposer<P: OptimizationProblem>: Send + Sync {
    fn id(&self) -> ProposerId;
    fn arity(&self) -> Arity;

    /// The erased proposer accepts a type-erased request, downcasting internally.
    fn propose_boxed<'a>(
        &'a self,
        request: Box<dyn Any + Send>,
        ctx: ProposalContext<'a, P>,
    ) -> BoxFuture<'a, Result<Metered<ProposalBatch<P>>, ProposalError>>;
}
```

Static proposers are the default; the dyn wrapper is for runtime-loaded plugins.

---

## 13. Renderers and Materializers

Rendering converts opaque values into consumer-specific values. Materialization
writes opaque values into a workspace for an agent, sandboxed evaluator, or
subprocess-backed tool. Both may be async and costful, but they are
intentionally different operations.

The library splits rendering into two trait families because the side effects differ:

- **`Renderer<P, T, Target>`** returns a value. Used for prompt assembly, JSON blobs, debug HTML, summary strings.
- **`Materializer<P, T>`** populates a workspace by side effect. Used for materializing artifacts, lineage history, traces, and any large structured filesystem layout that an agentic or sandboxed stage will read.

Materializer is not the normal path for a vanilla LLM call. If a stage is just
assembling messages for an LM provider, use `Renderer`. If a stage is preparing
files for an agent runtime, code runner, repo task, or remote sandbox, use
`Materializer`.

Conflating the two was awkward (an `()` view type plus reliance on a `&mut Workspace` smuggled through the context). The split makes both shapes honest.

### 13.1 `Renderer<P, T, Target>` — value-returning

```rust
pub trait Renderer<P: OptimizationProblem, T, Target>: Send + Sync {
    type View;

    async fn render(
        &self,
        value: &T,
        target: Target,
        ctx: RenderContext<'_, P>,
    ) -> Result<Metered<Self::View>, RenderError>;
}
```

Examples:

```text
Artifact     -> ReflectionPrompt        (View = String)
Evidence     -> ReflectionSummary       (View = String)
Lineage      -> PromptContext           (View = StructuredPrompt)
RunGraph     -> HumanDebugHtml          (View = String)
CandidatePair -> PairwiseJudgePrompt    (View = JudgePromptDoc)
```

### 13.2 `Materializer<P, T>` — side-effecting

```rust
pub trait Materializer<P: OptimizationProblem, T>: Send + Sync {
    async fn materialize_into(
        &self,
        value: &T,
        workspace: &mut WorkspaceView<'_>,
        ctx: MaterializeContext<'_, P>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError>;
}

pub struct MaterializationReport {
    pub files_written: usize,
    pub bytes_written: u64,
    pub truncations: Vec<TruncationNote>,
}
```

Examples:

```text
HarnessArtifact          -> writes harness.py into the workspace
ExecutionTrace           -> writes per-case trace files into traces/
LineageDirectorySnapshot -> writes a candidate-per-subdirectory tree
HistorySnapshot          -> the orchestrator that calls the above three
GitWorktreeRendering     -> ensures the worktree is at the parent commit
```

`WorkspaceView<'_>` is a borrowed handle into a workspace subtree, with
`subdir`, `write_file`, `read_file`, `list_files`, executable-bit helpers, and
`run_command`. Paths are
`WorkspacePath`s, not host paths. The same materializer code must work for local
tempdirs, E2B sandboxes, k8s containers, and git worktrees. See §16.6 for
workspace lifecycle.

### 13.3 Choosing between the two

If the consumer wants a value back (string for an LLM prompt, `LmMessages`, JSON
for a typed signature, HTML for a viewer), use `Renderer`. If the consumer needs
a directory tree it can `grep`, `cat`, execute, or mutate (agentic proposer,
sandboxed evaluator, debugger reproducing a run), use `Materializer`. The same
artifact can have both kinds attached for different consumers.

### 13.4 Stage-owned renderers and materializers are the default

Most stages should hold their renderers/materializers as direct fields, not look them up through a registry:

```rust
pub struct ReflectiveMutation<R, L> {
    renderer: R,         // Renderer<P, ParentLineage, ReflectionPrompt>
    lm: L,
}

pub struct AgenticHarnessProposer<HR, AR> {
    history_materializer: HR,       // Materializer<P, HistorySnapshot>
    agent_runtime: AR,
}
```

Stage-owned composition keeps understanding local — the rendering/materializing
used by a particular stage is visible at the type level — and avoids
action-at-a-distance through a global table.

No renderer/materializer registry ships in v0.2.2. Cross-stage shared rendering
and debug viewers should start with explicit typed fields. Add a registry only
after a real user needs runtime rendering choices and the erased
value/target/view contract is covered by tests.

There is no universal `Rendered` enum in core or engine. Text for LM prompts
belongs in `leaven-lm`/`leaven-agent` value types such as `LmMessages`,
`AgentPrompt`, or `PromptDocument`. HTML belongs in debug/viewer targets. JSON
belongs in typed target structs. Add erasure only when a real registry needs a
finite engine-owned set of target/view pairs.

### 13.5 How materialization reaches an agent

The framework does not magically pass a rendered artifact to an agent. The
proposer or evaluator composes the pieces explicitly:

```text
candidate/artifact
  -> materializer writes files into WorkspaceView
  -> renderer builds prompt/messages/config for the agent runtime
  -> AgentRuntime runs commands inside Workspace
  -> stage reads outputs back through Workspace APIs
  -> proposer emits ProposalEffect::Create or Change
```

`AgentRuntime` sees only the prompt/config it is handed and the workspace files
that earlier materializers wrote. It does not receive graph access, trust
policy, evaluation handles, or host filesystem paths.

The full provider-neutral runtime and agentic-stage adapter contract lives in
`docs/specs/agentic_stage_runtime.md`. That document is the implementation
source of truth for `leaven-agent` and `leaven-agentic`.

---

## 14. Preference Relations

```rust
pub trait PreferenceRelation<P: OptimizationProblem>: Send + Sync {
    fn compare(
        &self,
        left: CandidateId,
        right: CandidateId,
        scope: PreferenceScope,
        graph: RunGraphView<'_, P>,
    ) -> Preference;
}
```

Preference relations consume graph evidence and may be:

```text
pure functions over scalar evidence
Pareto relations over score-vector evidence
lexicographic relations
Copeland tournament aggregators (stateless aggregation over recorded judgments)
custom domain relations
```

Preference may be partial. `Incomparable` is a valid result.

**`PreferenceRelation` is stateless.** Stateful/fitted preference models (Bradley-Terry, Plackett-Luce, fitted human-preference aggregators) are owned by `Population` impls instead — typically `TournamentPopulation` — because their state needs to update as new pairwise/listwise observations arrive. See §15 for the population side of this.

---

## 15. Population and Frontier

A population is live optimizer state. A frontier is a kind of population.
Population owns archive membership, admission/update laws, fitted model state,
and strategy events. It does not own the policy for "what should we try next."
That policy is `CandidateSelector` for GEPA-shaped optimizers and analogous
selector traits for other optimizer families.

```rust
pub trait Population<P: OptimizationProblem>: Send {
    fn id(&self) -> PopulationId;

    fn insert_seed(
        &mut self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent>;

    fn observe_candidate(
        &mut self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn observe_assessment(
        &mut self,
        assessment: AssessmentId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn best(
        &self,
        graph: RunGraphView<'_, P>,
    ) -> Option<CandidateId>;

    fn view<'a>(&'a self, graph: RunGraphView<'a, P>) -> PopulationView<'a, P>;
}

pub struct PopulationView<'a, P: OptimizationProblem> {
    pub id: PopulationId,
    pub candidates: &'a [CandidateId],
    pub frontier: FrontierView<'a>,
    pub scores: ScoreView<'a, P>,
    pub niches: Option<NicheView<'a>>,
    pub selection_stats: SelectionStatsView<'a>,
}

pub struct FrontierView<'a> {
    pub members: &'a [CandidateId],
    pub dominated: &'a [CandidateId],
    pub by_case: Option<CaseFrontierView<'a>>,
    pub by_niche: Option<NicheFrontierView<'a>>,
}

pub struct CaseFrontierView<'a> {
    /// One row per `(case, leading candidate)`. Multiple rows for the same case
    /// mean ties or multiple non-dominated leaders.
    pub leaders: &'a [(CaseId, CandidateId)],
}

pub struct NicheFrontierView<'a> {
    /// One row per `(niche, elite candidate)`. Multiple rows for the same niche
    /// mean a pareto set inside that niche.
    pub elites: &'a [(NicheId, CandidateId)],
}

pub struct NicheView<'a> {
    pub assignments: &'a [(CandidateId, NicheId)],
}

pub struct ScoreView<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    population: PopulationId,
}

impl<'a, P: OptimizationProblem> ScoreView<'a, P> {
    pub fn latest_assessments(&self, candidate: CandidateId) -> impl Iterator<Item = AssessmentView<'a, P>> { /* graph query */ }
    pub fn case_outcomes(&self, candidate: CandidateId) -> impl Iterator<Item = (CaseId, CaseOutcome)> { /* graph query */ }
    pub fn aggregate_preference_rank(&self, candidate: CandidateId) -> Option<usize> { /* model/population query */ }
}

pub struct SelectionStatsView<'a> {
    pub attempts: &'a [(CandidateId, u64)],
    pub successes: &'a [(CandidateId, u64)],
    pub last_selected: &'a [(CandidateId, IterationId)],
}

pub struct SelectionContext<'a> {
    pub iteration: IterationId,
    pub rng: &'a mut dyn RngCore,
    pub budget: BudgetSnapshot,
    pub arity: Arity,
}

pub struct CandidateSelection {
    pub candidates: Vec<CandidateId>,
    pub rationale: SelectionRationale,
}

pub struct SelectionOutcome {
    pub selected: CandidateSelection,
    pub proposals: Vec<ProposalId>,
    pub applied: Vec<CandidateId>,
    pub admitted: Vec<CandidateId>,
    pub rejected: Vec<CandidateId>,
}

pub enum SelectionError {
    EmptyPopulation,
    UnsupportedArity { requested: Arity },
    InsufficientCandidates { requested: usize, available: usize },
    Message(String),
}

pub enum SelectionRationale {
    ParetoFrequency { covered_cases: usize },
    GreedyBest,
    Beam { rank: usize },
    Niche { niche: NicheId },
    Exploration,
    UserDefined(String),
}
```

Population events:

```rust
pub enum PopulationEvent {
    Inserted {
        population: PopulationId,
        candidate: CandidateId,
        reason: String,
    },

    Replaced {
        population: PopulationId,
        old: CandidateId,
        new: CandidateId,
        reason: String,
    },

    Removed {
        population: PopulationId,
        candidate: CandidateId,
        reason: String,
    },

    Ignored {
        population: PopulationId,
        candidate: CandidateId,
        reason: String,
    },

    Reweighted {
        population: PopulationId,
        candidate: CandidateId,
        weight: FiniteF64,
        reason: String,
    },

    Migrated {
        from: PopulationId,
        to: PopulationId,
        candidate: CandidateId,
        reason: String,
    },
}
```

Events are strategy opinions. The graph records them but does not treat them as universal truth.

### 15.1 Fitted preference state lives here

Stateful preference models (Bradley-Terry over pairwise judgments, Plackett-Luce over listwise rankings, fitted human-aggregators) are owned by `Population` impls — concretely, `TournamentPopulation` — rather than by `PreferenceRelation`. The reasoning:

- The state of the model depends on the run's accumulated observations.
- Updates fit naturally into `observe_assessment`, but the optimizer calls it explicitly. The engine records assessments into the graph; it does not know which population or fitted model should observe them.
- `best` and `view` expose the fitted model's current opinion without crossing trait boundaries.
- `PreferenceRelation` stays stateless, simple, and `&self`-only.

`TournamentPopulation` should make the fitted model explicit rather than hiding
preference learning behind generic bookkeeping:

```rust
pub trait PreferenceModel<P: OptimizationProblem>: Send {
    fn observe_pairwise(
        &mut self,
        left: CandidateId,
        right: CandidateId,
        judgment: PairwiseJudgment,
    ) -> Vec<ModelEvent>;

    fn compare(
        &self,
        left: CandidateId,
        right: CandidateId,
        scope: PreferenceScope,
        graph: RunGraphView<'_, P>,
    ) -> Preference;

    fn score(&self, candidate: CandidateId) -> Option<FiniteF64>;
}

pub enum ModelEvent {
    FitUpdated { observations: usize },
    ScoreChanged {
        candidate: CandidateId,
        old: Option<FiniteF64>,
        new: FiniteF64,
    },
}
```

```rust
pub struct TournamentPopulation<P: OptimizationProblem, M = BradleyTerryFit> {
    model: M,                         // updated in observe_assessment
    candidates: BTreeSet<CandidateId>,
    config: TournamentConfig,
}

impl<P, M> Population<P> for TournamentPopulation<P, M>
where
    P::Evidence: PairwiseEvidence,
    M: PreferenceModel<P>,
{
    fn observe_assessment(
        &mut self,
        assessment: AssessmentId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        let a = graph.assessment(assessment);
        if let Assessment::Pairwise { left, right, evidence, .. } = a {
            self.model.observe_pairwise(left, right, evidence.judgment());
        }
        // …
    }

    fn best(&self, _graph: RunGraphView<'_, P>) -> Option<CandidateId> {
        self.candidates
            .iter()
            .copied()
            .max_by(|a, b| self.model.score(*a).cmp(&self.model.score(*b)))
    }

    // …
}
```

### 15.2 ParetoFrontier and partition filtering

The standard `ParetoFrontier` population is built via a builder. Frontiers can declaratively ignore observations from specific case-set partitions. This is necessary for clean benchmark mode (only update from `SEARCH`, never from `TEST`) and for probe-eval handling.

```rust
let frontier = ParetoFrontier::<P, _>::builder()
    .axis_extracted("accuracy",       Direction::HigherIsBetter,
                    |e: &P::Evidence| e.accuracy())
    .axis_extracted("context_tokens", Direction::LowerIsBetter,
                    |e: &P::Evidence| e.context_tokens() as f64)
    .partition_filter(|target| matches!(target,
        AssessmentTarget::EvaluationSet(id) if is_search_partition(id)))
    .build();
```

Test-set assessments are still observed by the engine and recorded in the graph, but the frontier ignores them when deciding admission. Final test-set evaluation reads frontier members from outside the optimizer loop.

---

## 16. Trust and Capability Boundaries

Agentic stages require explicit boundaries.

### 16.1 Trust policy

`TrustPolicy` is run-level configuration that derives per-stage read scopes and
probe permissions. It is not generic over `P` unless an implementation adds
problem-specific policy predicates; the standard policy is partition-oriented.

```rust
pub struct TrustPolicy {
    hidden_from_proposers: BTreeSet<PartitionId>,
    probe_policy: ProbePolicy,
    evidence_visibility: EvidenceVisibility,
}

pub enum ProbePolicy {
    Disabled,
    SearchPartitionsOnly,
    Explicit(BTreeSet<EvaluationSetId>),
}

impl TrustPolicy {
    pub fn hide_from_proposer(
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> Self;

    pub fn read_scope_for_proposer(&self, proposer: ProposerId) -> ReadScope;
    pub fn read_scope_for_evaluator(&self, evaluator: EvaluatorId) -> ReadScope;
    pub fn probe_permission(&self, proposer: ProposerId) -> EvaluationSetPermission;
}
```

The examples use `TrustPolicy::hide_from_proposer([PartitionId::TEST])` as
constructor sugar. Runs with multiple proposers still start from one run-level
policy; if a future prototype needs proposer-specific hiding, add keyed entries
inside `TrustPolicy` rather than making every stage own an unrelated policy.

### 16.1.1 Actor capability table

Trust is enforced at the context and rendering/materialization boundary. The
engine owns graph truth; stages receive scoped views and explicit handles.

| Actor | Receives | May do | Must not do |
|---|---|---|---|
| Optimizer | `RunContext` and run-scope graph views | Apply proposals, request configured evaluations, update populations, emit events | Reach around `RunContext` to mutate graph internals |
| Candidate selector | `PopulationView`, scoped graph view, selection context | Choose candidate IDs or parent sets for the optimizer's next step | Read hidden case content or perform side-effectful work |
| Proposer | `ProposalContext`, optional `EvalHandle`, optional workspace factory | Read allowed graph/evidence renderings, allocate workspace if granted, request probe evals if granted | See hidden validation/test content or mutate graph directly |
| Evaluator | `EvaluationContext`, resolved request, requested artifacts/cases | Run assessments, allocate workspace if granted, write evidence | Request nested normal evaluations or update populations |
| Renderer | `RenderContext` | Return value views from visible graph/artifact/evidence data, including LM-facing prompt/messages | Touch workspace files or assume a materialized layout |
| Materializer | `MaterializeContext` plus a `WorkspaceView` subtree | Write visible data into the provided workspace subtree for an agent/sandbox consumer | Allocate/cleanup workspaces, assume `local_mount()`, or write hidden partitions |
| Agent runtime | Agent prompt/config plus `Workspace`/`WorkspaceView` | Run commands, read/write workspace files according to runtime policy | Receive graph handles, trust policy, evaluation handles, or host paths |
| Callback | Event payload filtered by callback visibility | Observe/report events | Mutate graph or request evaluations |

GEPA clean benchmark law: the reflective proposer may see feedback/minibatch
case content and trace summaries allowed by its proposer read scope, but it must
not see validation/test case content or traces. Candidate selection may use
validation scores if the run policy exposes them, because those scores are
selection signal rather than reflective learning text.

### 16.2 Proposal context

```rust
pub struct ProposalContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    budget: BudgetHandle<'a>,                     // unified budget access

    readable: ReadScope<P>,
    workspace: Option<&'a dyn WorkspaceFactory>,
    eval: Option<EvalHandle<'a, P>>,
}
```

### 16.3 Eval handle

```rust
pub struct EvalHandle<'a, P: OptimizationProblem> {
    allowed_sets: EvaluationSetPermission,
    evidence_visibility: EvidenceVisibility,
    budget: BudgetHandle<'a>,                     // same type as ProposalContext
    recorder: ProbeRecorder<'a, P>,
}
```

### 16.4 Render and materialize contexts

`RenderContext` and `MaterializeContext` are read-only stage contexts. They
inherit the caller's `ReadScope`; they do not allocate workspaces or mutate the
graph.

```rust
pub struct RenderContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    readable: ReadScope<P>,
    budget: BudgetHandle<'a>,
}

pub struct MaterializeContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    readable: ReadScope<P>,
    budget: BudgetHandle<'a>,
    root: WorkspacePath,
}

impl<'a, P: OptimizationProblem> MaterializeContext<'a, P> {
    pub fn render_context(&mut self) -> RenderContext<'_, P>;
    pub fn read_scope(&self) -> &ReadScope<P>;
    pub fn root(&self) -> &WorkspacePath;
}
```

Materializers receive the workspace subtree separately as `&mut WorkspaceView`.
The `root` in `MaterializeContext` is descriptive/accounting context, not a host
path.

### 16.5 Budget handle

The single type stages use to charge cost. Wraps the engine's `BudgetLedger` along with the stage tag, so charges are attributed automatically.

```rust
pub struct BudgetHandle<'a> {
    ledger: &'a mut BudgetLedger,
    stage: StageId,
}

impl<'a> BudgetHandle<'a> {
    pub fn snapshot(&self) -> BudgetSnapshot;
    pub fn remaining(&self, unit: CostUnit) -> Option<Amount>;

    /// Charge cost to the ledger under this handle's stage tag.
    pub fn charge(&mut self, cost: Cost) -> Result<(), BudgetExceeded>;

    /// Sub-stages (e.g. an evaluator's per-case subprocess invocation) get
    /// a re-borrowed handle with a more specific stage tag. The lifetime
    /// nests; only one mutable borrow exists at a time.
    pub fn sub_stage(&mut self, sub: StageId) -> BudgetHandle<'_>;
}
```

The point of the type is borrow safety: there is exactly one path from "I want to charge cost" to "the ledger gets mutated," and it's parameterized by a stage tag for free attribution. Stages never see `&mut BudgetLedger` directly.

`ctx.budget_handle()` returns `BudgetHandle` borrowed from the context's ledger. Callers can pass it into agent runtimes, evaluators, renderers, etc., and the borrow checker prevents two mutable accesses crossing.

Clean benchmark mode:

```text
proposer cannot read validation/test content
proposer cannot request validation/test probe evals
proposer sees only allowed evidence renderings
```

Exploratory mode:

```text
proposer may request probe evaluations
every probe is graph-recorded
probe candidates/assessments are tagged as probe-originated
population eligibility is controlled by policy
```

### 16.6 Workspace lifecycle

Agentic stages need a place to read and write files, possibly inside a sandbox. The library models this with three concepts that compose:

```
[engine]                  owns the WorkspaceFactory (chosen at config time)
   │
   ▼
[WorkspaceFactory]        creates Workspace handles on demand
   │                      (Local, E2B, Docker, K8s, Firecracker, GitWorktree, …)
   ▼
[Workspace]               a typed handle. filesystem ops + run-command.
   │                      backed by ONE sandbox of whatever flavor the factory makes.
   ├──▶ used by [Materializer]   (writes files into it)
   └──▶ used by [AgentRuntime]        (runs commands in it)
```

`Workspace` is a Leaven-owned lease handle, not a trait users implement. Users
choose or implement a `WorkspaceFactory`/`WorkspaceBackend`; stages receive the
concrete `Workspace` handle and use the same API for local and remote backends.

The workspace is the unit. One stage call (one `propose`, one `evaluate`) often
gets one workspace. It lives for the duration of that call unless a custom stage
deliberately carries it across helper boundaries. Explicit cleanup is the
primary teardown path; `Drop` only marks abandoned resources for best-effort
janitors.

Workspace paths are always `WorkspacePath`s: normalized, relative to the
workspace root, UTF-8, and rejecting `..`, absolute paths, drive prefixes, and
empty components. A `WorkspacePath` is not a host path. `local_mount()` is an
optional optimization for local-style backends, never a correctness dependency.

#### 16.6.1 Trait surface

```rust
#[async_trait]
pub trait WorkspaceFactory: Send + Sync {
    async fn allocate(&self, cfg: WorkspaceConfig) -> Result<Workspace, FactoryError>;
}

pub struct Workspace {
    inner: Option<Box<dyn WorkspaceBackend>>,    // None after cleanup() consumes
}

pub struct WorkspacePath { /* normalized relative path */ }

impl WorkspacePath {
    pub fn new(path: impl AsRef<str>) -> Result<Self, WorkspacePathError>;
    pub fn join(&self, child: impl AsRef<str>) -> Result<Self, WorkspacePathError>;
    pub fn as_str(&self) -> &str;
}

impl Workspace {
    pub fn view(&mut self) -> WorkspaceView<'_>;

    pub async fn write_file(
        &mut self,
        path: WorkspacePath,
        bytes: impl Into<bytes::Bytes>,
    ) -> Result<(), WorkspaceError>;

    pub async fn read_file(
        &mut self,
        path: WorkspacePath,
    ) -> Result<bytes::Bytes, WorkspaceError>;

    pub async fn list_files(
        &mut self,
        path: WorkspacePath,
    ) -> Result<Vec<WorkspacePath>, WorkspaceError>;

    pub async fn set_executable(
        &mut self,
        path: WorkspacePath,
        executable: bool,
    ) -> Result<(), WorkspaceError>;

    pub async fn is_executable(
        &mut self,
        path: WorkspacePath,
    ) -> Result<bool, WorkspaceError>;

    pub async fn run_command(
        &mut self,
        command: Command,
    ) -> Result<CommandOutput, WorkspaceError>;

    pub fn local_mount(&self) -> Option<&Path>;

    /// Explicit, awaited cleanup. Always preferred over relying on Drop.
    /// After this returns, the Workspace is consumed and its backend is gone.
    pub async fn cleanup(mut self) -> Result<(), WorkspaceError> {
        if let Some(backend) = self.inner.take() {
            backend.cleanup().await
        } else {
            Ok(())
        }
    }
}

pub async fn with_workspace<T, E, F>(
    factory: &dyn WorkspaceFactory,
    cfg: WorkspaceConfig,
    f: F,
) -> Result<T, WorkspaceRunError<E>>
where
    F: for<'w> FnOnce(&'w mut Workspace) -> BoxFuture<'w, Result<T, E>>,
{
    let mut ws = factory.allocate(cfg).await.map_err(WorkspaceRunError::Allocate)?;
    let result = f(&mut ws).await;
    let cleanup = ws.cleanup().await;

    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(err), Ok(())) => Err(WorkspaceRunError::Stage(err)),
        (Ok(_), Err(err)) => Err(WorkspaceRunError::Cleanup { source: err }),
        (Err(stage), Err(cleanup)) => Err(WorkspaceRunError::StageAndCleanup {
            stage,
            cleanup,
        }),
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        // Best-effort: synchronous local cleanup only. Cannot await async work
        // here, so remote sandbox destruction must go through cleanup().
        // If the backend was not consumed by an explicit cleanup() call, this
        // marks the workspace as "abandoned" — the factory's janitor (if any)
        // will reap it later.
        if let Some(backend) = self.inner.take() {
            backend.mark_abandoned();
        }
    }
}

#[async_trait]
pub trait WorkspaceBackend: Send + Sync {
    async fn write_file(
        &mut self,
        path: WorkspacePath,
        bytes: bytes::Bytes,
    ) -> Result<(), WorkspaceError>;

    async fn read_file(
        &mut self,
        path: WorkspacePath,
    ) -> Result<bytes::Bytes, WorkspaceError>;

    async fn list_files(&mut self, path: WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError>;
    async fn set_executable(&mut self, path: WorkspacePath, executable: bool) -> Result<(), WorkspaceError>;
    async fn is_executable(&mut self, path: WorkspacePath) -> Result<bool, WorkspaceError>;

    async fn run_command(&mut self, cmd: Command) -> Result<CommandOutput, WorkspaceError>;

    /// Async cleanup. Called by Workspace::cleanup. Implementors should fully
    /// release backend resources (destroy E2B sandbox, delete K8s container,
    /// remove git worktree) here.
    async fn cleanup(self: Box<Self>) -> Result<(), WorkspaceError>;

    /// Synchronous best-effort marking. Called by Workspace::Drop when the
    /// caller did not invoke cleanup(). Implementors should leave a marker
    /// for a factory-owned janitor to find later. Default: no-op (not all
    /// backends have a janitor; abandoned local tempdirs are usually fine).
    fn mark_abandoned(self: Box<Self>) {}

    /// For materializers that need a real local path (e.g. mounting into a subprocess
    /// running on the host). `None` for pure-remote backends like E2B without a
    /// local sync. Materializers should not depend on this returning `Some`.
    fn local_mount(&self) -> Option<&Path> { None }
}
```

**Always call `Workspace::cleanup().await` explicitly.** `Drop` is a safety net, not a primary cleanup path:

- Async work cannot be awaited inside `Drop`, so remote sandbox teardown (E2B, K8s, Firecracker) cannot happen there.
- A factory may run a periodic janitor that reaps abandoned workspaces — useful for crashes mid-run, but not a substitute for explicit cleanup.
- Stages with workspaces must call `cleanup()` on their workspace before returning, including in error paths. Prefer `with_workspace`; use bare `allocate()` only when the workspace lifetime genuinely crosses helper boundaries.

```rust
// idiomatic stage code with cleanup
async fn evaluate(&self, …) -> Result<…> {
    with_workspace(&*self.workspace_factory, WorkspaceConfig::default(), |ws| Box::pin(async move {
        self.run_evaluation(ws).await
    })).await.map_err(EvaluationError::from)
}
```

#### 16.6.2 Local and E2B semantics

Local and remote workspaces expose the same Leaven API.

Local backend shape:

```text
WorkspacePath("skills/foo.md") -> tempdir/skills/foo.md
write_file                    -> tokio::fs::write
read_file                     -> tokio::fs::read
run_command                   -> local child process with cwd at the workspace root
local_mount()                 -> Some(tempdir.path())
cleanup()                     -> remove tempdir / git worktree
```

E2B-style backend shape:

```text
WorkspacePath("skills/foo.md") -> /workspace/leaven/<run>/skills/foo.md inside sandbox
write_file                    -> sandbox.files().write(...)
read_file                     -> sandbox.files().read_bytes/read_string(...)
run_command                   -> sandbox.commands().run(...) with cwd at workspace root
local_mount()                 -> None
cleanup()                     -> sandbox.kill() or release to pool
```

The `Materializer` and `AgentRuntime` do not branch on backend type. They use
`WorkspacePath`, `write_file`, `read_file`, and `run_command`. Backend crates
translate those operations into local filesystem calls, E2B files/commands,
Docker exec/copy calls, k8s exec/copy calls, or git-worktree operations.

#### 16.6.3 Standard backends

The library ships a small set of reference backends. Most users will configure one of these or write their own:

```text
LocalWorkspaceFactory            tempdir on the host. cheap, no isolation. dev/test.
E2BWorkspaceFactory              one e2b sandbox per workspace. pooling supported.
DockerWorkspaceFactory           one docker container per workspace. local isolation.
K8sWorkspaceFactory              container-in-pod. pod is shared, container is per-workspace.
FirecrackerWorkspaceFactory      one microvm per workspace. strong isolation, slower spin-up.
GitWorktreeFactory<Inner>        wraps another factory; allocates a worktree at a parent commit.
```

`GitWorktreeFactory` is a composition: it takes any other factory as its inner sandbox and adds git-worktree semantics on top. The agent commits inside the worktree; the framework reads `HEAD` on cleanup; cleanup removes the worktree directory but leaves commit objects in the main repo. A git-backed artifact usually records the commit hash as `ArtifactIdentity::External` and may expose it as `CacheIdentity::ExternalContent`.

#### 16.6.4 Ownership table

| Thing | Lifetime | Owner |
|---|---|---|
| `WorkspaceFactory` | Full run | Engine (configured at startup) |
| `Workspace` handle | One stage call | The stage that called `allocate` |
| Underlying sandbox/container/VM | Workspace handle's lifetime | `WorkspaceBackend` impl |
| Pooled warm sandboxes | Process-lifetime; idle-evicted | The factory |
| `AgentRuntime` instance | User-defined | The proposer (or a registry) |
| Files inside the workspace | Workspace handle | Wiped on cleanup |

#### 16.6.5 What the framework does NOT manage

- **Backend choice.** The factory is yours. The library does not assume e2b, docker, or any specific sandbox.
- **Agent processes.** That is the `AgentRuntime`'s job. The runtime uses the workspace as a substrate via `run_command`.
- **Pooling.** A factory may pool internally (recommended for slow cold-starts like e2b or firecracker). The framework does not pool by default.
- **Isolation guarantees.** Trust comes from the factory choice. A `LocalWorkspaceFactory` gives you no isolation; a `FirecrackerWorkspaceFactory` gives you strong isolation. Pick deliberately.

---

## 17. Cache

### 17.1 Engine-owned

The engine owns evaluation caching.

### 17.2 Cache key

```rust
pub struct EvaluationCacheKey {
    pub evaluator_fingerprint: Fingerprint,
    pub request_fingerprint: Fingerprint,
    pub candidate_cache_identities: Vec<CacheIdentity>,
    pub evaluation_set_id: EvaluationSetId,
    pub case_set_version: CaseSetVersion,
    pub seed: Option<u64>,
}
```

For pairwise requests, ordering is preserved unless evaluator declares unordered symmetry.

`EvaluationCacheKey` uses `CacheIdentity`, not `ArtifactIdentity`. External
artifact identity is allowed in the graph, but it is not cache-safe unless the
artifact provides `CacheIdentity::ExternalContent` or the evaluator policy
provides `CacheIdentity::User`.

### 17.3 Cache policy

```rust
pub enum CachePolicy {
    Never,
    Deterministic,
    DeterministicWithSeed(u64),
    UserKey(Fingerprint),
}
```

Default is `Never`. Nondeterministic LLM/agent evaluators should not be cached accidentally.

`Deterministic` and `DeterministicWithSeed` require every candidate in the
resolved request to provide `Some(CacheIdentity)`. If any candidate returns
`None`, the cache is bypassed with an explicit `CacheStatus::Bypassed` reason;
the engine still records the evaluation normally. `UserKey` appends the supplied
fingerprint to the request/evaluator/case-set identity and is the escape hatch
for deterministic external systems whose immutable state is known to the user
but not visible to the artifact type.

---

## 18. Callbacks and Events

Callbacks are first-class.

```rust
pub trait Callback<P: OptimizationProblem>: Send {
    fn on_event(
        &mut self,
        event: &RunEvent<P>,
        graph: RunGraphView<'_, P>,
    );
}
```

Core events:

```rust
pub enum RunEvent<P: OptimizationProblem> {
    OptimizationStarted { run_id: RunId },

    OptimizationStopping { reason: StopReason },

    OptimizationEnded {
        run_id: RunId,
        best: Option<CandidateId>,
        budget: BudgetSnapshot,
    },

    IterationStarted { iteration: IterationId },

    IterationEnded {
        iteration: IterationId,
        status: StepStatus,
    },

    ProposalBatchProduced {
        iteration: IterationId,
        batch_id: ProposalBatchId,
        proposer: StageId,
        proposal_count: usize,
    },

    /// Per-proposal record. Effect kind and provenance summary live here;
    /// full causal_inputs / informed_by are queryable via graph.proposal_batch().
    ProposalRecorded {
        proposal_id: ProposalId,
        batch_id: ProposalBatchId,
        effect: ProposalEffectKind,         // Create or Change
        causal_inputs: CausalInputsSummary, // (variant + count)
        informed_by_count: usize,
    },

    ApplySucceeded {
        proposal_id: ProposalId,
        candidate_id: CandidateId,
        identity: ArtifactIdentity,
    },

    ApplyFailed {
        proposal_id: ProposalId,
        error: ErrorRecord,
    },

    EvaluationRequested {
        request_id: EvaluationRequestId,
        evaluator: EvaluatorId,
        request: EvaluationRequestSummary,
    },

    EvaluationCompleted {
        request_id: EvaluationRequestId,
        evaluator: EvaluatorId,
        assessment_ids: Vec<AssessmentId>,
        cost: Cost,
        cache: CacheStatus,
    },

    RenderCompleted {
        renderer: RendererId,
        target: String,
        cost: Cost,
    },

    MaterializationCompleted {
        materializer: MaterializerId,
        root: WorkspacePath,
        files_written: usize,
        cost: Cost,
    },

    PopulationUpdated {
        population_id: PopulationId,
        events: Vec<PopulationEvent>,
    },

    BudgetCharged {
        stage: StageId,
        cost: Cost,
        remaining: BudgetSnapshot,
    },

    CheckpointSaved {
        checkpoint: CheckpointId,
    },

    Error {
        stage: Option<StageId>,
        error: ErrorRecord,
        policy: ErrorPolicy,
    },
}
```

Engine emits lifecycle events. Context methods emit operation events.

---

## 19. Persistence and Evidence Storage

Core should not require:

```rust
Evidence: Serialize + DeserializeOwned
```

Instead:

```rust
pub trait EvidenceStore<E: Evidence>: Send + Sync {
    fn put(&self, evidence: E) -> Result<EvidenceRef, StoreError>;
    fn get(&self, reference: EvidenceRef) -> Result<E, StoreError>;
}
```

Default stores:

```text
InlineSerdeStore<E: Serialize + DeserializeOwned>
FileEvidenceStore
ObjectEvidenceStore
SqliteEvidenceStore
```

This avoids forcing giant agent traces into inline run graph serialization.

---

## 20. GEPA as an Optimizer

GEPA is one optimizer value.

```rust
pub struct Gepa<P, S, Pop = Box<dyn Population<P>>>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    surface: Arc<S>,
    population: Pop,
    proposer: Box<dyn GepaProposer<P, S>>,
    candidate_selector: Box<dyn CandidateSelector<P>>,
    part_selector: Box<dyn PartSelector<P, S>>,
    batch_sampler: Box<dyn BatchSampler<P>>,
    acceptance: Box<dyn Acceptance<P>>,
    validation: Box<dyn ValidationPolicy<P>>,
    merge: Option<Box<dyn GepaMerge<P, S>>>,
}
```

`S` stays static because `S::PartId`, `S::Edit`, and `S::fingerprint()` flow
through part selection, trace attribution, and proposal lowering. `Pop` is left
as a generic with a boxed default because fitted populations may want concrete
state and non-object-safe views during early implementation. Other GEPA policy
slots are boxed trait objects by default. Do not introduce parallel `Dyn*`
marker traits for slots that are already object-safe.

GEPA components:

```text
CandidateSelector selects candidate(s) from a PopulationView.
PartSelector selects surface part(s) to mutate.
BatchSampler selects train/minibatch cases.
Proposer emits surface edits or artifact-native proposals.
Acceptance decides whether a child gets validation/admission.
ValidationPolicy decides validation request.
Population maintains Pareto/frontier/live set.
MergeScheduler decides when to call merge proposer.
```

Population and candidate selection stay separate:

```text
Population        = what exists, how observations update it, what gets admitted/replaced
CandidateSelector = what parent to mutate next from the current population/archive view
Acceptance     = whether a freshly proposed child deserves follow-up validation
```

This separation is load-bearing. GEPA's paper baseline, frequency-weighted
instance Pareto sampling, greedy best-first, beam search, MAP-Elites-style
archive sampling, island migration, and skill-library hard-case loops all need
different selection policies over similar archive state.

```rust
pub trait CandidateSelector<P: OptimizationProblem>: Send {
    fn select(
        &mut self,
        population: PopulationView<'_, P>,
        graph: RunGraphView<'_, P>,
        ctx: SelectionContext<'_>,
    ) -> Result<CandidateSelection, SelectionError>;

    fn observe_selection_outcome(&mut self, _outcome: &SelectionOutcome) {}
}
```

GEPA proposers have a GEPA-specific request shape because the selected surface
part is part of the contract:

```rust
pub struct GepaMutationRequest<S: EditSurface<A>, A: Artifact> {
    pub parent: CandidateId,
    pub part: S::PartId,
    pub feedback_assessments: Vec<AssessmentId>,
    pub proposal_count: usize,
}

pub enum GepaProposal<P: OptimizationProblem, S: EditSurface<P::Artifact>> {
    SurfaceEdit {
        target: CandidateId,
        edit: SurfaceEdit<S, P::Artifact>,
        annotations: P::ProposalAnnotations,
        informed_by: Vec<InfoRef>,
    },
    Native(Proposal<P>),
}

pub trait GepaProposer<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    fn propose<'a>(
        &'a self,
        request: GepaMutationRequest<S, P::Artifact>,
        ctx: ProposalContext<'a, P>,
    ) -> BoxFuture<'a, Result<Metered<Vec<GepaProposal<P, S>>>, ProposalError>>;
}

pub trait GepaMerge<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    fn merge<'a>(
        &'a self,
        left: CandidateId,
        right: CandidateId,
        ctx: ProposalContext<'a, P>,
    ) -> BoxFuture<'a, Result<Metered<Vec<GepaProposal<P, S>>>, ProposalError>>;
}
```

`CandidateSelector::select` is synchronous and side-effect-light. It receives
borrowed graph/population views and must not await. If a "selector" needs LLM
calls, subprocesses, or remote state, model it as an optimizer step or proposer
substage and feed the result into a normal selector/admission policy.

Standard GEPA selectors:

```text
ParetoFrequencyWeighted   paper-style instance-pareto frequency sampling
SelectBestCandidate          greedy ablation / TextGrad-like baseline
BeamCandidateSelector        top-k beam-style candidate choice
UniformFrontier           exploration over current frontier members
NicheWeighted             MAP-Elites/quality-diversity archive sampling
RoundRobinCandidate       deterministic reproduction/debug selector
```

Mapping to GEPA paper:

| GEPA algorithm concept | Library concept |
|---|---|
| candidate pool `P` | `Population` |
| Pareto front by instance | `ParetoFrontier::by_case()` |
| SELECTCANDIDATE | `CandidateSelector` |
| SELECTMODULE | `PartSelector<P, S>` over `EditSurface<P::Artifact>` |
| minibatch from `D_feedback` | `BatchSampler` + `EvaluationSet::Partition(TRAIN)` |
| per-instance score table | `CasewiseEvidence` + `AssessmentGranularity::PerCase` |
| reflective prompt update | `Proposer` |
| score improves on minibatch | `Acceptance` |
| evaluate on `D_pareto` | `ValidationPolicy` |
| add to pool | `Population::observe_candidate` |
| merge/crossover | another `Proposer` scheduled by GEPA |

Canonical GEPA step skeleton:

```text
1. view = population.view(ctx.graph())
2. parent = candidate_selector.select(view, ctx.graph(), selection_ctx)?
3. part = part_selector.select(parent, ctx.graph(), surface)?
4. batch = batch_sampler.sample(...)
5. run parent on minibatch and gather casewise/attribution evidence
6. proposer proposes one or more surface edits for selected part
7. GEPA lowers surface edits through S into artifact-native changes
8. apply proposals through RunContext
9. gate decides which children receive validation
10. validation policy chooses validation request
11. optimizer calls population.observe_candidate/observe_assessment
12. selector observes outcome for its own lightweight stats
```

The paper baseline uses round-robin `SELECTMODULE`; Leaven ships that as
`RoundRobinPart` for reproduction. Leaven may also ship trace-aware selectors
such as `InvokedAndFailingPart`:

```rust
pub struct InvokedAndFailingPart {
    /// Exploration weight for parts that have no current failure attribution.
    pub unexplained_part_weight: FiniteF64,
}

impl<P, S> PartSelector<P, S> for InvokedAndFailingPart
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
    P::Evidence: AttributableEvidence<S::PartId>,
{
    // sample from failure-attributed parts, with an exploration floor for
    // never-invoked or never-attributed parts.
}
```

`RoundRobinPart` is the paper algorithm. `InvokedAndFailingPart` is the
recommended default only when the evaluator has parsed traces/evaluation
feedback into `AttributableEvidence<S::PartId>` under the same
`SurfaceFingerprint`.

#### Surface edit lowering

Generic GEPA proposers should not need to know every artifact's native
`Change` type. A GEPA proposer may return a surface edit:

```rust
pub struct SurfaceEdit<S: EditSurface<A>, A: Artifact> {
    pub part: S::PartId,
    pub edit: S::Edit,
}
```

GEPA lowers `SurfaceEdit` through `surface.change_part(artifact, part, edit)`
to produce `<P::Artifact as Artifact>::Change`, then records
`ProposalEffect::Change { target, change }`. Artifact-native proposals are still
allowed for specialized proposers, but the generic path is surface-edit first.

#### Merge canonicalization in GEPA

GEPA's merge picks per-part "best" content from two candidates `(a, b)`.
`Artifact::apply_change` only sees one artifact, so the merge proposer
canonicalizes: it picks one parent (say `a`) as the apply target, reads `b`
through the same surface to extract the parts it wants to import, and constructs
surface edits that lower to a native `Change` against `a`. The resulting
`Proposal` has `effect: ProposalEffect::Change { target: a, change }` and
`provenance.causal: CausalInputs::Pair(a, b)` so lineage queries see both
contributors, but the apply step is single-parent. The constructor sugar
`Proposal::merge(a, b, change)` packages artifact-native merge changes.

GEPA customization:

```rust
let gepa = Gepa::default()
    .surface(SkillDirByFrontmatterId::default())
    .proposer(ReflectiveMutation::new(lm).n_alternatives(3))
    .population(ParetoFrontier::by_case().frequency_weighted())
    .part_selector(RoundRobinPart)
    .batch_sampler(EpochShuffled::new(4))
    .acceptance(StrictImprovement)
    .validation(FullValidation)
    .merge(SystemAwareMerge::adaptive());
```

---

## 21. MuF/Edit-Style Typed Claims

MuF/Edit fits as typed annotations.

```rust
pub struct EditAnnotations {
    pub diagnosis: MuFOutput,
    pub rationale: String,
    pub rhetorical_strategy: String,
    pub should_fix: String,
    pub should_not_break: String,
    pub rollback_note: String,
    pub confidence: Confidence,
}
```

Capability trait:

```rust
pub trait HasBehavioralClaims {
    fn should_fix(&self) -> &str;
    fn should_not_break(&self) -> &str;
    fn confidence(&self) -> Confidence;
}
```

Acceptance:

```rust
pub struct ClaimsHeldAcceptance<J> {
    judge: J,
}

impl<P, J> Acceptance<P> for ClaimsHeldAcceptance<J>
where
    P: OptimizationProblem<ProposalAnnotations = EditAnnotations>,
    J: ClaimJudge<P>,
{
    fn decide(
        &self,
        candidate: CandidateId,
        parent: CandidateId,
        scope: PreferenceScope,
        graph: RunGraphView<'_, P>,
    ) -> AcceptanceDecision {
        let proposal = graph.proposal_that_created(candidate);
        let claims = &proposal.annotations;

        if self.judge.claims_held(parent, candidate, claims, scope, graph) {
            AcceptanceDecision::Promote
        } else {
            AcceptanceDecision::RecordOnly
        }
    }
}
```

MuF/Edit is natural but not core-shaped. `should_fix` does not become a universal primitive.

---

## 22. User-Facing API Tiers

### 22.1 Tier 1: simple use

```rust
let result = optimize(seed_prompt)
    .train(train_cases)
    .score(|ctx| async move {
        // returns scalar evidence through an adapter
    })
    .using(Gepa::default().with_reflection_lm(lm))
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

### 22.2 Tier 2: GEPA customization

```rust
let result = optimize(seed_agent)
    .train(repo_tasks)
    .validation(heldout_repo_tasks)
    .evaluator(RepoAgentEvaluator::new(sandbox))
    .using(
        Gepa::default()
            .surface(RepoPathSurface::default())
            .proposer(AgenticProposer::new(runtime))
            .candidate_selector(ParetoFrequencyWeighted)
            .part_selector(InvokedAndFailingPart::default())
            .population(ParetoFrontier::by_case_and_axis())
            .batch_sampler(EpochShuffled::new(4))
    )
    .budget(Budget::usd(100.0))
    .run()
    .await?;
```

### 22.3 Tier 3: optimizer author

```rust
struct MyTournamentOptimizer {
    /// Owns the fitted Bradley-Terry model internally.
    population: TournamentPopulation<MyProblem>,
    selector: ThompsonPairSelector,
}

impl Optimizer<MyProblem> for MyTournamentOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, MyProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let pair = self.selector.select_pair(
            self.population.view(ctx.graph().view()),
            ctx.graph().view(),
            ctx.selection_context(Arity::Pair),
        )?;

        let report = ctx.evaluate_with(
            EvaluatorId::PAIRWISE_JUDGE,
            EvaluationRequest::Pairwise {
                left: pair[0],
                right: pair[1],
                set: EvaluationSet::Partition(PartitionId::TRAIN),
                granularity: AssessmentGranularity::Aggregate,
                purpose: EvaluationPurpose::Selection,
                order: PairOrder::Ordered,
            },
        ).await?;

        // observe_assessment updates the population's fitted Bradley-Terry model
        for assessment in report.assessments {
            let events = self.population.observe_assessment(assessment.id, ctx.graph().view());
            ctx.record_population_events(self.population.id(), events);
        }

        Ok(StepStatus::Continue)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, MyProblem>,
    ) -> Option<CandidateId> {
        self.population.best(graph)
    }
}
```

### 22.4 Worked example: pairwise tournament — non-GEPA optimizer

This optimizer learns from pairwise judgments. It has no part selector, no GEPA
minibatch gate, and no validation rhythm. It uses the same engine substrate:
propose, apply, evaluate, observe population state.

```rust
pub struct PairwisePromptProblem;

impl OptimizationProblem for PairwisePromptProblem {
    type Artifact = PromptProgram;
    type Case = QaCase;
    type Evidence = PairwiseJudgeEvidence;
    type ProposalAnnotations = EditNotes;
}

#[derive(Clone)]
pub struct PromptProgram {
    modules: BTreeMap<ModuleId, Arc<str>>,
    identity: ArtifactIdentity,
}

impl Artifact for PromptProgram {
    type Change = PromptProgramEdit;
    type ApplyError = PromptProgramError;

    fn identity(&self) -> ArtifactIdentity { self.identity.clone() }
    fn apply_change(&self, edit: &PromptProgramEdit) -> Result<Self, Self::ApplyError> {
        /* apply one prompt edit and re-identify */
    }
}

pub struct PairwiseJudgeEvidence {
    judgment: PairwiseJudgment,
    confidence: FiniteF64,
    rationale_ref: Option<EvidenceRef>,
}

impl Evidence for PairwiseJudgeEvidence {}

impl PairwiseEvidence for PairwiseJudgeEvidence {
    fn judgment(&self) -> PairwiseJudgment { self.judgment }
    fn confidence(&self) -> FiniteF64 { self.confidence }
}
```

The evaluator compares two concrete candidates over the same resolved cases and
returns one pairwise assessment:

```rust
pub struct PairwiseJudgeEvaluator<J> {
    judge: J,
    cases: Arc<CaseSet<QaCase>>,
    evidence_store: Arc<dyn EvidenceStore<PairwiseJudgeEvidence>>,
}

#[async_trait]
impl<J> Evaluator<PairwisePromptProblem> for PairwiseJudgeEvaluator<J>
where
    J: Judge<PairwisePromptProblem>,
{
    async fn evaluate(
        &self,
        request: EvaluationRequest,
        mut ctx: EvaluationContext<'_, PairwisePromptProblem>,
    ) -> Result<Metered<Vec<Assessment<PairwisePromptProblem>>>, EvaluationError> {
        let EvaluationRequest::Pairwise { left, right, set, order, .. } = request
            else { return Err(EvaluationError::UnsupportedRequestShape); };

        let left_artifact = ctx.graph().artifact(left).unwrap().clone();
        let right_artifact = ctx.graph().artifact(right).unwrap().clone();
        let cases = set.resolve(&self.cases)?;

        let judged = self.judge.compare_pair(
            &left_artifact,
            &right_artifact,
            cases,
            ctx.budget_handle(),
        ).await?;

        let evidence = PairwiseJudgeEvidence {
            judgment: judged.judgment,
            confidence: judged.confidence,
            rationale_ref: self.evidence_store.put_blob(&judged.rationale).await?,
        };

        Ok(Metered::new(vec![Assessment::Pairwise {
            left,
            right,
            order,
            target: AssessmentTarget::EvaluationSet(set.id()),
            evidence,
            cost: judged.cost.clone(),
            metadata: MetadataBag::new(),
        }], judged.cost))
    }
}
```

The fitted preference model is explicit and owned by the population:

```rust
pub struct BradleyTerryFit {
    scores: BTreeMap<CandidateId, FiniteF64>,
    observations: usize,
}

impl PreferenceModel<PairwisePromptProblem> for BradleyTerryFit {
    fn observe_pairwise(
        &mut self,
        left: CandidateId,
        right: CandidateId,
        judgment: PairwiseJudgment,
    ) -> Vec<ModelEvent> {
        self.observations += 1;
        self.update_fit(left, right, judgment)
    }

    fn compare(
        &self,
        left: CandidateId,
        right: CandidateId,
        _scope: PreferenceScope,
        _graph: RunGraphView<'_, PairwisePromptProblem>,
    ) -> Preference {
        self.compare_scores(left, right)
    }

    fn score(&self, candidate: CandidateId) -> Option<FiniteF64> {
        self.scores.get(&candidate).copied()
    }
}
```

The optimizer drives observation. The engine records the assessment; it does not
mutate `TournamentPopulation` automatically.

```rust
pub struct PairwiseTournamentOptimizer<Pr> {
    proposer: Pr,
    population: TournamentPopulation<PairwisePromptProblem, BradleyTerryFit>,
    selector: ThompsonCandidateSelector,
}

#[async_trait]
impl<Pr> Optimizer<PairwisePromptProblem> for PairwiseTournamentOptimizer<Pr>
where
    Pr: Proposer<PairwisePromptProblem, Request = MutateCandidate>,
{
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, PairwisePromptProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let parent = self.selector
            .select(
                self.population.view(ctx.graph()),
                ctx.graph(),
                ctx.selection_context(Arity::Single),
            )?
            .candidates
            .pop()
            .ok_or(OptimizerError::EmptyPopulation)?;

        let proposal = ctx.propose(&self.proposer, MutateCandidate { parent }).await?;
        let applied = ctx.apply_batch(proposal.batch).await?;
        let Some(child) = applied.successful_candidates().next() else {
            return Ok(StepStatus::Continue);
        };

        let events = self.population.observe_candidate(child, ctx.graph());
        ctx.record_population_events(self.population.id(), events);

        let report = ctx.evaluate_with(
            EvaluatorId::PAIRWISE_JUDGE,
            EvaluationRequest::Pairwise {
                left: parent,
                right: child,
                set: EvaluationSet::Partition(PartitionId::TRAIN),
                granularity: AssessmentGranularity::Aggregate,
                purpose: EvaluationPurpose::Selection,
                order: PairOrder::Randomized,
            },
        ).await?;

        for assessment in report.assessments {
            let events = self.population.observe_assessment(assessment.id, ctx.graph());
            ctx.record_population_events(self.population.id(), events);
        }

        Ok(StepStatus::Continue)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, PairwisePromptProblem>,
    ) -> Option<CandidateId> {
        self.population.best(graph)
    }
}
```

Driver code:

```rust
let result = optimize(seed_prompt_program)
    .train(train_cases)
    .evaluator(EvaluatorId::PAIRWISE_JUDGE, PairwiseJudgeEvaluator {
        judge: LmJudge::new(judge_lm),
        cases: Arc::new(train_cases.clone()),
        evidence_store: Arc::new(SqliteEvidenceStore::open("pairwise.db")?),
    })
    .using(PairwiseTournamentOptimizer {
        proposer: LocalPromptEditProposer::new(edit_lm),
        population: TournamentPopulation::new(BradleyTerryFit::default()),
    })
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

Key takeaways:

- This optimizer does not touch `leaven-gepa`.
- Pairwise comparison is one `Assessment::Pairwise`, not two independent scores.
- `BradleyTerryFit` is stateful model data owned by `TournamentPopulation`.
- The optimizer explicitly decides which assessments update the fitted model.

### 22.5 Worked example: `gskill` — GEPA over a skill-directory surface

`gskill` evolves a directory of skill files. The evaluator runs an LLM-based coding agent inside a sandboxed workspace against each task; the proposer is reflective. This exercises `EditSurface`, `Materializer`, `WorkspaceFactory`, trace-aware attribution, and the trust boundary in §16.

```rust
pub struct GskillProblem;
impl OptimizationProblem for GskillProblem {
    type Artifact = SkillDir;                    // a directory of .md skill files
    type Case = SweSmithTask;                    // task, repo, expected behavior
    type Evidence = ResolveEvidence;             // pass/fail + agent transcript ref
    type ProposalAnnotations = ReflectionNotes;
}

#[derive(Clone)]
pub struct SkillDir {
    files: BTreeMap<SkillFileId, Arc<str>>,
    identity: ArtifactIdentity,
}

impl Artifact for SkillDir {
    type Change = SkillEdit;                     // add/edit/remove a single file, or multi-edit
    type ApplyError = SkillError;
    fn identity(&self) -> ArtifactIdentity { self.identity.clone() }
    fn apply_change(&self, c: &SkillEdit) -> Result<Self, _> { /* clone+mutate, re-identify */ }
}

pub struct SkillDirByFrontmatterId;

impl EditSurface<SkillDir> for SkillDirByFrontmatterId {
    type PartId = SkillFileId;
    type Address = SkillPath;
    type View<'a> = SkillFileView<'a>;
    type Edit = SkillFileEdit;

    fn fingerprint(&self) -> SurfaceFingerprint { /* parser + ID rule fingerprint */ }
    fn parts<'a>(&self, dir: &'a SkillDir) -> Result<Vec<Part<SkillFileId, SkillPath, SkillFileView<'a>>>, SurfaceError> { /* one stable logical skill per file */ }
    fn change_part(&self, dir: &SkillDir, id: SkillFileId, edit: SkillFileEdit) -> Result<SkillEdit, SurfaceError> { /* lower surface edit */ }
}

pub struct SkillDirMaterializer;

impl Materializer<GskillProblem, SkillDir> for SkillDirMaterializer {
    async fn materialize_into(
        &self,
        dir: &SkillDir,
        ws: &mut WorkspaceView<'_>,
        ctx: MaterializeContext<'_, GskillProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        let mut count = 0;
        for (id, content) in &dir.files {
            ws.write_file(
                WorkspacePath::new(format!("skills/{}.md", id))?,
                content.as_bytes(),
            ).await?;
            count += 1;
        }
        Ok(Metered::new(MaterializationReport::file_count(count), Cost::zero()))
    }
}

pub struct GskillEvaluator<R: AgentRuntime> {
    workspace_factory: Arc<dyn WorkspaceFactory>,
    runtime: R,
    artifact_materializer: SkillDirMaterializer,
    cases: Arc<CaseSet<SweSmithTask>>,
    evidence_store: Arc<dyn EvidenceStore<ResolveEvidence>>,
}

#[async_trait]
impl Evaluator<GskillProblem> for GskillEvaluator<MiniSweAgentRuntime> {
    async fn evaluate(
        &self,
        request: EvaluationRequest,
        mut ctx: EvaluationContext<'_, GskillProblem>,
    ) -> Result<Metered<Vec<Assessment<GskillProblem>>>, EvaluationError> {
        let EvaluationRequest::Independent { candidates, set, .. } = request
            else { return Err(EvaluationError::UnsupportedRequestShape); };

        let mut out = Vec::new();
        let mut total_cost = Cost::zero();

        for cand in candidates {
            // Own the artifact before awaiting. Do not hold graph-backed views
            // across workspace rendering or agent runtime calls.
            let artifact = ctx.graph().artifact(cand).unwrap().clone();
            let mut per_case = BTreeMap::new();

            for case_id in set.resolve(&self.cases)? {
                let case = self.cases.get(case_id).clone();
                let artifact_for_case = artifact.clone();

                // Fresh workspace per case. `with_workspace` tears down the
                // sandbox on success and error, so a `?` below cannot leak E2B
                // sandboxes or remote worktrees.
                let (case_result, cost) = with_workspace(
                    &*self.workspace_factory,
                    WorkspaceConfig::default(),
                    |ws| Box::pin(async move {
                        let materialized = self.artifact_materializer.materialize_into(
                            &artifact_for_case,
                            &mut ws.view(),
                            ctx.materialize_context(),
                        ).await?;

                        let session = self.runtime.run_session(ws, AgentSessionConfig {
                            task: case.task_description.clone(),
                            repo: case.repo_url.clone(),
                            skills_path: WorkspacePath::new("skills")?,
                            budget: ctx.budget_handle(),
                        }).await?;

                        let trace_ref = self.evidence_store.put_blob(&session.transcript).await?;
                        let cost = materialized.cost.checked_add(&session.cost)?;

                        Ok((ResolveCaseResult {
                            resolved: session.resolved,
                            trace_ref,
                        }, cost))
                    }),
                ).await?;

                total_cost.checked_add_assign(&cost)?;
                per_case.insert(case_id, case_result);
            }

            out.push(Assessment::Independent {
                candidate: cand,
                target: AssessmentTarget::EvaluationSet(set.id()),
                evidence: ResolveEvidence { per_case },
                cost: total_cost.clone(),
                metadata: MetadataBag::new(),
            });
        }

        Ok(Metered::new(out, total_cost))
    }

    fn cache_policy(&self, _: &EvaluationRequest) -> CachePolicy { CachePolicy::Never }
    fn fingerprint(&self) -> Fingerprint { /* runtime + cases versions */ }
    fn id(&self) -> EvaluatorId { EvaluatorId::PRIMARY }
}

impl CasewiseEvidence for ResolveEvidence {
    fn case_outcome(&self, case: CaseId) -> Option<CaseOutcome> {
        self.per_case.get(&case).map(|r| CaseOutcome::pass_fail(r.resolved))
    }

    fn case_outcomes(&self) -> Vec<(CaseId, CaseOutcome)> { /* map per_case */ }
}

impl AttributableEvidence<SkillFileId> for ResolveEvidence {
    fn attribution_domain(&self) -> AttributionDomain {
        AttributionDomain::Surface(SkillDirByFrontmatterId.fingerprint().0)
    }

    fn attributions(&self) -> Vec<Attribution<SkillFileId>> {
        /* evaluator-parsed trace blame: failed tests -> skill ids */
    }

    fn evidence_for(&self, key: &SkillFileId) -> Option<AttributionEvidence<'_>> { /* trace slice */ }
}
```

Driver code:

```rust
let result = optimize(seed_skills)
    .train(swe_smith_tasks)
    .partitions(&[(PartitionId::TRAIN, train_ids), (PartitionId::VALIDATION, val_ids)])
    .evaluator(GskillEvaluator {
        workspace_factory: Arc::new(E2BFactory::pooled(/* … */)),
        runtime: MiniSweAgentRuntime::new(/* model config */),
        cases: Arc::new(swe_smith_tasks.clone()),
        evidence_store: Arc::new(SqliteEvidenceStore::open("traces.db")?),
    })
    .using(
        Gepa::default()
            .surface(SkillDirByFrontmatterId)
            .proposer(ReflectiveMutation::with_lm(reflection_lm))
            .part_selector(InvokedAndFailingPart::default())
            .population(ParetoFrontier::by_case())
    )
    .trust_policy(TrustPolicy::hide_from_proposer([PartitionId::VALIDATION]))
    .budget(Budget::usd(50.0))
    .run()
    .await?;
```

Key takeaways:

- `EditSurface` is the bridge from artifact to selectable/editable skill parts.
- `Materializer` is the bridge from typed artifact to filesystem layout the agent reads.
- `WorkspaceFactory` (here e2b, pooled) handles sandbox topology; the evaluator uses `&Workspace` agnostically.
- One workspace per case is the evaluator's choice — for skill evolution where the agent mutates the repo, isolation matters.
- `CasewiseEvidence` feeds GEPA's instance Pareto frontier; `AttributableEvidence<SkillFileId>` feeds trace-aware part selection.
- `EvidenceStore` keeps multi-MB agent transcripts out of the inline graph.
- `TrustPolicy::hide_from_proposer` ensures the reflective proposer never sees validation case content.

### 22.6 Worked example: Meta-Harness — agentic proposer over full graph history

Meta-Harness (Lee et al. 2026) writes a fresh harness program each iteration, with a coding-agent proposer that reads the entire run history through a filesystem. This exercises `Materializer`, `ProposalEffect::Create`, `Arity::None`, multi-axis `ParetoFrontier`, and the rendering of large execution traces.

```rust
pub struct MetaHarness;
impl OptimizationProblem for MetaHarness {
    type Artifact = HarnessArtifact;             // single .py file
    type Case = ClassificationCase;
    type Evidence = HarnessEvidence;             // per-case correctness + token cost
    type ProposalAnnotations = ProposerNotes;
}

// Artifact, Evaluator — same pattern as gskill, omitted for brevity.
// HarnessArtifact is rendered by a Materializer that writes harness.py.

// The history materializer is the load-bearing piece. It populates a workspace
// with per-candidate directories the agent will grep.
pub struct MetaHarnessHistoryMaterializer<AM, TM> {
    artifact_materializer: AM,    // Materializer<MetaHarness, HarnessArtifact>
    traces_materializer:   TM,    // Materializer<MetaHarness, HarnessEvidence>
    task_description:   Arc<str>,
    instructions:       Arc<str>,
}

#[async_trait]
impl<AM, TM> Materializer<MetaHarness, HistorySnapshot<'_>>
    for MetaHarnessHistoryMaterializer<AM, TM>
where
    AM: Materializer<MetaHarness, HarnessArtifact>,
    TM: Materializer<MetaHarness, HarnessEvidence>,
{
    async fn materialize_into(
        &self,
        snap: &HistorySnapshot<'_>,
        ws: &mut WorkspaceView<'_>,
        ctx: MaterializeContext<'_, MetaHarness>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        ws.write_file(WorkspacePath::new("README.md")?, self.instructions.as_bytes()).await?;
        ws.write_file(WorkspacePath::new("TASK.md")?, self.task_description.as_bytes()).await?;

        let mut harnesses = ws.subdir(WorkspacePath::new("harnesses")?)?;
        for &cand in &snap.visible_candidates {
            let cand_view = snap.graph.candidate(cand).unwrap();
            let mut dir = harnesses.subdir(WorkspacePath::new(directory_name_for(cand, &snap.graph))?)?;

            self.artifact_materializer.materialize_into(&cand_view.artifact, &mut dir, ctx.clone()).await?;
            dir.write_file(
                WorkspacePath::new("scores.json")?,
                scores_summary_json(&snap.graph, cand).as_bytes(),
            ).await?;

            let mut traces = dir.subdir(WorkspacePath::new("traces")?)?;
            for assessment in snap.graph.assessments(cand) {
                // ReadScope hides test-partition assessments; materializer respects it
                if !is_visible(&assessment.target, &ctx.read_scope()) { continue; }
                self.traces_materializer.materialize_into(&assessment.evidence, &mut traces, ctx.clone()).await?;
            }
        }
        let _ = ws.subdir(WorkspacePath::new("output")?)?;     // where the agent writes new harnesses
        Ok(Metered::new(MaterializationReport::default(), Cost::zero()))
    }
}

// The proposer's request type — owned and lightweight. The proposer builds
// the borrowed HistorySnapshot internally from ctx.graph() inside propose().
// This avoids putting a lifetime on the trait's associated type.
pub struct HistoryProposalRequest {
    /// Which candidates the optimizer wants the agent to consider.
    /// Computed by the optimizer from its read scope; the proposer doesn't
    /// re-derive visibility.
    pub visible_candidates: Vec<CandidateId>,
    pub k: usize,
}

// The agentic proposer. arity = None: no causal parents.
// Note the materializer is a stage-owned field, not a registry lookup.
pub struct AgenticHarnessProposer<R, HR> {
    runtime: R,                                            // claude-code wrapper
    history_materializer: HR,
}

#[async_trait]
impl<R, HR> Proposer<MetaHarness> for AgenticHarnessProposer<R, HR>
where
    R: AgentRuntime,
    // The materializer takes any HistorySnapshot lifetime; we'll feed it a borrow
    // of the graph view we hold inside propose().
    HR: for<'a> Materializer<MetaHarness, HistorySnapshot<'a>>,
{
    type Request = HistoryProposalRequest;

    fn id(&self) -> ProposerId { ProposerId::new("meta_harness/claude_code") }
    fn arity(&self) -> Arity { Arity::None }

    async fn propose(
        &self,
        request: Self::Request,
        mut ctx: ProposalContext<'_, MetaHarness>,
    ) -> Result<Metered<ProposalBatch<MetaHarness>>, ProposalError> {
        // Build the borrowed snapshot from ctx.graph(). It lives only for this
        // call; the materializer consumes it before the await on run_session.
        let snapshot = HistorySnapshot {
            graph: ctx.graph(),
            visible_candidates: &request.visible_candidates,
            current_iteration: ctx.current_iteration(),
        };

        let factory = ctx.workspace.as_ref().unwrap();
        with_workspace(factory, WorkspaceConfig::default(), |ws| Box::pin(async move {
            let materialized = self.history_materializer.materialize_into(
                &snapshot, &mut ws.view(), ctx.materialize_context()
            ).await?;

            let session = self.runtime.run_session(ws, AgentSessionConfig {
                task: HARNESS_SEARCH_PROMPT,
                output_dir: WorkspacePath::new("output")?,
                budget: ctx.budget_handle(),
            }).await?;

            let referenced = parse_referenced_candidates(&session.transcript);

            let mut proposals = Vec::new();
            for i in 0..request.k {
                let Ok(source_bytes) = ws.read_file(
                    WorkspacePath::new(format!("output/harness_{i}.py"))?,
                ).await else { continue };
                let source = String::from_utf8(source_bytes)?;
                let notes = read_optional(
                    ws,
                    WorkspacePath::new(format!("output/notes_{i}.md"))?,
                ).await;

                // ProposalEffect::Create — brand-new authored artifact.
                // No "Change applied to nothing" lie; the proposal honestly says
                // "create this artifact, here's what informed me."
                proposals.push(
                    Proposal::create(HarnessArtifact::from_source(Arc::from(source)))
                        .informed_by(referenced.iter().map(|&c| InfoRef::Candidate(c)))
                        .annotations(ProposerNotes { rationale: notes })
                        .build()
                );
            }

            Ok(Metered::new(
                ProposalBatch {
                    proposals,
                    semantics: ProposalBatchSemantics::Alternatives,
                    metadata: MetadataBag::new(),
                },
                materialized.cost.checked_add(&session.cost)?,
            ))
        })).await.map_err(ProposalError::from)
    }
}

// Optimizer: no candidate selector, no merge, just propose-evaluate-observe.
pub struct MetaHarnessOptimizer<R, HR, Axes> {
    proposer:    AgenticHarnessProposer<R, HR>,
    population:  ParetoFrontier<MetaHarness, Axes>,
    k_per_iter:  usize,
}

#[async_trait]
impl<R, HR, Axes> Optimizer<MetaHarness> for MetaHarnessOptimizer<R, HR, Axes>
where
    R: AgentRuntime,
    HR: Materializer<MetaHarness, HistorySnapshot<'static>>,
    Axes: ParetoAxes<MetaHarness>,
{
    async fn step(&mut self, ctx: &mut RunContext<'_, MetaHarness>)
        -> Result<StepStatus, OptimizerError>
    {
        // Compute which candidates are visible to the proposer right now.
        // This is owned data; no graph references in the request itself.
        let visible_candidates = ctx
            .graph()
            .candidates_visible_to(ctx.read_scope())
            .collect();

        let report = ctx.propose(
            &self.proposer,
            HistoryProposalRequest {
                visible_candidates,
                k: self.k_per_iter,
            },
        ).await?;

        // apply_batch processes every proposal; interface validation lives in
        // HarnessArtifact::apply (or the artifact constructor for Create);
        // failures land as ApplyFailed events in the graph.
        let applied = ctx.apply_batch(report.batch).await?;

        for cand_id in applied.successful_candidates() {
            let eval = ctx.evaluate(EvaluationRequest::Independent {
                candidates: vec![cand_id],
                set: EvaluationSet::Partition(PartitionId::SEARCH),
                granularity: AssessmentGranularity::PerCase,
                purpose: EvaluationPurpose::Search,
            }).await?;
            for a in eval.assessments {
                let events = self.population.observe_assessment(a.id, ctx.graph().view());
                ctx.record_population_events(self.population.id(), events);
            }
        }
        Ok(StepStatus::Continue)
    }

    fn best_candidate(&self, g: RunGraphView<'_, MetaHarness>) -> Option<CandidateId> {
        self.population.best(g)
    }
}
```

Driver code:

```rust
let frontier = ParetoFrontier::<MetaHarness, _>::builder()
    .axis_extracted("accuracy",       Direction::HigherIsBetter,
                    |e: &HarnessEvidence| e.accuracy())
    .axis_extracted("context_tokens", Direction::LowerIsBetter,
                    |e: &HarnessEvidence| e.context_tokens() as f64)
    .partition_filter(|t| matches!(t,
        AssessmentTarget::EvaluationSet(id) if is_search_partition(id)))
    .build();

let result = optimize(seed_harnesses)
    .train(text_classification_tasks)
    .partitions(&[(PartitionId::SEARCH, search_ids), (PartitionId::TEST, test_ids)])
    .evaluator(HarnessEvaluator { /* … */ })
    .using(MetaHarnessOptimizer {
        proposer: AgenticHarnessProposer { /* claude code in firecracker */ },
        population: frontier,
        history_materializer: history_materializer.clone(),
        k_per_iter: 2,
    })
    .trust_policy(TrustPolicy::hide_from_proposer([PartitionId::TEST]))
    .budget(Budget::new().iterations(20).usd(500.0))
    .run()
    .await?;
```

Key takeaways:

- **`ProposalEffect::Create` and `Arity::None`** are essential for this style: the agent authors fresh harnesses each iteration; the proposal is honestly a `Create`, not a `Change` whose target is meaningless. Lineage is bibliographic via `informed_by`, never causal.
- **`Materializer`** is the load-bearing primitive. The orchestrator materializer composes per-artifact and per-evidence sub-materializers into the candidate-per-directory layout the agent greps.
- **`ParetoFrontier::partition_filter`** keeps the test partition out of the frontier even though it's still observable to post-run evaluation.
- **`TrustPolicy::hide_from_proposer`** combines with the materializer's `read_scope` check to ensure test-partition traces never appear in the agent's workspace.
- **`EvidenceStore`** is non-optional at this scale — execution traces can hit 10M tokens; only refs live in the graph.

---

## 23. Standard Library Surface

### 23.1 Artifacts

```text
TextArtifact
PartMapArtifact
DirArtifact
GitArtifact
#[derive(Optimize)]
```

### 23.2 Evidence

```text
ScalarEvidence
ScoreVectorEvidence
PairwiseJudgmentEvidence
ListwiseRankingEvidence
MixedEvidence
StringEvidence
JsonEvidence
```

### 23.3 Preference relations

```text
HigherScoreIsBetter
LowerScoreIsBetter
ParetoPreference
LexicographicPreference
CopelandPreference
BordaPreference
CondorcetPreference
```

Note: `BradleyTerryFit` and `PlackettLuceFit` are *fitted models*, not stateless preference relations. They live on `TournamentPopulation<F>` (see §15.1 and §23.4).

### 23.4 Populations

```text
KeepBest
ParetoFrontier
MapElites
BeamPopulation
IslandsPopulation
TournamentPopulation
LenientParetoFrontier
NoveltyPopulation
NoPopulation
```

### 23.5 GEPA pieces

```text
ReflectiveMutation
SystemAwareMerge
ParetoFrequencyWeighted
RoundRobinPart
InvokedAndFailingPart
EpochShuffled
StrictImprovement
ImprovementOrEqual
NoRegression
FullValidation
MinibatchThenValidation
```

### 23.6 Stages

```text
EvaluatorFn
ProposerFn
LmProposer
AgenticProposer
DiagnoseAndPropose
SurrogateProposer
EnsembleProposer
```

---

## 24. Trait Laws

### Artifact

```text
apply is functional.
failed apply does not mutate artifact state.

identity is stable for the artifact state recorded in the graph.
same artifact state under the artifact's own identity law => same
  ArtifactIdentity.

ArtifactIdentity is not automatically cache identity.

contract on the user (not framework-enforced):
  - artifacts are observationally immutable; no interior mutability
    that affects library-visible behavior.
  - identity is deterministic across machines when the run is durable.
  - external mutable handles are allowed as graph identity, but must not
    be returned as CacheIdentity.
```

### CacheIdentified

```text
CacheIdentity encodes everything a deterministic evaluator may depend on:
  artifact state, immutable external content reference, or explicit user key.

the cache trusts CacheIdentity absolutely. lying about it produces
  silently incorrect cache results.

content-addressed external handles may be CacheIdentity::ExternalContent:
  - git commit hash
  - IPFS CID
  - OCI/docker image digest

mutable external handles must return None:
  - branch name
  - filesystem path
  - unversioned S3 key
  - database row ID without version/ETag

hash strength:
  - blake3 or sha-256 recommended for durable cross-run cache keys.
  - 128-bit non-cryptographic hashes acceptable for in-process cache only.
  - 64-bit hashes are unsafe at typical run scales (>10^5 candidates).
```

### EditSurface

```text
surface fingerprint changes when interpretation changes:
  parsing rules, filters, ignored files, ID extraction, layout policy.

part IDs are scoped to SurfaceFingerprint.
if identity is path-based, rename is remove + add.
if rename continuity matters, the surface must encode stable logical IDs.

AttributableEvidence<S::PartId> may be consumed only when its attribution
domain matches the surface fingerprint.

borrowed surface views are inspection-only. async stages turn them into
owned request/rendering data before awaiting external work.
```

### Evaluator

```text
returns assessments matching request shape.
reports all costs.
declares cache policy honestly.
must not mutate artifact state in graph.
must return UnsupportedGranularity when it cannot provide requested granularity.
```

### PreferenceRelation

```text
may return Incomparable.
must document whether it is total, partial, stochastic, fitted, or graph-derived.
must not silently treat missing evidence as zero unless explicitly documented.
```

### Population

```text
PopulationView exposes only existing candidates.
best may return None.
population events are strategy opinions, not graph truth.
population must not erase graph history.
```

### Optimizer

```text
all graph mutations happen through RunContext.
all costful work happens through metered stages or explicit budget charges.
may stop itself, but external stoppers remain engine-owned.
```

### Proposal (effect / provenance validation)

The framework validates these combinations before recording a proposal in the
graph. Invalid combinations return `ApplyError::InvalidProposal` and are
recorded as `ApplyFailed` events. They are NOT silent passes.

```text
ProposalEffect::Create  + CausalInputs::None         OK  (fresh authoring; Meta-Harness)
ProposalEffect::Create  + CausalInputs::NAry(...)    OK  (aggregate of N -> 1)
ProposalEffect::Create  + CausalInputs::Single(_)    INVALID
                                                     (use Change with that target instead)
ProposalEffect::Create  + CausalInputs::Pair(_, _)   INVALID
                                                     (Pair only meaningful for merge under Change)

ProposalEffect::Change  + CausalInputs::Single(p)    OK iff target == p
                                                     (otherwise: which is the apply target?)
ProposalEffect::Change  + CausalInputs::Pair(a, b)   OK iff target == a OR target == b
                                                     (target is the canonical apply parent;
                                                      change embeds content from the other)
ProposalEffect::Change  + CausalInputs::None         INVALID
                                                     (cannot apply to nothing)
ProposalEffect::Change  + CausalInputs::NAry(...)    OK iff target ∈ NAry list
                                                     (rare; for n-ary structured merges)
```

informed_by has no validation constraints — it's a free-form bibliography of
candidates, assessments, proposals, or external references the proposer read.
Empty informed_by is fine. Self-referential informed_by (where the proposer
records reading evidence about a candidate it later created) is also fine
because artifact identity determines whether they're the same graph state.

### Renderer / Materializer

```text
rendering is a view, not a transformation of truth.
renderer is the ordinary LM/debug/value path.
materializer is the workspace path for agents, sandboxes, and subprocess tools.
lossy rendering must be explicit.
target (or workspace contents) determines rendering shape.
costful rendering reports cost via Metered.
Materializers must respect the caller's read_scope:
  do not write evidence from forbidden partitions into the workspace.
Materializers should be idempotent within a single workspace
  (calling the same materializer twice with the same value is a no-op or
  produces the same files).
```

---

## 25. Expressibility Targets

The design must express naturally:

```text
GEPA
GEPA+Merge
MIPRO / MIPROv2
TextGrad
Trace / OptoPrime
MuF/Edit
MAP-Elites for prompts
C-Evolve
MOPrompt
GSkill
MemSkill
SkillFoundry
EvoSkills
Graph-of-Skills
Memento-Skills
VISTA
TEP
Pareto-lenient consensus
AlphaEvolve / OpenEvolve / ShinkaEvolve
pairwise-tournament continual learning
single-task keep-best search
recursive meta-optimization
ComBE-style aggregation
confidence-aware logprob-derived evaluators
```

Pass condition: a competent model can implement the optimizer using user-facing primitives without new core traits or engine modifications.

---

## 26. Implementation Plan

The prototypes are deliberately ordered to surface design problems early. P2 stresses what is *new* in this design (Pairwise eval requests, fitted preference relations, tournament populations); P3 validates that the design *also* expresses the well-understood case (GEPA). If P2 fights the API, you learn at the cheapest possible moment. If P3 lands clean, GEPA parity is a refinement exercise rather than a validation step.

### Prototype 1: scalar keep-best single-task

Goal: prove `Optimizer + RunContext + RunGraph`.

```text
TextArtifact
scalar evidence
HigherScoreIsBetter
KeepBest
simple mutation proposer
no dataset or singleton dataset
```

### Prototype 2: pairwise tournament

Goal: stress the parts of this design that don't exist in Python GEPA — pairwise evaluation requests, fitted preference relations on populations, tournament-style step rhythms.

```text
pairwise LLM judge evaluator
PairwiseJudgmentEvidence
TournamentPopulation (owns its Bradley-Terry fit)
EvaluationRequest::Pairwise
EvaluatorRegistry with EvaluatorId::PAIRWISE_JUDGE
```

### Prototype 3: GEPA parity

Goal: reproduce Python GEPA shape naturally on top of the surface validated by P1 and P2.

```text
PartMapArtifact
PartMapSurface
ReflectiveMutation
ProposalBatch::Alternatives
AssessmentGranularity::PerCase
CasewiseEvidence
ParetoFrontier::by_case
ParetoFrequencyWeighted
RoundRobinPart
StrictImprovement
train/validation partitions
```

### Prototype 4: agentic Git artifact

Goal: prove rendering, materialization, trust boundaries, and the workspace lifecycle.

```text
GitArtifact + GitWorktreeFactory
Materializer composition (artifact + traces + history orchestrator)
agentic proposer with ProposalEffect::Create
repo-task evaluator with isolated workspaces
AgentTrajectoryEvidence via EvidenceStore
budget and sandbox hooks
```

---

## 27. Open Questions

### 27.1 Renderer registry typing — DEFERRED

Rendering is split into two trait families: `Renderer<P, T, Target>` (value-returning) and `Materializer<P, T>` (side-effecting workspace population). See §13. v0.2.2 does not ship a renderer/materializer registry or erased renderer traits. Stage-owned typed fields are the implementation path. Revisit a registry only after a real plugin/debug user needs runtime rendering choices.

### 27.2 Evidence persistence

Core should not require serde on `Evidence`. Default stores support serde evidence; large evidence uses external stores. See §19.

### 27.3 Optimizer dyn dispatch

Do we need `Box<dyn DynOptimizer<P>>`? Probably not for v0.1. Optimizers are static values. Revisit if runtime-loaded optimizers become necessary.

### 27.4 Distributed execution

Out of scope for v0.1. Graph/event design should not preclude future merging.

### 27.5 Cache correctness for stochastic evaluators

Default no-cache. Deterministic cache only with explicit evaluator fingerprint and cache policy.

### 27.6 Preference relation state — RESOLVED

Resolved by placing fitted/stateful preference models on `Population` impls (concretely `TournamentPopulation`) rather than on `PreferenceRelation`. The state of a fitted model depends on accumulated observations; updates fit naturally into `observe_assessment`; `best` and `view` expose the fit to selectors without making `PreferenceRelation` stateful. See §14 and §15.1.

### 27.7 Renderer registry vs stage-owned composition

Surfaced by the Meta-Harness walkthrough: a complex `Materializer` (e.g. `MetaHarnessHistoryMaterializer`) is a composition of smaller materializers (`ArtifactMaterializer`, `TracesMaterializer`). Today these compose as generic stage-owned fields. A typed registry or object-safe adapter could replace these fields later, but field-based composition is more explicit and easier to typecheck. Defer a registry until a real second user wants different sub-materializers without recompiling.

---

## 28. Non-goals

```text
Python GEPA API compatibility
CLI
hosted service
distributed engine
built-in observability backend
specific LLM SDK dependency
automatic artifact-structure inference
automatic evidence-shape inference
skill marketplace
domain-specific shortcuts in core
```

---

## 29. Final Design Thesis

The library should make this sentence true:

> A Rust optimizer is a configured value that drives a typed run graph by proposing changes to artifacts, requesting assessments, interpreting evidence through preference relations, and maintaining live populations, while the engine provides budgeted, observable, capability-scoped execution.

Everything else falls out.

GEPA is one optimizer. MIPRO is one optimizer. TextGrad is one optimizer. A future paper should be one optimizer.

The engine is dumb. The optimizer is smart. The types tell the truth.

---

## 30. Changelog

### v0.2.7 (2026-05-07) - command-backed provider runtime cutover

This pass makes command-backed execution through `WorkspaceView::run_command`
the default product path for provider CLIs and demotes app-server-over-stdio to
a local-mount adapter.

- **Provider CLIs run inside the workspace backend.** The runtime writes
  provider setup files, invokes the CLI through workspace command APIs, captures
  native logs/session artifacts, validates output contracts, and returns a
  durable `AgentSession`.
- **Codex app-server stdio is a local compatibility path.** It stays useful
  for local app-server sessions, but it requires `local_mount()` and must not be
  presented as backend-neutral for containers or remote sandboxes.
- **Workspace command semantics are specified.** Command cwd, env, stdin,
  timeouts, output limits, optional user identity, exit status, duration, and
  truncation are part of the workspace capability contract.
- **Runtime setup files are operational presentation.** Provider homes, skill
  registrations, MCP config, native logs, and session files become graph state
  only when materialized from artifacts or parsed back into typed proposals.
- **Session durability becomes implementation-facing.** Agent sessions and
  transcripts must be serializable enough for evidence stores and checkpoint
  resume so milestone examples stop hand-rolling persistence.
- **Companion plan added.** See
  `docs/plans/2026-05-07-harbor-style-agent-runtime.md`.

### v0.2.6 (2026-05-07) - live EvoSkill iteration proof

This pass replaces the rejected toy P5 skill-paper map with a live
EvoSkill-shaped iteration that exercises the real agentic skill substrate.

- **P5 now runs one live EvoSkill iteration.**
  `examples/p5_evoskill_iteration` drives an empty `SkillBank` through executor
  failure, proposer diagnosis, skill-builder output, skill-folder validation,
  workspace proposal parsing, `RunContext` proposal application, child
  evaluation, `KeepBest` observation, evidence persistence, and checkpoint
  completion.
- **Codex execution is mandatory for the gate.** The milestone uses Codex CLI
  with `gpt-5.4-mini`, low reasoning, and developer instructions.
  The run is opt-in through `LEAVEN_CODEX_LIVE=1` because it spends live model
  calls.
- **Prompt fidelity is documented.** The example uses EvoSkill source prompts
  for the executor, proposer, and skill-builder, with a small Leaven/Codex
  wrapper that defines the skill mount and JSON output contract.
- **The proof is intentionally scoped.** The live gate proves Leaven product
  wiring; it is not yet a full OfficeQA/SealQA paper reproduction.
- **Paper-reproduction acceptance is tightened.** Future examples may own
  prompts, datasets, scorers, and harnesses, but must not reimplement generic
  skill artifacts, graph application, workspace parsing, evidence stores,
  checkpoint/restore, repair, runtime, or population substrate.
- **Companion plan updated.** See
  `docs/plans/2026-05-07-milestone-5-skill-paper-reproductions.md`.

### v0.2.5 (2026-05-07) - Codex app-server provider adapter

This pass specifies the first real provider runtime adapter needed for
agentic-skill paper reproduction while keeping provider code out of generic
Leaven layers.

- **Codex app-server is a leaf runtime adapter.**
  `leaven-agent-codex-app-server` implements provider-neutral `AgentRuntime`;
  `leaven-agent-codex` remains a thin facade. Neither knows candidates,
  proposals, assessments, `RunGraph`, GEPA, git artifacts, or skill banks.
- **Codex protocol dependencies are contained.** `codex-app-server-protocol`
  and process/protocol dependencies are confined to
  `leaven-agent-codex-app-server` and feature-gated from umbrella/facade
  crates.
- **Stdio app-server requires a local mount.** The stdio connector is honest
  about workspace semantics: pure-remote workspaces fail before launch unless
  they expose a real local mount. Non-stdio app-server execution should be a
  separate connector.
- **Codex app-server sessions are materialized by default.** Provider thread
  history is evidence and replay/debug substrate. Ephemeral sessions remain an
  explicit opt-in for throwaway runs, and the runtime does not call
  `thread/read includeTurns` for them because app-server intentionally refuses
  that operation on unmaterialized threads.
- **Request mapping is explicit.** `AgentRunRequest` maps to Codex
  `thread/start` and `turn/start`; output-contract validation remains runtime-
  level and proposal parsing remains stage-owned.
- **Transcript normalization is specified.** The adapter records assistant
  messages, commands, tool calls, output files, status, and raw provider events
  without leaking Codex protocol types into `leaven-agent`.
- **Codex skill layout has an owner without owning skills.** Codex-specific
  workspace layout and skill-reference ABI live in the provider crate, while
  skill folder validation, materialization, mutation, and proposal parsing stay in
  artifact/agentic layers.
- **DSRs copy path recorded.** The reusable pieces are the app-server
  transport/client/session/history patterns; DSRs repo materialization,
  steering policy, Firkin setup, and git readback stay out of the provider
  runtime.
- **Companion spec added.** See
  `docs/specs/codex_app_server_agent_runtime.md`.

### v0.2.4 (2026-05-07) - agentic skill optimization primitives

This pass records the generic substrate needed to reproduce current
skill-optimization papers without baking a paper-specific loop into the engine.

- **Skill folders are first-class artifacts.** A valid skill is an arbitrary
  directory with mandatory `SKILL.md` frontmatter (`name`, `description`) and
  optional scripts, references, assets, and extra files.
- **Skill changes are filesystem-native.** `ReplaceSkill` means replacing the
  whole folder; rewriting `SKILL.md` is a file write. File permissions are
  preserved without making executable files semantically special.
- **Skill surfaces are explicit.** Folder, file, manifest/frontmatter, and
  retrieval/index surfaces are named as standard lenses over a `SkillBank`.
- **Git is first-class but not default.** Git stores immutable artifact state;
  Leaven stores optimization causality. Checkout/readback strategies are
  operational details.
- **Paper pressure map added.** EvoSkill is the first reproduction target, while
  Trace2Skill, Memento-Skills, D2Skill, and SkillReducer define the remaining
  generic primitive requirements.
- **Companion spec added.** See
  `docs/specs/agentic_skill_optimization_primitives.md`.
- **Skill primitive design tightened.** Optional Agent Skills fields are not
  baked into core skill types; extra frontmatter is generic skill metadata.
  `SKILL.md` validity requires `name`, `description`, and non-empty body.
  `RenameSkill`, typed validation errors, bounded reproposal policy, standalone
  selector placement, and explicit private checkpoint state are called out as
  implementation-facing contracts.
- **Design-standard checklist added.** The companion spec now records the
  type, trait, error, and test invariants that must hold before the skill
  substrate is implemented.
- **Validation lifecycle clarified.** Apply/validate failure now has an
  explicit lifecycle: it records an `ApplyFailed` attempt and creates no
  candidate. Bounded repair/reproposal is proposer-owned stage policy before a
  `ProposalBatch` is returned, not hidden engine behavior.
- **Reproposal scoped to same proposer.** Proposal repair now routes back to
  the same proposer stage that authored the invalid proposal. The reusable
  primitive is proposal-stage scoped, suitable for skills, code-editing agents,
  harness generation, and config synthesis, not a generic evaluator retry loop.
- **Workspace proposal parsing specified.** Agentic proposers may parse edited
  workspaces into typed `ProposalBatch` values through their stage-owned
  `ProposalParser`. Parsers do not mutate the graph; `RunContext` remains
  graph authority.
- **Skill telemetry and utility state scoped.** Skill-use telemetry is optional
  evidence capability, not mandatory trajectory modeling. Skill utility is
  population/private optimizer state by default and becomes artifact state only
  when it changes candidate behavior.
- **Checkpoint restore laws expanded.** Checkpoints must preserve graph truth,
  explicit private optimizer/population/selector state, cache state, clean stage
  boundaries, and abandoned workspace facts without replaying committed work.

### v0.2.3 (2026-05-07) - agentic stage runtime contract

This pass pins the layer split for real agentic optimization over evolving
codebases, skill libraries, harnesses, manifests, and `AGENTS.md` files.

- **`AgentRuntime` narrowed to one session in one workspace.** It is
  provider-neutral execution vocabulary and has no dependency on core, engine,
  optimizer, graph, proposal, or assessment types.
- **`leaven-agentic` owns the stage adapters.** Agentic proposers and evaluators
  compose materializers, renderers, runtimes, and parsers, then return typed
  `ProposalBatch` or `Assessment` values.
- **Artifact semantics split from workspace layout.** Candidate-owned harness,
  skill, manifest, and agent-doc wiring lives in the artifact. Runnable
  filesystem layout, commands, and output contracts live in materializers and
  stage config.
- **Runtime capability matching is explicit.** Backends with no local mount are
  first-class; local-mount-only providers must declare the requirement and fail
  early when paired with remote workspaces.
- **Companion spec added.** See
  `docs/specs/agentic_stage_runtime.md`.

### v0.2.2 (2026-05-07) — workspace/materialization and GEPA selection minor bump

This pass records the decisions needed before implementing agentic stages and
`leaven-gepa`.

- **`WorkspaceRenderer` renamed to `Materializer`.** The side-effecting
  workspace trait now has workspace-native vocabulary and no compatibility
  alias. `Renderer` remains the value-returning path for ordinary LM calls;
  `Materializer` is the workspace path for agents, sandboxed evaluators, and
  subprocess-backed tools. `Materializer` itself is in scope for v0.2.2;
  only renderer/materializer registry erasure is deferred.
- **Workspace API made backend-neutral.** `Workspace` is a concrete Leaven lease
  handle; users implement factories/backends. File APIs use `WorkspacePath`.
  Examples no longer rely on `local_mount()` or backend-specific absolute paths.
- **Actor trust table added.** Optimizer, selector, proposer, evaluator,
  renderer, materializer, agent runtime, and callback capabilities are spelled
  out directly.
- **GEPA candidate selection split from population.** Populations expose
  archive/frontier/model state through `PopulationView`; `CandidateSelector`
  chooses which candidate to mutate next and is explicitly swappable.
- **Future skill-library direction captured.** The spec names likely extension
  points for skill routing, hard-case selection, target selection, and admission
  without pulling them into core.

### v0.2.1c (2026-05-06) — surface/evidence/cache finishing pass

This pass folds the v0.2.1b topology correction into the long-form spec and
settles the load-bearing seams before `leaven-gepa` implementation.

- **Artifact identity and cache identity split.** `Artifact::identity()` is graph
  truth; `CacheIdentified::cache_identity()` is the stronger deterministic cache
  promise. Mutable external handles are graph-valid but not cache-valid.
- **`Decomposable` removed from the main spec.** Parts now come from
  `EditSurface<A>` with explicit `SurfaceFingerprint` laws.
- **Evidence measurement and attribution split.** `CasewiseEvidence` feeds
  instance-pareto frontiers; `AttributableEvidence<K>` feeds trace-aware
  routing and credit assignment.
- **GEPA now owns `S: EditSurface<P::Artifact>`.** Generic GEPA proposers may
  emit surface edits; GEPA lowers them to artifact-native changes.
- **Workspace cleanup examples use `with_workspace`.** Examples no longer rely
  on `Drop` or local filesystem paths for remote workspaces.
- **Pairwise tournament worked example added.** It demonstrates a non-GEPA
  optimizer rhythm with pairwise assessments and a fitted Bradley-Terry model.

### v0.2.1a (2026-05-06) — pre-implementation patch

Project name locked: **leaven**. A pre-implementation review of v0.2.1 flagged real Rust-mechanics issues (lifetime-on-trait, async-Drop, scattered `&mut BudgetLedger`) and residual wording inconsistencies from the v0.2 → v0.2.1 edit pass. v0.2.1a is the last polish before P0/P1 prototypes.

#### Type-level fixes

- **`Proposer::Request` no longer requires `'static`.** Convention spelled out: requests are owned and lightweight (just identify *what to do*); proposers construct rich views internally from `ctx.graph()`. The Meta-Harness example was updated to construct its `HistorySnapshot` inside `propose`, not pass it through the trait's associated type. Removes lifetime gymnastics on the trait.
- **`<P::Artifact as Artifact>::Change` is the canonical change type.** `OptimizationProblem` does not define `type Change`. Constructor sugar signatures fixed throughout.

#### New explicit machinery

- **§8.4 Report types defined explicitly.** `ProposalBatchReport`, `ApplyReport`, `ApplyOneReport`, `EvaluationReport`. Reports return IDs and graph-backed views, not graph-owned values. Includes `ApplyOutcome::{Success, Failure}` for per-proposal outcome tracking.
- **§5.9 `ResolvedEvaluationRequest` and resolution boundary.** RunContext resolves dynamic sets (`Recent`, `Sample`, `Stratified`) before passing to evaluators; cache key uses `ResolvedEvaluationSetId`. Evaluators never see unresolved expressions. Both expressions are recorded in the graph.
- **§16.3 `BudgetHandle<'a>` is the single budget access type.** Replaces scattered `&'a mut BudgetLedger` references on `ProposalContext` and `EvalHandle`. Wraps ledger + stage tag; one mutable borrow path; `sub_stage()` for nested attribution. Prevents borrow-hostile multi-handle situations.

#### Lifecycle clarifications

- **`Workspace::cleanup(self)` is explicit, not Drop-driven.** Async cleanup cannot be reliably awaited in `Drop`. The trait now distinguishes `async fn cleanup` (full backend teardown) from `Drop::mark_abandoned` (sync best-effort marker for factory janitors). Stages must call `cleanup().await` explicitly; idiomatic pattern documented.
- **§24 Proposal validation laws.** Per-combination rules for `(ProposalEffect, CausalInputs)`. Cheap correctness checks before graph insertion. Invalid combinations produce `ApplyFailed` events, never silent passes.

#### Wording cleanup

- **`informed_by` consistently described as `ProposalProvenance::informed_by`.** §0.1 entry 17 and §10.2 doc comment fixed; "backed by typed metadata" wording removed.
- **`BradleyTerryPreference` removed from stateless-preference lists.** Renamed to `BradleyTerryFit` (model object), placed under populations as `TournamentPopulation<BradleyTerryFit>`. §3 nomenclature, §5.15, §14, and §23.3 updated. `CopelandPreference` and `BordaPreference` (stateless graph aggregation) remain where they were.

#### Branding

- Project named **leaven**. Tagline: *Optimize anything in Rust.* Crate plan: umbrella `leaven` re-exporting `leaven-core`, `leaven-engine`, `leaven-std`, `leaven-workspace`, `leaven-derive`. Metaphor matches the design's "set up conditions, walk away, come back to a transformed substrate" pattern.

#### Stress tests still pass

The four pressure tests from v0.2 (cross-branch synthesis, Meta-Harness, workspace lifecycle, multi-agent composite) all still pass against v0.2.1a. The §22.4 (gskill) and §22.5 (Meta-Harness) worked examples have been updated for the new types and the cleanup pattern.

### v0.2.1 (2026-05-06) — post-review tightening

External review of v0.2 (sharp, terse, mostly fair) flagged that v0.2 retained shapes from v0.1 that became lies once new capabilities were layered in. Specifically: a `Proposal` carrying `parents: Parents::None + change` was incoherent for fresh-author cases like Meta-Harness; `informed_by` was promised as "a typed graph relation" but backed by stringly-typed metadata; the universal `ProposalRequest<P>` would collapse to an enum or bag once multiple proposer shapes shipped. v0.2.1 fixes those without changing architecture.

#### Proposal model

- **`Proposal::effect: ProposalEffect`** replaces bare `change + parents` (§5.5). `ProposalEffect::Create { artifact }` for fresh authoring; `ProposalEffect::Change { target, change }` for mutation. Removes the "Change applied to nothing" lie that `Parents::None` produced.
- **`Proposal::provenance: ProposalProvenance`** replaces inline `parents` and stringly-typed informed_by (§5.5). `causal: CausalInputs` records lineage that contributed to artifact state; `informed_by: Vec<InfoRef>` records bibliographic influence that did not. Both are typed structured fields.
- **Constructor sugar** (`Proposal::mutate / merge / create / aggregate` + `ProposalBuilder`) keeps common cases one-line. Verbosity tax paid by the spec, not by users.
- **Merge canonicalization** documented inline: `Proposal::merge(a, b, change)` produces `effect: Change { target: a, change }` with `causal: Pair(a, b)`. The change embeds content sourced from `b`.

#### Proposer shape

- **`Proposer::Request` is an associated type** (§12). GEPA reflective mutation, merge, Meta-Harness, ComBE, MIPRO acquisition, and human edits don't share a request shape; an associated type is the rust-native answer and matches the static-first proposer story already chosen in v0.1. `DynProposer` wraps the request as `Box<dyn Any>` for runtime-loaded plugins.
- **`Arity` reframed as a request hint** (§12). Describes what shape the optimizer should provide as input *when the optimizer drives candidate selection*. Proposers may emit proposals with different causal shapes than declared arity.

#### Context shape

- **`RunContext::apply_batch` and `apply_proposal`** replace `apply(parents, batch)` (§8.2). Per-proposal effects subsume the parents argument.

#### Removed

- **`Parents` enum.** Subsumed by `CausalInputs` (variant names match) plus `ProposalEffect` (which captures the apply target, not the parent).
- **`ProposalBatchSemantics::Ordered`** (§5.7). Multi-batch optimizer rhythm covers ordered-dependency cases. Re-add if a real prototype forces it.
- **`Materializable` from cold core** (§5.1). Moved to standard library as a convenience trait used by default `Materializer` impls. Cold-core `Artifact` stays free of workspace concerns.

#### Renderer/materializer policy

- **Stage-owned renderers/materializers are the default** (§13.4). Most stages should hold them as fields (`pub renderer: R`, `pub materializer: M`). Renderer/materializer registries are deferred until a real erased contract exists.

#### Trait law softening

- **Historical cache note superseded by v0.2.1c.** v0.2.1 kept cache identity on `Artifact`; v0.2.1c splits graph identity from cache identity.

#### Event shapes refreshed

- `ProposalBatchProduced` no longer carries `parent_ids` (§18). New `ProposalRecorded` event carries per-proposal effect kind, causal-inputs summary, and informed_by count. Full provenance via `graph.proposal_batch(id)`.

#### Open questions

- **27.7 Renderer registry vs stage-owned composition** is now answered: stage-owned by default, registry deferred. Removed from open questions.

#### Stress tests re-run

The four pressure tests from v0.2 (cross-branch synthesis, Meta-Harness, workspace lifecycle, multi-agent composite) still pass against v0.2.1 with cleaner code. Worked examples in §22.4 (gskill) and §22.5 (Meta-Harness) updated to use the new types.

### v0.2 (2026-05-06) — post-stress-test refinement

The v0.1 second-pass spec survived the conceptual stress tests. The corrections in this pass are local refinements that emerged when implementations were walked through end-to-end against four pressure tests: cross-branch synthesis, the Meta-Harness paper, the workspace-abstraction case, and the multi-agent-system case.

#### Type-level changes

- **`parents` moved from `ProposalBatch` to `Proposal` (§5.5, §5.7).** Sibling proposals in one batch can have different causal parents (cross-branch synthesis surfaced this). The batch carries `semantics + metadata`; each proposal carries its own `parents`.
- **`Parents::None` added (§5.5).** Brand-new authored artifacts with no causal predecessor — the Meta-Harness pattern. Lineage is bibliographic via `informed_by`, not causal.
- **`Arity::None` added (§12).** Proposers that don't ask for parents at all.

#### Trait surface changes

- **Renderer split into two trait families (§13).**
  - `Renderer<P, T, Target>` — value-returning, for prompt assembly, JSON, debug HTML.
  - `Materializer<P, T>` — side-effecting, for materializing artifacts, lineage, traces into a workspace.
  - Resolves open question 27.1.
- **`PreferenceRelation` is stateless (§14).** Fitted/stateful models (Bradley-Terry, Plackett-Luce) live on `Population` impls instead — concretely `TournamentPopulation`. Updates happen in `observe_assessment`. Resolves open question 27.6.
- **`ParetoFrontier::partition_filter` builder method (§15.2).** Frontiers can declaratively ignore observations from specific case-set partitions (e.g. only update from `SEARCH`, never from `TEST`). Replaces ad-hoc filter logic in optimizer step bodies.

#### Graph query additions

- **`graph.informed_by(c)` and `graph.informed(c)` (§10.2).** Typed graph relation for "candidates this proposer read from during reflection." Promoted from string-keyed `MetadataBag` access. Avoids the python-gepa stringly-typed metadata-parsing failure mode.

#### Trait law tightening

- **`ContentId` collision resistance (§24).** Strengthened from "observational identity" to "MUST be a cryptographic hash of all observationally-relevant state." Hand-rolled impls are a footgun; ship a derive macro for safe-by-default behavior.

#### Documentation additions

- **`16.6 Workspace lifecycle.`** New full section. `WorkspaceFactory`, `WorkspaceBackend`, `Workspace`, ownership table, standard backends (Local, E2B, Docker, K8s, Firecracker, GitWorktree). Agent runtimes are kept separate from workspaces — they take a workspace and run commands in it.
- **Merge canonicalization (§5.5, §20).** `Artifact::apply` only sees one artifact, so for `Parents::Pair(a, b)` the change canonicalizes to one parent and embeds cross-parent content. Spelled out so readers don't expect the framework to magically combine two artifacts.
- **`ProposalBatchSemantics::Alternatives` cost behavior (§5.7).** All alternatives are evaluated independently if applied successfully. Cost is N×eval, not amortized — the framework does not deduplicate.

#### Plan changes

- **Implementation plan reorders prototypes 2 and 3 (§26).** Pairwise tournament now runs before GEPA parity. Pairwise stresses what is *new* in this design (Pairwise eval requests, fitted preference relations, tournament populations) and is therefore the more informative early test.
- **Two coding-agent worked examples added (§22.4, §22.5).**
  - `gskill`: agentic SWE-smith evaluator with workspace materialization and a reflective proposer.
  - Meta-Harness: agentic proposer reading full graph history via `Materializer`, `Parents::None`, `Arity::None`, multi-axis pareto with `partition_filter`.

#### Stress tests passed

The v0.2 surface was verified against:

1. **Cross-branch synthesis** — proposer reads evidence across two branches, emits a fix as a single proposal with one causal parent and many `informed_by` entries; or two sibling proposals with different parents in one batch.
2. **Meta-Harness end-to-end** — agentic harness search, multi-MB execution traces via `EvidenceStore`, fresh artifacts via `Parents::None`, multi-axis pareto with declarative test-partition filtering.
3. **Workspace lifecycle under k8s and git-worktree backends** — pod-shared, per-workspace containers; worktree-per-workspace with git commit identity; agent commits inside the worktree, framework reads HEAD on cleanup.
4. **Composite multi-agent artifact** — four-agents-and-substrate as one artifact; surface-addressed via `EditSurface`; per-part blame attribution via `AttributableEvidence<S::PartId>`. No new primitives required.

20 literature targets from §25 were mentally implemented against this surface. All expressible. The pressure tests surfaced exactly the changes listed above and no others.

### v0.1 second pass (2026-05-05)

Tightened the post-reset design. Major moves: cost as infrastructure not metadata; `ProposalAnnotations` typed vs `MetadataBag` operational; `EvaluationRequest` as a sum type with `Independent / Pairwise / Listwise` variants; `AssessmentGranularity` as an explicit knob; `EvaluatorRegistry` replacing single evaluator; concrete engine shape and run loop; concrete `RunEvent` enum; explicit `CachePolicy` (default `Never`); explicit static-first / dyn-friendly async policy; `EvidenceStore` separating large-evidence persistence from inline graph state.

### v0.1 (2026-05-04, deprecated)

First post-reset draft. Replaced by the second pass.

### v1.0 design lock (2026-05-03, deprecated)

Pre-reset attempt. Six strategy traits, four capability traits, 35+ stdlib impls, multiple coexisting archives, cardinal-only `Score`, capability traits on `Evidence`. Critique surfaced architectural over-engineering; full reset to v0.1.

---

## 31. Future Note: Skill-Library Optimizers

Do not force skill-library evolution into GEPA just because it also mutates
natural-language artifacts. The likely future crate is `leaven-skill` or an
agentic optimizer crate that reuses engine, surface, evidence, population,
workspace, and materializer primitives while owning its own rhythm.

Likely extension slots:

```text
SkillRouter            chooses which existing skills an agent receives for a task
HardCaseSelector       chooses failures or near-misses worth turning into skill updates
SkillTargetSelector    chooses which skill/artifact part should be rewritten or created
SkillAdmissionPolicy   accepts, rolls back, merges, or retires skill changes
SkillLibraryPopulation stores skill versions, utilities, snapshots, and transfer stats
```

These are not cold-core traits. They belong in an optimizer/strategy crate once
a real skill-library implementation needs them. The core lesson to preserve now:
selection policy is swappable, population/archive state is separate, and
workspace materialization is the bridge from typed skill artifacts to the agent
runtime.
