# Layer 1 Original-Vision Refinement

Short answer: **No, Layer 1 is not aligned with the original Leaven vision.**
The first-pass audit is directionally right and catches several blockers, but it
still lets the current public surface look closer to the vision than it is. The
original promise was not merely "the AIME example runs" or "GEPA-shaped pieces
exist." It was a short, ordinary path for optimizing an LM/program/agent over
real work with scoring, feedback, traces, budgets, caching, held-out data, and an
honest report, while keeping engine machinery out of the user's head.

The minimum ordinary surface I would require before calling Layer 1 real is:

```text
optimize(seed artifact/program)
  + work input: single task, train cases, optional validation/test cases, or a
    domain task suite that lowers to stable case ids and split roles
  + execution: optional async runner/CandidateRunner when score does not execute
    the candidate itself
  + scoring: async score function or evaluator returning Score/Metered<Score>,
    with primary comparable score, metrics, natural-language feedback, traces,
    attachments, and explicit errors
  + optimizer: Gepa::default() or a visible optimizer value
  + LM/runtime roles: solver/reflector/model-judge runtimes with cache/cost policy
  + budget: explicit budget or explicit unlimited budget
  + storage: evidence store and optional durable resume store
  + result: best candidate, stop reason, split/case report, cost/cache summary,
    public events, GEPA summary, and evidence refs without requiring RunGraph
```

## Findings

### L1-OV-001: The first-pass answer is right, but the denominator is too narrow

- `id`: `L1-OV-001`
- `severity`: `high`
- `vision promise`: Leaven is "Optimize anything in Rust": a Rust library for
  optimizers over arbitrary artifacts whose behavior can be assessed, not a
  GEPA-only engine or an AIME demo harness.
- `current audit coverage`: The audit answers "No" for ordinary users and names
  the right shell: `optimize(seed).train(...).validation(...).test(...).runner(...).score(...).using(...).run()`.
- `gap`: The audit mostly evaluates whether today's GEPA/AIME-shaped public path
  is honest. It does not make the original broader promise the denominator:
  arbitrary artifacts, LM programs, agentic work, non-dataset tasks, recoverable
  runs, and optimizer-family neutrality. That leaves room for a future fix to
  make AIME less fake while still missing the library-product promise.
- `correction`: Treat the Layer 1 acceptance bar as "ordinary user can run a real
  optimizer over a real LM/agent/program workload," with AIME only one proof. The
  integrated docs should state the minimum public surface explicitly and require
  examples to prove that surface rather than a proxy.
- `evidence`:
  - `docs/specs/initial_library.md:404-443` defines the broad optimizer-library
    goal and says GEPA is one optimizer value, not the engine.
  - `docs/specs/initial_library.md:453-468` promises end users a short path and
    says they should not understand every internal trait.
  - `docs/specs/gepa_optimizer_surface.md:38-52` says current GEPA-shaped
    primitives are not yet an off-the-shelf optimizer surface.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/agent-report-layer-1.md:24-32`
    correctly concludes Layer 1 is not yet an honest off-the-shelf optimizer
    surface.

### L1-OV-002: The agent report dropped the single-task/no-dataset promise

- `id`: `L1-OV-002`
- `severity`: `blocker`
- `vision promise`: Ordinary users can optimize one task or environment without
  faking train/validation/test data. Datasets are optional; cases are units of
  work, not necessarily labeled examples.
- `current audit coverage`: `public-api-ledger.md` has a finding for missing
  single-task search, but the main agent report's consolidated findings omit it.
- `gap`: This is not a minor ergonomic miss. For benchmarks, interactive
  environments, live human evals, pairwise online tournaments, and many agentic
  tasks, the natural Layer 1 input is not a dataset split. Today's builder fixes
  the case type only through `.train(...)`, rejects held-out data without train,
  and has no `.task(...)`, `.cases(...)`, `.environment(...)`, or unscoped mode.
- `correction`: Promote the single-task/no-dataset issue into the integrated
  Layer 1 report as a blocker. The public surface must support either an explicit
  single-task mode or a first-class unscoped work input, while keeping workspace
  and environment ownership in adapter crates.
- `evidence`:
  - `docs/specs/gepa_public_private_surface.md:101-110` says single-task search
    should feel as native as train/validation/test search.
  - `docs/specs/gepa_public_private_surface.md:727-730` defines mode inference:
    no train/validation/test means single-task.
  - `docs/specs/eval_lowering_detail.md:230-242` says datasets are optional and
    agentic case suites keep workspace requirements outside `leaven-eval`.
  - `docs/specs/initial_library.md:1092-1108` includes single-task and
    `EvaluationSet::Unscoped`.
  - `crates/leaven-run/src/builder.rs:92-114` only fixes the case type through
    `.train(...)`.
  - `crates/leaven-run/src/builder.rs:207-216` rejects held-out data without
    train and immediately builds a dataset from concatenated split vectors.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/public-api-ledger.md:29-45`
    records this finding, but `agent-report-layer-1.md:73-555` does not carry it
    into the main findings list.

