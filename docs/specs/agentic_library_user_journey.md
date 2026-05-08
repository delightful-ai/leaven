# Leaven v0.2.9 - Agentic Library User Journey

> Status: pre-implementation companion spec.  
> Date: 2026-05-08.  
> Governing spec: `docs/specs/initial_library.md`.  
> Runtime companion: `docs/specs/agentic_stage_runtime.md`.  
> Agent workload companion: `docs/specs/agentic_task_execution_substrate.md`.  
> Skill companion: `docs/specs/agentic_skill_optimization_primitives.md`.  
> Purpose: describe the intended end-user experience for Leaven as an agentic
> optimization library, and name the sharp edges that remain user-owned.

This document is not an API lock. It is a product journey spec: what a user
should experience when they use Leaven to evaluate and improve an agentic
artifact.

The guiding rule:

```text
Users define the task and the thing being optimized.
Leaven owns the optimization, execution, evidence, and recovery infrastructure.
Provider adapters own provider-specific runtime mechanics.
```

Leaven should not feel like a bag of traits. It should feel like a reliable
optimization harness whose lower-level traits are available when the defaults
are not enough.

---

## 1. User Types

Leaven should serve five user tiers without forcing everyone into the lowest
level.

### Tier 1: Data-shaped evaluator user

This user has:

- a seed artifact
- cases in JSONL/fixtures
- an obvious scorer such as exact match, test pass/fail, or model judge
- a stock agent presentation pattern

They should not implement traits. They should configure a workload, runtime,
budget, and optimizer recipe.

### Tier 2: Custom scoring user

This user can use stock case loading and presentation, but needs custom domain
scoring.

They implement or provide an `AgentCaseScorer<P>` and still rely on Leaven for
case execution, workspace lifecycle, transcript capture, evidence storage,
checkpointing, and optimizer integration.

### Tier 3: Custom presentation user

This user needs to control how a candidate artifact and case become an agent
workspace.

They implement an `AgentCasePresenter<P>`, but still use stock
`AgentCaseEvaluator<P>`, runtime adapters, case records, retry policy,
checkpointing, and optimizers.

### Tier 4: Custom stage user

This user needs a custom proposer, evaluator, parser, or population strategy.

They implement Leaven stage traits directly, but should still use reusable
workspace, runtime, store, evidence, checkpoint, and population pieces.

### Tier 5: Paper reproduction / optimizer author

This user is reproducing a paper or writing a new optimizer.

They can implement `Optimizer<P>` or specialized strategy slots, but must still
use real artifact, surface, agent-workload, evidence, cache, and checkpoint
primitives when those are generic Leaven concepts.

---

## 2. First Successful Journey

The first good user journey should be:

```text
I have an agent task.
I have a candidate artifact.
I want GEPA or another optimizer to improve that artifact.
I can get one real optimization run with durable evidence and resume support
without writing my own optimization harness.
```

For a skill-learning task, that should look conceptually like:

```rust
let artifact = SkillBank::from_dir("skills")?;

let workload = AgentWorkload::builder()
    .cases(CaseSuite::from_jsonl("cases/train.jsonl")?)
    .validation_cases(CaseSuite::from_jsonl("cases/valid.jsonl")?)
    .presentation(AgentPresentationPreset::repo_task())
    .scorer(ScorerPreset::exact_match("answer"))
    .limits(AgentCaseLimits::default())
    .build()?;

let evaluator = AgentCaseEvaluator::builder(workload)
    .runtime(codex_runtime)
    .candidate_materializer(SkillBankNativeLayout::codex())
    .build()?;

let proposer = AgentAuthoredProposer::builder()
    .runtime(codex_runtime)
    .presenter(SkillMutationPresentation::default())
    .parser(SkillBankProposalParser::default())
    .repair_policy(ProposalRepairPolicy::attempts(3)?)
    .build()?;

let gepa = Gepa::builder(problem)
    .surface(SkillBankSurface::default())
    .evaluator(evaluator)
    .proposer(proposer)
    .population(ParetoFrontier::default())
    .build()?;

let result = Engine::builder(problem)
    .optimizer(gepa)
    .seed_artifact(artifact)
    .store(LocalStore::open(".leaven/run")?)
    .budget(Budget::iterations(20).or_cost(Cost::usd(50)?))
    .run()
    .await?;
```

The final implementation may offer convenience constructors over this, but the
conceptual ownership should stay visible:

```text
GEPA is the optimizer.
SkillBank is the artifact.
AgentWorkload is how cases run.
Codex is a stage runtime dependency.
Leaven Engine records graph truth.
```