### L1-OV-003: Runner/scorer sync-ness is a symptom of a missing execution contract

- `id`: `L1-OV-003`
- `severity`: `blocker`
- `vision promise`: Leaven is async by default. Users can run deterministic
  functions, LM judges, subprocess runners, agentic sandboxes, compiler/profiler
  harnesses, human review, pairwise judges, and listwise rankers through the
  ordinary surface.
- `current audit coverage`: The audit correctly flags sync-only runner and scorer
  callbacks as a blocker.
- `gap`: The report states the callback problem, but it should frame the fix as a
  full candidate-execution and scoring contract, not just "make closures async."
  The public contract must carry runner output, trace, attachments, errors,
  scorer cost, and score-on-error policy in one path.
- `correction`: The integrated docs should hard-cut `leaven-run` to
  `CandidateRunner` plus `Scorer` as the canonical lowering path. Simpler scalar
  and sync closures may exist only as adapters into that path, not as the semantic
  product contract.
- `evidence`:
  - `docs/specs/initial_library.md:520-527` calls out async-by-default and
    ergonomic builders as core design expectations.
  - `docs/specs/initial_library.md:2128-2165` defines async evaluators and lists
    LM judges, subprocess runners, agentic sandboxes, compilers, pairwise judges,
    and rankers.
  - `docs/specs/gepa_public_private_surface.md:987-1039` defines async
    `CandidateRunner` and async `Scorer` returning result-bearing values.
  - `docs/specs/gepa_public_private_surface.md:1070-1081` requires metered
    scoring cost and says errors are not scores.
  - `crates/leaven-run/src/builder.rs:28-29` defines runner/scorer as synchronous
    `Fn` callbacks.
  - `crates/leaven-run/src/evaluator.rs:97-128` calls the sync runner/scorer,
    flattens the result, and charges one metric call per case.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/agent-report-layer-1.md:75-131`
    identifies the sync-only public runner/scorer blocker.

### L1-OV-004: Score, reward, feedback, and trace vocabulary needs one public story

- `id`: `L1-OV-004`
- `severity`: `high`
- `vision promise`: The ordinary public concept is a scoring function. It can
  consume fixed answers, hidden verifier targets, LM judges, human judgments,
  environment reward signals, open-ended task scoring, traces, and history, then
  return comparable score axes plus feedback evidence.
- `current audit coverage`: The audit correctly says `RunOutput`, `Score`, and
  `ScoreContext` are too thin for real feedback evidence.
- `gap`: The audit does not explicitly resolve the user-facing naming issue:
  "reward" should be a possible score source, not the Layer 1 public verb. The
  public verb and type should stay `.score(...)` / `Score`; reward signals,
  metrics, exact match, model judges, and natural-language critique all normalize
  into `Score`.
- `correction`: Update integrated docs to state: use "score" for the public API,
  "reward signal" only as an input/source kind. `ScoreContext` must be an
  accessor view over candidate, optional case, output, run error, trace, bounded
  history, and budget. `Score` must carry primary comparable score, metrics,
  natural-language feedback, structured feedback, attachments, and metadata.
- `evidence`:
  - `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:129-143`
    asks how users score programs and give natural-language feedback.
  - `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:147-157`
    stress-tests reward contents and asks for the reward function contract.
  - `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:161-170`
    asks whether score or reward is the better public word.
  - `docs/specs/gepa_public_private_surface.md:894-913` says the ordinary public
    concept is a scoring function and shows rich feedback.
  - `docs/specs/gepa_public_private_surface.md:930-985` defines `ScoreContext` as
    a typed view and bans `RunGraph`, `Actor`, `ReadScope`, and `TrustPolicy` from
    that view.
  - `docs/specs/gepa_public_private_surface.md:1115-1182` defines rich `Score`
    contents and says environment reward signals are score sources, not dataset
    requirements.
  - `crates/leaven-run/src/evidence.rs:3-54` currently exposes `RunOutput` as a
    string plus trace strings, `Score` as `f64 + String + Vec<(String, String)>`,
    and `ScoreContext` as public fields.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/evaluation-datasets-results.md:10-31`
    records the thin score/output finding.

### L1-OV-005: Train/validation/test is present, but not user-intuitive enough

- `id`: `L1-OV-005`
- `severity`: `high`
- `vision promise`: Users think in train/search work, held-out validation, and
  final test. Datasets, case IDs, split policies, hidden targets, and trust
  lowering exist, but they should be explained after the ordinary intent.
- `current audit coverage`: The audit correctly flags positional case IDs and
  lost dataset identity.
- `gap`: The audit understates the larger divergence: Layer 1 is still not
  teaching train/validation/test as a user-intuitive contract. It catches case ID
  loss, but it should also require duplicate rejection, split-role semantics,
  final-test-only defaults, hidden target handling, and report statements about
  whether test evidence affected the run.
- `correction`: Refine Layer 1 docs around `Case` / `CaseSuite` rather than raw
  split vectors. Plain vectors can be a dense-id convenience, but the real public
  examples should show stable IDs and optional targets. The report must mark
  train/validation/test use by intent: in-loop feedback, held-out model
  selection, final-report-only.
- `evidence`:
  - `docs/specs/gepa_public_private_surface.md:124-136` frames generalization in
    ordinary ML words.
  - `docs/specs/gepa_public_private_surface.md:650-674` defines builder behavior
    and required pre-run rejections for duplicate IDs, split overlap, and
    invalid held-out use.
  - `docs/specs/gepa_public_private_surface.md:776-820` defines the minimum
    public case shape and split semantics.
  - `docs/specs/eval_lowering_detail.md:650-679` requires product builders to
    construct dataset/splits once, reject duplicate IDs, default to disjoint
    splits, and lower split-use intent into trust policy.
  - `docs/specs/eval_lowering_detail.md:780-809` requires reports to cite graph
    refs and state final-test-only use.
  - `crates/leaven-run/src/builder.rs:214-216` builds a dataset from concatenated
    split vectors.
  - `crates/leaven-run/src/builder.rs:302-355` generates dense positional case
    IDs and split membership.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/agent-report-layer-1.md:385-440`
    identifies positional case IDs and source identity loss.

### L1-OV-006: Environments are represented, but only as something Layer 1 should not own

- `id`: `L1-OV-006`
- `severity`: `medium`
- `vision promise`: Users can optimize agentic and environment-shaped tasks, but
  `leaven-run` must not grow a hidden environment abstraction. Domain adapters own
  workspace/agent/process semantics and lower case IDs/split roles into the eval
  substrate.
- `current audit coverage`: The Layer 1 audit mentions LM/agent/programs and
  workspace/subprocess needs, but it does not separately audit whether
  environments are presented in user-intuitive terms.
- `gap`: Without this refinement, a fix could add `.environment(...)` directly to
  `leaven-run` and violate the topology, or avoid environments entirely and fail
  the original user promise. The right shape is "runner/domain adapter captures
  environment handles" plus "agentic adapters preserve hidden targets and
  workspace evidence."
- `correction`: Add a Layer 1 wording rule: public docs may say "task suite" or
  "environment-backed runner," but `leaven-run` owns only runner/scorer/case
  lowering. Agentic/environment details live in `leaven-agentic`, domain crates,
  and workspace/agent crates.
- `evidence`:
  - `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:31-47`
    explicitly separates evals, datasets, and environments.
  - `docs/specs/eval_lowering_detail.md:29-32` separates user input, lowered eval
    data, execution, and environment.
  - `docs/specs/eval_lowering_detail.md:47-62` says agentic task/environment
    semantics are domain/runtime-shaped and not owned by `leaven-eval`.
  - `docs/specs/eval_lowering_detail.md:709-720` defines agentic adapter behavior:
    keep hidden targets/workspace requirements in `leaven-agentic`, run through
    workspace/agent crates, and record transcript evidence.
  - `docs/specs/gepa_public_private_surface.md:1023-1027` says generic runners
    capture environment handles and `leaven-run` must not grow a hidden
    environment abstraction.
  - `docs/specs/gepa_optimizer_surface.md:153-169` assigns ownership so
    `leaven-run` owns public builder/lowering while domain/runtime internals stay
    elsewhere.

### L1-OV-007: Reflection is currently a false-positive public name

- `id`: `L1-OV-007`
- `severity`: `blocker`
- `vision promise`: GEPA reflection consumes selected trace/evidence/feedback and
  proposes edits through a real reflector/proposer. A mock LM is acceptable for
  proof; a pre-authored fixed edit is not reflection.
- `current audit coverage`: The audit correctly flags the fixed edit fixture and
  the live AIME bypass.
- `gap`: The current public names are worse than "not implemented": they teach a
  false mental model. `ReflectiveMutation::new(edit)` sounds like GEPA reflection
  but returns one stored edit and ignores artifact, surface, part, score
  feedback, traces, history, and budget.
- `correction`: Move/rename the fixed fixture out of Layer 1. Acceptable names:
  `FixedEditProposer` or `DeterministicEditFixture`, confined to tests/examples
  or an explicit testing module. Reserve `ReflectiveMutation` for the real
  evidence-aware reflector with an LM/agent-backed constructor such as
  `with_lm(...)`. Remove or hide placeholder `ReflectiveMutationConfig` and
  `SystemAwareMerge` from ordinary docs until implemented.
- `evidence`:
  - `docs/specs/gepa_optimizer_surface.md:322-357` describes the ordinary
    reflective mutation iteration, including feedback assessment IDs and
    `informed_by` provenance.
  - `docs/specs/gepa_optimizer_surface.md:463-486` lists reflection inputs:
    selected part, casewise evidence, attribution, objective/background,
    transcripts, validation/apply errors, and candidate summaries.
  - `crates/leaven-gepa/src/proposer.rs:21-56` implements
    `ReflectiveMutation` as a deterministic fixture and exposes placeholder
    names.
  - `examples/p8_aime_gepa/src/main.rs:75-99` wires
    `ReflectiveMutation::new(AimePromptEdit::ReplaceSystem(...))` into the public
    AIME run.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/examples-and-end-to-end-proof.md:11-29`
    records that the AIME example proves a fixed edit, not GEPA reflection.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/agent-report-layer-1.md:133-203`
    gives the broader fixed-reflection finding.

### L1-OV-008: Cache/runtime semantics are split across low-level wrappers and run bypasses

- `id`: `L1-OV-008`
- `severity`: `high`
- `vision promise`: LM calls, reflection calls, model-judge scoring, and
  evaluation cache use should have coherent runtime/cache/cost semantics. Users
  configure runtime roles and policy; lower-level cache stores remain swappable.
- `current audit coverage`: The audit correctly says the cache is taught as
  `CachedLm` wrapper composition and that `leaven-run` disables evaluation cache.
- `gap`: The audit should explicitly separate three things ordinary users need to
  understand: response cache for LM completions, evaluation cache for scored
  candidate assessments, and budget/cost charging across solver/scorer/reflector
  roles. The current docs/code expose pieces without an ordinary role-based
  runtime story.
- `correction`: Refine specs to add a Layer 1 runtime role surface, for example
  solver LM, reflector LM, and judge LM with cache policy. Keep `CachedLm`,
  `LmCacheStore`, `LmCacheKey`, and `InMemoryLmCache` in advanced/cache docs, not
  ordinary `leaven::prelude::*` examples. The public run report should summarize
  cache hits/misses and cost by role.
- `evidence`:
  - `docs/specs/lm_runtime_and_response_cache.md:15-31` teaches manual
    `OpenAiLm::from_env(...)` plus `CachedLm::read_write(...)`.
  - `docs/specs/lm_runtime_and_response_cache.md:54-57` distinguishes LM response
    cache from engine evaluation cache.
  - `docs/specs/lm_runtime_and_response_cache.md:154-207` defines low-level
    response-cache policies, stores, wrapper, and key ingredients.
  - `crates/leaven-lm-cache/src/cached.rs:6-17` exposes `CachedLm` as a wrapper
    with inner/cache/policy.
  - `crates/leaven-lm-cache/src/lib.rs:15-19` re-exports cache internals in the
    cache prelude.
  - `crates/leaven-run/src/evaluator.rs:61-63` returns `CachePolicy::Never`.
  - `examples/p8_aime_gepa/src/main.rs:271-301` shells out to Python for live
    OpenAI calls.
  - `examples/p8_aime_gepa/scripts/openai_solver.py:24-45` calls the OpenAI
    Responses API directly.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/agent-report-layer-1.md:258-320`
    records the cache-wrapper/run-wiring finding.