No public API should imply that Codex, skills, or GEPA are the universal root
of the library.

---

## 3. Preflight Experience

Before spending money on agent runs, users should be able to run a preflight:

```rust
let report = AgentRunPreflight::new()
    .artifact(&artifact)
    .workload(&workload)
    .runtime(&codex_runtime)
    .store(&store)
    .check()
    .await?;
```

Preflight should check:

- artifact parses and validates
- edit surface can enumerate parts
- cases parse and have stable ids
- partitions are non-empty where required
- hidden targets are not candidate-visible
- materializer can write a representative candidate/case workspace
- runtime is reachable enough to report a fingerprint
- output contract paths are valid workspace paths
- scorer can handle a fixture or dry output shape
- store can write graph, artifact, evidence, checkpoint, and blob records
- budget policy is valid
- cache identity can be computed or caching is explicitly disabled

Preflight should not call a paid model by default. If a runtime smoke requires
money or network, it must be explicit.

Preflight output should be actionable:

```text
OK: SkillBank valid, 6 skills, 42 files.
OK: 120 train cases, 40 validation cases, stable ids.
OK: Codex runtime fingerprint resolved.
WARN: skill-use telemetry will be inferred from transcript, not observed.
ERROR: hidden target field would be written by selected presenter.
```

---

## 4. Run-Time Experience

During a run, the user should see progress in the language of their task, not
only internal graph terms.

Useful live status:

```text
iteration 4/20
parent candidate c17 selected from validation frontier
proposer attempt 1 failed SkillBank validation: missing SKILL.md in "csv-cleanup"
proposer attempt 2 accepted proposal p31
evaluating candidate c22 on 8 train cases
6 pass, 2 fail, cost $1.73
validation frontier updated: c22 improves 5/40 validation cases
checkpoint written: .leaven/run/checkpoints/...
```

The user should not need to know how graph internals work to understand what
is happening. They should be able to inspect:

- current best candidate
- candidate lineage
- changed artifact files
- proposal attempts and repair feedback
- case run records
- scorer explanations
- provider transcript refs
- skill/tool use evidence when available
- cost and budget ledger
- cache hits/misses

---

## 5. Result Experience

At the end of a run, the user should receive a result object and durable files
that answer:

```text
What is the best candidate?
Why was it selected?
Which cases improved or regressed?
What changed in the artifact?
What did the agent do during evaluation?
What did the proposer try and repair?
How much did this cost?
Can I resume or reproduce this?
```

Result shape:

```rust
pub struct AgenticRunReport<P>
where
    P: OptimizationProblem,
{
    pub run_id: RunId,
    pub best_candidate: CandidateId,
    pub best_artifact: ArtifactRef<P::Artifact>,
    pub frontier: FrontierReport,
    pub lineage: LineageReport,
    pub evaluations: EvaluationSummary,
    pub costs: CostSummary,
    pub checkpoints: Vec<CheckpointRef>,
    pub warnings: Vec<RunWarning>,
}
```

For skill learning, a domain report can add:

```text
skills added
skills renamed
skills rewritten
descriptions changed
skill-use telemetry summary
failure clusters attributed to skills
```

Those are report views over artifacts and evidence, not separate graph truth.

---

## 6. Resume Journey

Resume must be a normal path, not disaster recovery magic.

```rust
let result = Engine::resume(store, run_id)
    .with_budget(Budget::additional_iterations(10))
    .run()
    .await?;
```

The user should not have to manually rebuild sampler state, population state,
repair counters, or cached evaluation indexes.

Resume should:

- restore graph truth
- restore optimizer/population private state or fail clearly
- restore case sampler positions
- reuse completed case run records only when identities match
- not charge budget twice
- abandon or janitor in-flight workspaces
- preserve failed proposals and repair attempts
- preserve cache invalidation reasons

If resume cannot proceed, the error should say which invariant failed:

```text
Cannot resume run r7:
population state schema fingerprint does not match optimizer fingerprint.
Use --restart-from-graph to rebuild graph-derived state or choose a compatible
optimizer version.
```

---

## 7. Paper Reproduction Journey

A paper reproduction should feel like a normal advanced Leaven user, not a
forked framework.

The paper-specific crate owns:

- dataset download/loading
- exact train/validation/test splits
- paper prompts
- paper scorers/judges
- algorithm-specific thresholds and schedules
- ablation commands

The paper-specific crate must reuse:

- real artifact types
- real edit surfaces
- real `AgentWorkload` / `AgentCaseEvaluator`
- real proposer repair
- real population/frontier primitives when generic
- real evidence
- real cache identity
- real checkpoint/resume

If reproduction code has to invent a fake skill type, fake task runner, fake
graph, fake evidence, or fake checkpoint, that is a Leaven substrate gap.

---

## 8. What Leaven Should Hide

Leaven should hide or standardize:

- graph candidate/proposal ids for the common path
- proposal admission details
- workspace allocation and cleanup
- provider session capture
- raw transcript storage
- durable case run records
- retry and score-on-error bookkeeping
- repair attempt bookkeeping
- budget charging
- cache key construction
- checkpoint envelopes
- population/frontier update plumbing
- artifact blob persistence
- common case loaders and partitioning

Users should still be able to inspect all of this, but they should not have to
author it to run an ordinary agentic optimization.

---

## 9. Sharp Edges We Still Leave Sharp

Some edges should remain user-owned because smoothing them would mean lying.

### 9.1 Task semantics

Leaven cannot know what success means for an arbitrary agent task.

Stock scorers can cover exact match, regex, JSON field match, unit tests,
command exit status, model judge, and pairwise preference. Domain correctness,
rubrics, hidden labels, and failure taxonomy remain user/paper code.

### 9.2 Artifact truth

Leaven can provide stock artifacts such as `SkillBank`, `AgentKit`, prompt
maps, and git-backed code artifacts. If a user optimizes a new kind of object,
they must define the artifact and surface truth.

This is sharp by design: pretending every agent artifact is just text patches
would destroy semantic information.

### 9.3 Presentation choices

How a candidate and case should be presented to an agent is often part of the
experiment. Stock presets should cover common layouts, but custom workspaces,
hidden data rules, harness commands, and multi-agent setup remain sharp.

### 9.4 Provider behavior

Every provider has different skill loading, transcript shape, tool events,
approval controls, context compaction, and failure modes. Leaven normalizes what
it can and preserves raw evidence. It cannot make provider telemetry complete.

Missing skill-use events mean unknown, not false.

### 9.5 Non-determinism

Agent runs are often nondeterministic. Leaven can fingerprint runtime config,
record seeds where available, cache only when identities make it sound, and
preserve transcripts. It cannot promise bit-for-bit reproducibility for
providers that do not provide deterministic execution.

### 9.6 Security and tools

Tool approval, network access, sandbox policy, credentials, and data visibility
are user/operator choices. Leaven should make the policy explicit and record
decisions, but it should not silently grant broad tools to make demos easier.

### 9.7 Cost and latency

Leaven can budget, meter, cache, resume, and stop. It cannot make expensive
agent tasks cheap. Users must choose case counts, validation frequency,
provider, and budget policy.

### 9.8 Data quality

Optimization is only as good as the cases and scorer. Leaven can detect obvious
case-format issues, leakage through presentation, unstable ids, and invalid
targets. It cannot know that the distribution is representative.

### 9.9 Proposal semantics

Leaven can ensure proposed artifacts are structurally valid before graph
admission. It cannot guarantee that a valid mutation is useful. Bad ideas are
evidence for the optimizer, not validation errors.

### 9.10 Paper faithfulness

Leaven can expose the primitives needed to reproduce papers. The reproduction
crate still owns exact prompts, splits, metrics, baselines, and ablations.

---

## 10. UX Definition of Done

The user journey is acceptable when these are true:

- a Tier 1 user can run a data-shaped agent evaluation without implementing a
  trait
- a GEPA user can optimize `SkillBank` through normal `Gepa<P, S>` composition
  without a fake skill optimizer facade
- a custom task user implements only scoring or presentation, not graph
  infrastructure
- every run emits durable case run records and final reports
- invalid proposals repair before graph admission or fail visibly
- resume works as a normal command path
- costs and abandoned workspaces are visible
- provider-specific telemetry loss is explicit
- paper reproduction crates can reuse generic Leaven primitives rather than
  locally rebuilding the substrate

---

## 11. Short Form

```text
Common user journey:
  artifact + cases + runtime + scorer + budget
  -> preflight
  -> GEPA/custom optimizer run
  -> durable evidence and best candidate
  -> resume/reproduce

Leaven hides:
  graph plumbing, workspace lifecycle, runtime capture, case records,
  retries, repair, cache, checkpoint, budget ledger, population updates.

Leaven leaves sharp:
  task semantics, artifact semantics, presentation, provider quirks,
  security, cost, data quality, nondeterminism, paper faithfulness.
```