### L1-OV-009: The result facade still hides run-time truth ordinary users need

- `id`: `L1-OV-009`
- `severity`: `high`
- `vision promise`: The ordinary completed-run handle tells users what won, why
  it won, what improved or regressed by split/case, what stopped the run, what it
  cost, what was cached, what evidence/attachments exist, and whether resume or
  reproduction is possible, without requiring `RunGraph`.
- `current audit coverage`: The audit correctly says `OptimizeResult` and
  `OptimizationReport` are thin snapshots.
- `gap`: The audit understates resume/persistence and stop semantics. The current
  builder has `.store(...)` but no `.resume(...)`, and the current result has a
  mandatory `best` instead of optional best plus stop reason. Missing evidence is
  also collapsed to numeric zero in report construction.
- `correction`: Update the integrated Layer 1 docs to require `Optimized` /
  `OptimizeResult` to expose optional best, `StopReason`, graph-backed report,
  public event summaries, GEPA summary, cost/cache summary, evidence refs, and
  resume status. Missing or failed scores must be represented as absent/error, not
  `0.0`.
- `evidence`:
  - `docs/specs/gepa_public_private_surface.md:310-311` maps persistence and
    report to ordinary APIs.
  - `docs/specs/gepa_public_private_surface.md:658-661` specifies `.store(...)`,
    `.resume(...)`, and `.run()` behavior.
  - `docs/specs/gepa_public_private_surface.md:1184-1228` defines the result
    contract, including optional best, stop reason, graph-backed report,
    final-test semantics, and no `RunGraph` requirement.
  - `crates/leaven-run/src/result.rs:6-18` requires `best` and clones best/seed
    artifacts into the result.
  - `crates/leaven-run/src/result.rs:35-71` exposes aggregate score fields and
    returns `0.0` for empty averages.
  - `crates/leaven-run/src/builder.rs:439-490` uses `.unwrap_or(0.0)` for missing
    train averages and collects events as strings.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/evaluation-datasets-results.md:51-63`
    separately catches missing evidence reported as zero.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/agent-report-layer-1.md:498-555`
    records the thin result facade issue.

### L1-OV-010: The ordinary import surface violates the three-layer design

- `id`: `L1-OV-010`
- `severity`: `high`
- `vision promise`: Ordinary users should touch seed/program, train or task work,
  optional validation/test, runner, score/evaluator, GEPA, budget, and result.
  GEPA customizers touch selectors/reflectors/frontiers. Optimizer authors touch
  `Optimizer`, `RunContext`, evaluation requests, graph views, and the full
  substrate.
- `current audit coverage`: The audit correctly flags `leaven::prelude::*`
  exporting engine internals next to Layer 1 names.
- `gap`: The correction should be stronger and name the public concepts to remove
  from Layer 1 docs/imports. The problem is not that the types exist publicly; it
  is that ordinary imports teach engine-author and customizer concepts as the
  default product surface.
- `correction`: Split imports and docs into ordinary, GEPA customizer, and engine
  author surfaces. Remove or move these names out of ordinary Layer 1 examples and
  `leaven::prelude::*`: `RunGraph`, `RunGraphView`, `RunContext`, `TrustPolicy`,
  `EvaluationRequest`, `EvaluationSet`, `Assessment`, `AssessmentGranularity`,
  `Population`, `ParentSelector`, `PartSelector`, `Proposer`, `Evaluator`,
  `Renderer`, `Materializer`, engine `CachePolicy`, cache store/key types,
  `CachedLm`, `InMemoryLmCache`, `LmCacheEntry`, `LmCacheKey`, and
  `LmCacheStore`. Keep them available through explicit advanced, engine, GEPA, or
  cache preludes.
- `evidence`:
  - `docs/specs/gepa_public_private_surface.md:49-83` defines the three layers and
    says ordinary Layer 1 users should not touch engine/private names.
  - `docs/specs/gepa_public_private_surface.md:172-208` reserves GEPA strategy
    slots for Layer 2 customizers.
  - `docs/specs/gepa_public_private_surface.md:210-227` reserves `Optimizer` /
    `RunContext` style machinery for Layer 3 optimizer authors.
  - `docs/specs/gepa_public_private_surface.md:472-502` defines the
    ordinary-public filter and says internal-lowered names may still be public
    Rust APIs but are not the default story.
  - `crates/leaven/src/prelude.rs:3-25` re-exports core, engine, LM, run, and
    surface internals together.
  - `crates/leaven/src/prelude.rs:48-49` re-exports the LM cache prelude when the
    `lm-cache` feature is enabled.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/public-api-ledger.md:10-27`
    records the ordinary prelude leak.
  - `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/agent-report-layer-1.md:442-496`
    carries the prelude finding into the main report.

## Refinement Edits Recommended

1. Update
   `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/agent-report-layer-1.md`
   to add the missing single-task/no-dataset blocker, environment/domain-adapter
   refinement, score-vs-reward naming correction, and a stronger explicit list of
   names that must move out of the ordinary surface.

2. Update
   `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/public-api-ledger.md`
   to separate ordinary, GEPA customizer, engine-author, LM-runtime, and
   cache-store names. Keep the existing prelude finding, but make the removal/move
   list exact.

3. Update
   `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/evaluation-datasets-results.md`
   to widen the dataset finding into the full train/validation/test contract:
   stable case IDs, optional targets, single-task/no-dataset mode, final-test-only
   defaults, report visibility, absent-score semantics, and score-vs-reward
   vocabulary.

4. Update
   `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/examples-and-end-to-end-proof.md`
   to require the product proof to exercise the minimum ordinary surface: async
   runner/scorer, rich score, stable cases, mock LM/agent reflection consuming
   feedback, Leaven LM/runtime/cache roles, and graph-backed result reporting.

5. Update
   `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-1-user/README.md`
   so the seed questions include single-task mode, environment-backed runners,
   score-vs-reward naming, result/resume semantics, and public-name quarantine.

6. Update the cross-cutting LM/cache refinement report, if/when integrated, to
   distinguish response cache, evaluation cache, runtime roles, and budget/cost
   accounting instead of treating `CachedLm` wrapper ergonomics as the whole
   Layer 1 cache story.

7. Update the GEPA customizer refinement report, if/when integrated, to reserve
   `ReflectiveMutation` for real reflection and move fixed edit fixtures out of
   production-looking public API.
