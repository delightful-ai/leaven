# GEPA Reference Behavior And Leaven Parity

Status: implementation reference spec and parity audit target.
Date: 2026-05-16.

This document is the durable Leaven source of truth for what "real GEPA" means.
Benchmark-specific specs, including `gepa_aime_paper_parity.md`, must defer to
this document for GEPA loop semantics, defaults, cache behavior, frontier
behavior, and result reporting.

This document is not a compatibility plan for Python APIs. Leaven remains
Rust-native: typed artifacts, typed edit surfaces, durable graph truth,
target-safe evidence projection, provider-neutral LM roles, and explicit run
stores. Parity means matching the reference algorithm's observable decisions and
accounting, not copying Python's `dict[str, str]` API.

Verified upstream snapshots:

- `gepa-ai/gepa` main at `ff60b615f2c99044a81d626717f56d80e93ce60d`
- `stanfordnlp/dspy` main at `99427f8e2525f16168cfea02cb9938671bbcae9d`

Primary upstream anchors:

- `src/gepa/core/engine.py`: seed validation, main loop, accepted-candidate
  validation, callbacks, stop behavior.
- `src/gepa/core/state.py`: `GEPAState`, per-candidate validation subscores,
  Pareto frontier maps, per-candidate/example `EvaluationCache`.
- `src/gepa/proposer/reflective_mutation/reflective_mutation.py`: candidate
  selection, train minibatch sampling, trace capture, reflective dataset build,
  proposal, child screening, skip-perfect behavior.
- `src/gepa/strategies/candidate_selector.py` and `src/gepa/gepa_utils.py`:
  Pareto-front weighted candidate selection.
- `src/gepa/strategies/eval_policy.py`: default full validation policy.
- `src/gepa/api.py`: core `gepa.optimize(...)` defaults.
- `src/gepa/optimize_anything.py`: optimize-anything profiles and ASI adapter.
- `dspy/teleprompt/gepa/gepa.py` and `dspy/teleprompt/gepa/gepa_utils.py`:
  DSPy wrapper defaults and DSPy trace/feedback adapter.

## 1. What Counts As Real GEPA

Real GEPA is the reflective evolutionary optimizer described by the paper and
implemented in the upstream GEPA package:

1. Start from a seed candidate containing one or more mutable text components.
2. Evaluate the seed on the validation/Pareto set before the optimization loop.
3. Track per-validation-item best scores and candidate membership on a Pareto
   frontier.
4. Repeatedly select a parent candidate from that validation Pareto frontier.
5. Sample a feedback minibatch from the train/search set.
6. Evaluate the parent on that minibatch with trajectories or side information.
7. Reflect on the selected component's current text and observed feedback.
8. Propose replacement text for the selected component.
9. Evaluate the child on the same minibatch.
10. Accept the child only when the acceptance criterion passes on the minibatch.
11. Full-evaluate accepted children on the validation/Pareto set.
12. Update candidate lineage, validation subscores, Pareto frontier maps, and
    discovery/budget counters.
13. Stop at the configured budget or stopper boundary.
14. Return the candidate with the best validation aggregate under the validation
    evaluation policy.

Any Leaven path that skips seed validation, selects parents from train-only
scores, does not update a validation Pareto frontier for accepted candidates, or
returns the train-frontier best as the ordinary GEPA result is not real GEPA.

## 2. Terminology Mapping

| Reference GEPA | Leaven term |
| --- | --- |
| candidate/program | `CandidateId` plus typed `P::Artifact` |
| component/parameter | `EditSurface` part |
| text replacement | surface edit lowered to artifact-native `Change` |
| trainset / feedback set | train/search partition |
| valset / Pareto set | validation/Pareto partition |
| testset | final-report partition only |
| rollout / metric call | one candidate evaluation on one case |
| trajectory | runner output, scorer feedback, trace/evidence refs, and optional stage transcript |
| ASI / side info | scorer/evaluator-produced optimizer-visible feedback |
| reflective dataset | `ReflectRequest.examples` plus provenance refs |
| proposer | `GepaReflector` over LM-backed, agent-backed, or custom proposal backend |
| state | graph truth plus GEPA private continuation |
| evaluation cache | deterministic candidate/case/evaluator assessment cache |
| LM cache | provider-neutral LM response cache |

The train/search partition produces learning signal. The validation/Pareto
partition drives parent selection and final best selection. The held-out test
partition is not used in-loop by default.

## 3. Reference Profiles

The word "GEPA" is overloaded upstream. Leaven must name the profile it is
matching.

### 3.1 Core GEPA Profile

This is the `gepa.optimize(...)` reference profile.

Required defaults:

- candidate selection: Pareto candidate selector;
- validation policy: full validation evaluation;
- batch sampler: epoch-shuffled minibatches;
- reflection minibatch size: `3` when using the default epoch sampler;
- component selector: round robin;
- acceptance: strict improvement;
- perfect score: `1.0`;
- skip perfect minibatches: true;
- merge: disabled by default;
- track best outputs: true by default in `gepa.optimize`;
- evaluation cache: disabled by default unless explicitly enabled;
- stop: `max_metric_calls` or explicit stopper is required.

### 3.2 DSPy GEPA Profile

This is `dspy.GEPA(...)`, a wrapper over `gepa.optimize(...)`.

Differences from the core profile:

- requires a reflection LM or custom instruction proposer;
- defaults `reflection_minibatch_size=3`;
- defaults `candidate_selection_strategy="pareto"`;
- defaults `skip_perfect_score=True`;
- defaults `component_selector="round_robin"`;
- defaults `use_merge=True` with `max_merge_invocations=5`;
- defaults `track_stats=False`;
- requires exactly one of `auto`, `max_full_evals`, or `max_metric_calls`;
- uses `valset=trainset` when no validation set is provided, with a warning;
- uses DSPy trace capture and predictor-level feedback projection.

Leaven must not call a run "DSPy GEPA parity" unless merge semantics and DSPy
trace/feedback behavior are either implemented or explicitly disabled with the
comparison labeled as "DSPy GEPA with merge disabled".

### 3.3 Optimize-Anything AIME Profile

This is the current upstream AIME example in `examples/aime_math`.

Reference settings:

- seed prompt: `Solve the math problem carefully. Break down the steps and provide the final answer as a single number.`
- train/validation source: `AI-MO/aimo-validation-aime`, shuffled with
  `random.Random(0)` and split 50/50;
- held-out test source: `MathArena/aime_2025`;
- solver model: `gpt-4.1-mini`;
- solver temperature: `1.0`;
- solver max tokens: `32000`;
- candidate execution: DSPy `ChainOfThought(MathSolverSignature)`;
- search budget: `max_metric_calls=500`;
- parallel evaluation: enabled;
- workers: `32`;
- evaluation cache: enabled;
- track best outputs: enabled;
- reflection model: upstream example currently uses `openai/gpt-5.1`;
- reflection examples include input, prompt, output answer, reasoning when
  present, scalar score, and execution feedback.

Leaven may intentionally use a stronger reflection model, such as the current
P8 default `gpt-5.4-mini` medium reasoning, but reports must label that as a
model delta rather than algorithm parity.

## 4. Phase Contract

### 4.1 Preflight

Before any provider call or candidate evaluation, GEPA must resolve:

- seed candidate and edit surface;
- train/search partition;
- validation/Pareto partition;
- optional held-out test partition;
- evaluator/scorer fingerprint;
- candidate cache identity policy;
- batch sampler state and RNG seed;
- candidate selector state;
- part selector state;
- acceptance criterion;
- validation evaluation policy;
- reflection renderer/parser/model/runtime fingerprint;
- merge configuration, if enabled;
- budget and stopper configuration;
- durable run directory and cache backends.

Missing train data, missing validation data in generalization mode, missing edit
surface, missing reflection backend, or missing deterministic cache identities
under deterministic evaluation caching must fail before the optimization loop.

Inference-time search is the explicit exception: it may set validation/Pareto to
the same case set as train/search, but this must be a named mode and reported.

### 4.2 Seed Full Validation

The first algorithmic act is full validation evaluation of the seed candidate.
This happens after optimization-start reporting and before iteration 1.

Required effects:

- evaluate the seed on every validation/Pareto case selected by the validation
  evaluation policy;
- record one per-case assessment row per validation case;
- initialize candidate list with seed at index 0;
- initialize per-candidate validation subscores for the seed;
- initialize per-validation-item best-score map and frontier membership;
- set total metric calls to the number of actually evaluated validation cases;
- store seed validation outputs when best-output tracking is enabled;
- emit/report baseline validation score as iteration 0;
- checkpoint the initialized state before the first mutation iteration.

If this phase is delayed until after a train minibatch or only recorded as a
scalar "validation best", parent selection is not GEPA-equivalent.

### 4.3 Iteration Start And Checkpoint

At each loop iteration:

1. Check stop conditions before starting new work.
2. Persist a clean checkpoint of graph, budget, evaluation cache, LM cache
   references, and GEPA continuation.
3. Increment the GEPA iteration counter.
4. Emit an iteration-start event with enough state to observe progress.

No new train evaluation, reflection call, proposal, or validation evaluation may
start after the metric-call stopper has tripped, except for already scheduled
parallel work whose overshoot is reported.

### 4.4 Parent Candidate Selection

Default parent selection is validation-Pareto selection, not train-population
selection.

For each validation item or frontier axis:

1. Find the best score achieved by any candidate on that validation item.
2. Put all tied candidates for that item into that item's candidate set.
3. Remove candidates dominated by other frontier candidates.
4. Count how many surviving validation items each candidate leads.
5. Sample a parent candidate with probability proportional to that count.

For core GEPA, the candidate selector reads validation frontier state:

```text
program_at_pareto_front_valset
prog_candidate_val_subscores
program_full_scores_val_set
per_program_tracked_scores
```

A selector that returns the deterministic best average candidate is a different
ablation. It may exist as `current_best`, but it is not the default GEPA
selector.

### 4.5 Train Minibatch Sampling

The default train sampler draws a minibatch from the train/search set. The
upstream default batch size is `3`.

Required behavior:

- sample only from train/search by default;
- record sampled case IDs in the iteration trace;
- make sampling deterministic under stored seed and sampler state;
- use the same sampled minibatch for parent screening and child screening.

Validation and test cases must not appear in reflection examples under the
ordinary generalization profile.

### 4.6 Parent Evaluation With Trace Capture

The selected parent is evaluated on the sampled train minibatch with trace or
side-info capture enabled.

Required behavior:

- produce one output and one scalar score per sampled case;
- capture trajectory/trace material when available;
- capture scorer/evaluator feedback text;
- count actual uncached case evaluations as metric calls;
- cache the parent minibatch outputs/scores per candidate/case when evaluation
  caching is enabled;
- skip the proposal if no trajectories or usable side information exist;
- skip the proposal when `skip_perfect_score=true` and every parent minibatch
  score is at least the configured perfect score.

Leaven can treat scorer feedback plus runner output as trace material for simple
single-prompt AIME, but multi-module and agentic tasks need module-local trace
projection, not only final scalar evidence.

### 4.7 Part Or Component Selection

GEPA selects which component to mutate after parent minibatch evaluation.

Default behavior:

- round-robin over mutable components/parts;
- one component per reflective mutation iteration.

Allowed explicit strategies:

- update all components;
- trace-aware component selection;
- custom strategy using trajectories and scores.

Part selection state must checkpoint and restore. It must not be inferred from
surface ordering after resume unless the ordering is fingerprinted and stable.

### 4.8 Reflective Dataset Construction

The reflective dataset is built from parent train-minibatch trajectories and
feedback for the selected component.

Required contents for each reflective example:

- runner-visible input projection;
- generated output for the selected component or whole system;
- scalar score;
- feedback text;
- parse/format failure feedback when available;
- trace refs and assessment refs;
- selected component name/part;
- parent candidate ref.

For DSPy parity, the example shape is:

```text
Inputs
Generated Outputs
Feedback
```

DSPy chooses one matching predictor trace per example, prefers failed parses
when present, and can inject output-format instructions as feedback. Leaven's
generic `ReflectiveExample` may use Rust fields instead, but the rendered prompt
must carry equivalent information.

Hidden target data may only reach reflection through scorer-produced feedback.
Reflection builders must not read raw targets directly.

### 4.9 Reflection Prompt And Parser

The default GEPA instruction prompt is the upstream instruction-proposal
template:

````text
I provided an assistant with the following instructions to perform a task for me:
```
<curr_param>
```

The following are examples of different task inputs provided to the assistant
along with the assistant's response for each of them, and some feedback on how
the assistant's response could be better:
```
<side_info>
```

Your task is to write a new instruction for the assistant.

Read the inputs carefully and identify the input format and infer detailed task
description about the task I wish to solve with the assistant.

Read all the assistant responses and the corresponding feedback. Identify all
niche and domain specific factual information about the task and include it in
the instruction, as a lot of it may not be available to the assistant in the
future. The assistant may have utilized a generalizable strategy to solve the
task, if so, include that in the instruction as well.

Provide the new instructions within ``` blocks.
````

The default parser extracts the text between the first and last triple-backtick
fences, strips an optional language line, and otherwise falls back to stripped
assistant text. JSON reflection output is a non-default Leaven extension and
must be labeled as such.

### 4.10 Proposal And Child Evaluation

The reflection output creates a child candidate by changing only the selected
component(s) in a copy of the parent.

Leaven lowering:

```text
ReflectRequest
  -> LmRequest or agent proposal request
  -> assistant text
  -> parsed surface edit
  -> EditSurface::change_part(parent_artifact, part, edit)
  -> Proposal::mutate(parent, change)
  -> RunContext::record_proposal_batch
  -> RunContext::apply_batch
```

The child is evaluated on the exact same train minibatch as the parent.

Required behavior:

- child screening uses the same sampled case IDs;
- child screening produces one output and one score per case;
- acceptance sees parent and child scores for the same cases;
- rejected children may remain graph truth, but they do not enter the GEPA
  candidate pool/frontier;
- accepted children preserve parent lineage and `informed_by` refs.

### 4.11 Acceptance

The default acceptance criterion is strict improvement on the train minibatch.

Core GEPA accepts when the child minibatch score improves according to the
configured acceptance criterion. `improvement_or_equal` is an explicit
alternative. Merge uses its own parent-comparison rule.

Acceptance must be computed on train/search screening evidence only. Validation
does not retroactively decide whether the child exists; validation decides
frontier and final selection after acceptance.

### 4.12 Accepted-Candidate Full Validation

Every accepted child must be evaluated according to the validation policy.
The default validation policy is full validation over the validation/Pareto set.

Required effects:

- evaluate all validation/Pareto cases for the accepted child under the full
  validation policy;
- use evaluation cache hits when available;
- count only uncached case evaluations as metric calls;
- append the child to the candidate list;
- record parent lineage;
- record per-validation-item scores for the child;
- update aggregate validation score;
- update per-validation-item Pareto-front candidate sets;
- update objective or hybrid Pareto maps when configured;
- record discovery metric-call count;
- update best validation candidate according to validation policy;
- emit validation/Pareto update events.

This is the major distinction from train-only prompt search. A Leaven GEPA run
that only validates a scalar best candidate, or that validates but does not feed
validation subscores into parent selection, is not real GEPA.

### 4.13 Optional Merge

Merge is part of the GEPA family but not the core `gepa.optimize` default.
DSPy GEPA enables it by default.

Merge behavior requirements when enabled:

- schedule merge attempts only when configured and previous search produced
  enough candidate lineage;
- select parent candidates from frontier state;
- merge text components by combining complementary parent components;
- screen the merged candidate on a merge-selected subset;
- accept merge only when it improves over the relevant parent scores;
- full-validate accepted merges and update the same validation frontier;
- record lineage with both parents.

Leaven must expose merge as an explicit slot/profile until it is implemented.
The plain-GEPA AIME comparison should disable merge; the DSPy-default comparison
should enable merge or disclose its absence.

### 4.14 Stop And Final Result

Stop conditions are checked before each iteration. The ordinary metric-call
stopper observes actual metric calls counted by GEPA.

Final result must expose:

- all accepted candidates, including seed;
- parent lineage for each candidate;
- validation aggregate score per candidate;
- validation subscores per candidate and validation item;
- per-validation-item best candidate sets;
- discovery metric-call count per accepted candidate;
- total metric calls;
- number of full validation evaluations;
- best candidate index under the validation policy;
- optional best outputs per validation item;
- run directory and seed.

The ordinary Leaven `Optimized<A>` facade may keep a smaller user surface, but a
GEPA detailed result/report must exist for audits and parity experiments.

## 5. Cache Parity

GEPA has two distinct caches.

### 5.1 Evaluation Cache

The upstream evaluation cache stores evaluation results per `(candidate,
example)` pair:

```text
candidate content hash
example id
  -> output
  -> score
  -> optional objective scores
```

Cache parity requirements:

- independent per-case GEPA evaluations must be reusable across overlapping
  minibatches and full validation runs;
- cache hits return outputs/scores and charge zero new metric calls;
- cache keys must include evaluator/scorer behavior, candidate cache identity,
  case identity/content version, and any request shape that changes scoring;
- cache keys must not include API keys, wall-clock time, local run-dir paths, or
  transient provider ids;
- parent minibatch evaluations are cache-written even if the proposal is skipped
  for no trajectories or all-perfect scores;
- accepted-candidate validation uses the same cache;
- durable resume restores cache entries consistently with graph/evidence truth.

Leaven's current request-level `EvaluationCacheKey` is useful but not enough for
GEPA parity because a request containing case IDs `[a, b, c]` cannot satisfy a
later request containing `[a]` or `[a, c, d]`. GEPA parity requires a per-case
cache adapter for independent per-case evaluations, or evaluation lowering that
decomposes GEPA requests into per-case cacheable units while preserving one
GEPA-level evaluation event.

### 5.2 LM Response Cache

The LM response cache is separate from evaluation caching. It memoizes solver
LM and reflection LM calls.

Parity requirements:

- solver LM cache hits must return zero new provider cost and zero new metric
  cost when the surrounding evaluator result is also cached;
- reflection LM cache hits must return zero new reflection cost;
- reflection LM cache keys must include prompt template, rendered messages,
  model, sampling, output mode, provider-relevant hints, and role fingerprint;
- solver and reflection roles must have separate telemetry;
- reports must distinguish evaluation-cache hits from LM response-cache hits.

Upstream GEPA's `max_metric_calls` counts evaluator metric calls, not reflection
LM token cost. Leaven may also track dollars, tokens, and reflection cost, but
the GEPA reference metric-call stopper must not silently spend fewer or more
rollouts because reflection calls share the same cap.

## 6. Leaven Default Profiles

Leaven should expose named profiles instead of making users infer parity from a
pile of knobs.

### 6.1 `Gepa::reference()`

Matches core GEPA semantics:

- validation full-eval seed initialization;
- validation-Pareto parent selection;
- epoch-shuffled train minibatch size 3;
- round-robin part selection;
- default upstream reflection prompt and fenced parser;
- strict-improvement acceptance;
- skip-perfect enabled;
- merge disabled;
- per-case evaluation cache enabled when the run cache policy allows it;
- detailed GEPA result/report enabled.

### 6.2 `Gepa::dspy_reference()`

Matches DSPy wrapper semantics:

- all `Gepa::reference()` behavior;
- merge enabled by default with max 5 merge invocations;
- DSPy-style trace/feedback projection when used with a DSPy-compatible adapter
  or equivalent Leaven trace adapter;
- warning/report disclosure if validation is omitted and train is reused as
  validation/Pareto.

### 6.3 `Gepa::aime_reference()`

Matches the upstream optimize-anything AIME example:

- AIME seed prompt;
- AIME dataset split;
- solver role `gpt-4.1-mini`, temperature 1.0, max tokens 32000;
- search budget 500 metric calls;
- parallel evaluation 32 workers;
- evaluation cache enabled;
- best outputs tracked;
- reflection prompt/parser as reference;
- reflection model configurable, with upstream `openai/gpt-5.1` as the strict
  reference label and Leaven's stronger model labeled as a model delta.

### 6.4 `Gepa::leaven_plus()`

This profile may be better than upstream GEPA, but it must not be used for an
unqualified parity claim.

Allowed improvements:

- stronger reflection model;
- richer target-safe trace/evidence projection;
- durable per-case cache and LM response cache;
- clearer progress callbacks and structured reports;
- parallel evaluation with bounded concurrency;
- typed surfaces instead of string maps;
- better resume/refusal behavior.

Disallowed under a parity label:

- train-only parent selection;
- scalar-best parent selection unless labeled as `current_best`;
- JSON-only reflection output under the default GEPA prompt;
- reflection on validation/test oracle material;
- different acceptance criterion without labeling;
- merge enabled or disabled contrary to the claimed profile;
- budget accounting that changes the number of evaluator rollouts.

## 7. Current Leaven Audit

This section audits the current Leaven direction against the target above. It is
not the target; the preceding sections are the target.

| Area | Required reference behavior | Current Leaven state | Gap |
| --- | --- | --- | --- |
| Seed validation | Full validation before iteration 1 initializes frontier | `Gepa::initialize` is no-op; seed validation happens during first `step` | Move seed validation and frontier initialization into `initialize` |
| Parent selection | Weighted stochastic selection from validation Pareto frontier | `ParetoFrequencyWeighted` delegates to `population.best_candidate()` | Implement real frequency-weighted Pareto selector state |
| Frontier source | Validation/Pareto subscores drive selection | P8 config filters `ParetoFrontier` to `TRAIN`; validation best is scalar-only | Maintain validation frontier separately from train screening |
| Accepted candidates | Full-validate every accepted child and update Pareto maps | `FullValidation` can evaluate accepted children, but population update remains train-based | Feed validation casewise observations into selection/frontier state |
| Candidate result | Return best validation aggregate plus detailed candidate table | `Optimized<A>` returns best facade; GEPA detailed result is not parity-shaped | Add detailed GEPA report/result payload |
| Eval cache | Per candidate/case cache reuse | Engine cache is request-level over full case-id vector | Add GEPA per-case cache adapter or per-case request lowering |
| Cache identity | Candidate content identity required for deterministic eval cache | P8 `AimePrompt` now exposes `cache_identity` | Keep and require this in parity tests |
| Metric-call budget | `max_metric_calls` counts evaluator rollouts | Leaven budget ledger charges generic costs and proposal costs too | Expose GEPA search metric-call stopper over evaluator rollouts |
| Skip perfect | Default skips all-perfect parent minibatches | No default skip-perfect GEPA policy yet | Add skip-perfect policy and events |
| No trajectories | Skip proposal when no trajectories/examples exist | Reflective dataset may be empty and renderer can continue | Refuse/skip no-example reflection by default |
| Reflection prompt | Upstream fenced text replacement prompt | Leaven default template matches the text shape | Preserve as default; label custom JSON prompts |
| Reflection examples | Inputs, outputs, feedback, trace/format failures | Leaven carries input/output/score/feedback; module trace parity is partial | Add trace/format-failure projection for multi-module parity |
| Component selection | Round-robin default over components after parent eval | `RoundRobinPart` exists | Ensure state is checkpointed and ordering fingerprinted |
| Merge | Core default off; DSPy default on | No real merge path in Leaven GEPA loop | Implement or disclose per profile |
| Parallel proposals | Optimize-anything can run parallel proposals | Leaven evaluates cases in parallel, but proposals are serial | Not required for core parity; label if absent in optimize-anything parity |
| Progress | Upstream callbacks cover candidate selection, minibatch, eval, reflection, accept/reject, validation, budget, state save | P8 progress callback maps generic engine events only | Add GEPA-specific events/callback summaries |
| AIME solver | DSPy ChainOfThought answer field | P8 Rust solver returns answer text through `leaven-lm` | Acceptable Rust-native delta if report labels prompt/trace difference |
| AIME dataset | HF AIME split and MathArena test | P8 cache materialization is intended to match | Add parity test/report that prints source counts and split hash |

## 8. Systematic Fix Plan

The implementation order should preserve proof quality and commit granularity.

### P0. Freeze The Reference Contract

Add tests that fail on today's non-parity behavior:

- seed full validation happens before first train minibatch;
- initial seed validation creates validation frontier entries;
- default candidate selector samples from validation frontier, not train scores;
- accepted child validation updates frontier state;
- all-perfect parent minibatch skips reflection;
- no reflective examples skips reflection;
- detailed GEPA result exposes candidate table, validation subscores, Pareto
  membership, lineage, discovery counts, and total metric calls.

Focused gate: `cargo nextest run -p leaven-gepa --test gepa_smoke`.

### P1. Introduce GEPA Reference State

Add optimizer-private state that mirrors the reference concepts without copying
Python shapes:

```text
accepted_candidates: Vec<CandidateId>
parents: Vec<Vec<CandidateId>>
validation_subscores: Vec<BTreeMap<CaseId, ScalarEvidence>>
validation_outputs: Option<...>
per_case_frontier: BTreeMap<CaseId, BTreeSet<CandidateId>>
aggregate_validation_scores: Vec<f64>
discovery_metric_calls: Vec<u64>
total_metric_calls: u64
```

This state lives in `leaven-gepa`, because it is optimizer strategy state.
Reusable casewise frontier utilities may remain in `leaven-population`.

### P2. Move Seed Validation Into Initialize

`Gepa::initialize` must:

- find seed candidate;
- evaluate validation/Pareto set;
- initialize GEPA reference state;
- initialize detailed result/report state;
- checkpoint before `step` can mutate.

This is a hard cutover. Do not keep the old first-step seed baseline path as a
parallel default.

### P3. Make Parent Selection Validation-Pareto Native

Replace the default selector behavior with reference weighted sampling:

- derive candidate frequency from `per_case_frontier`;
- remove dominated candidates before sampling;
- use stored RNG state;
- preserve `SelectBestCandidate` as an explicit ablation only.

P8 must stop configuring a train-filtered population for the ordinary GEPA
profile.

### P4. Fix Cache Parity

Implement one of:

1. GEPA-specific independent per-case cache lookup/write that reuses engine
   assessment rows; or
2. engine-level per-case decomposition for `AssessmentGranularity::PerCase`
   independent requests.

The selected path must prove overlapping requests hit:

```text
candidate A on [1,2,3] writes 3 reusable entries
candidate A on [2,3,4] hits 2 and evaluates 1
candidate A full validation hits all previously seen validation cases
```

### P5. Add Skip/Failure Semantics

Add default GEPA skip policies:

- no parent trajectories/examples -> skip proposal;
- all parent scores perfect -> skip proposal;
- reflection parser failure -> proposal failure event, no child;
- child evaluation failure -> failed proposal/screening event, no admission.

These states must be visible in callbacks and reports.

### P6. Complete Reflection Trace Parity

For one-prompt AIME, output/score/feedback is enough to reproduce the
optimize-anything side-info path. For DSPy-style multi-module parity, Leaven
needs a trace projection that can select examples for a specific surface part.

Required additions:

- module/part-local trace refs;
- parse/format failure feedback;
- selected trace instance choice policy;
- renderer support for nested `Inputs` / `Generated Outputs` / `Feedback`
  records.

### P7. Add Merge Profile

Implement merge only after plain GEPA parity is green.

Required proof:

- core profile keeps merge disabled;
- DSPy profile enables merge by default;
- accepted merge full-validates and updates the same validation frontier;
- merge lineage records both parents.

### P8. Add Operator Profiles And Reports

Add named operator profiles and report labels:

- `reference-core-gepa`;
- `reference-dspy-gepa`;
- `reference-aime-gepa`;
- `leaven-plus-aime-gepa`.

Reports must disclose:

- upstream snapshot/profile;
- model roles and model deltas;
- merge setting;
- validation/test source hashes/counts;
- evaluation cache hit/miss/bypass counts;
- LM cache hit/miss/bypass counts by role;
- metric calls spent on search;
- final report evaluations outside search;
- accepted/rejected/skipped proposal counts;
- best candidate lineage and validation score.

## 9. Proof Requirements

Doc/path verification for this spec:

- `docs/specs/gepa_reference_behavior.md`
- `docs/specs/gepa_aime_paper_parity.md`
- `docs/specs/gepa_optimizer_surface.md`
- `docs/specs/gepa_reflection_evidence_visibility.md`
- `crates/leaven-gepa/AGENTS.md`
- `examples/p8_aime_gepa/AGENTS.md`

Focused implementation gates as fixes land:

- `cargo nextest run -p leaven-gepa --test gepa_smoke`
- `cargo nextest run -p leaven-gepa --test lm_reflection`
- `cargo nextest run -p leaven-gepa --test agent_stage_routing`
- `cargo nextest run -p leaven-population`
- `cargo nextest run -p p8_aime_gepa`
- `cargo test -p leaven --test topology_contract`

Live AIME parity gate:

```text
OPENAI_API_KEY=...
LEAVEN_AIME_LIVE_OPENAI=1
LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
cargo run --release -p p8_aime_gepa
```

The live gate must not be claimed as GEPA parity until the report proves the
reference profile, split sources, cache behavior, budget accounting, and model
deltas named above.

## 10. Source Crosswalk For Implementation

Use this table when implementing or reviewing parity. The line numbers are from
the verified upstream snapshots named at the top of this file. If upstream line
numbers drift, the function names remain the authority.

| Behavior | Upstream source | Leaven landing point | Implementation note |
| --- | --- | --- | --- |
| Build public optimizer defaults | `src/gepa/api.py:43 optimize` | `crates/leaven-gepa/src/builder.rs`, `crates/leaven-gepa/src/optimizer.rs` | Core GEPA default is Pareto selection, full validation policy, round-robin module selection, strict improvement, merge off, skip-perfect on. |
| Build optimize-anything defaults | `src/gepa/optimize_anything.py:1119 optimize_anything` | P8 profile builder and examples | AIME uses optimize-anything, not DSPy wrapper. It adds ASI, optional seedless/single-instance modes, cache storage modes, and parallel proposal resolution. |
| Build DSPy defaults | `dspy/teleprompt/gepa/gepa.py:336 GEPA.__init__`, `:476 compile` | future DSPy-profile adapter or report profile | DSPy wrapper turns merge on by default and uses DSPy trace capture. Do not mix this with plain GEPA unless the report says so. |
| Seed validation before loop | `src/gepa/core/engine.py:458 run`, especially seed eval at `:527` | `Gepa::initialize` | This must happen before train minibatch sampling. |
| Initialize GEPA state | `src/gepa/core/state.py:660 initialize_gepa_state`, `GEPAState.__init__` at `:142` | new `GepaReferenceState` in `leaven-gepa` | Seed candidate is candidate index 0. Validation subscores and frontier maps start from seed full validation. |
| Per-case evaluation cache | `src/gepa/core/state.py:46 EvaluationCache` | `leaven-engine` per-case cache or GEPA adapter cache | Upstream key is candidate content hash plus example id. Leaven request-level cache is currently too coarse. |
| Parent selection from validation Pareto | `src/gepa/strategies/candidate_selector.py:11 ParetoCandidateSelector`, `src/gepa/gepa_utils.py:90 select_program_candidate_from_pareto_front` | `crates/leaven-gepa/src/selector.rs` | The default selector must not call `population.best_candidate()` directly. |
| Dominance pruning | `src/gepa/gepa_utils.py:23 is_dominated`, `:37 remove_dominated_programs` | `leaven-population` helper or `leaven-gepa` selector helper | Dominance is over frontier membership sets, not only average score. |
| Full validation policy | `src/gepa/strategies/eval_policy.py:34 FullEvaluationPolicy` | `crates/leaven-gepa/src/validation.rs` | Full validation returns all validation ids and picks best aggregate with coverage tie-break. |
| Iteration checkpoint and stop | `src/gepa/core/engine.py:620` main loop, `src/gepa/utils/stop_condition.py:163 MaxMetricCallsStopper` | engine stopper plus GEPA continuation | Check stop before incrementing iteration and starting more work. |
| Reflective mutation preparation | `src/gepa/proposer/reflective_mutation/reflective_mutation.py:176 prepare_proposal` | beginning of `Gepa::step` | Select parent from validation frontier, sample train minibatch, emit events. |
| Parent evaluation with traces | `reflective_mutation.py:260 execute_proposal` | `evaluate_casewise` plus trace/evidence projection | Parent evaluation is with `capture_traces=True`. Leaven needs equivalent trace/evidence refs. |
| Skip no trajectories | `reflective_mutation.py:294` | GEPA skip policy | Proposal is skipped, but parent evaluation still counts/caches. |
| Skip all perfect | `reflective_mutation.py:309` | GEPA skip policy | Requires `perfect_score`. Default in GEPA/DSPy is 1.0. |
| Reflective dataset build | `reflective_mutation.py:329`, DSPy adapter at `dspy/gepa_utils.py:198` | `ReflectiveDatasetBuilder`, P8 AIME builder, future trace-aware builders | Build exactly once and pass to all reflection backends. |
| Proposal call and parser | `src/gepa/strategies/instruction_proposal.py:12` | `DefaultReflectionRenderer`, `PlainTextEditParser` | Leaven currently matches the default prompt/parser shape. |
| Child evaluation on same minibatch | `reflective_mutation.py:371` | child `evaluate_casewise` in `Gepa::step` | Must use exact same case IDs as parent screening. |
| Apply proposal output and parent cache write | `reflective_mutation.py:430 apply_proposal_output` | cache write after parent evaluation, before/with step accounting | Upstream writes parent minibatch results to cache even if later skipped. |
| Acceptance processing | `src/gepa/core/engine.py:287 _accept_reflective_proposal`, `:350 _process_proposal_output` | acceptance block in `Gepa::step` | Acceptance is train-minibatch based. Full validation follows acceptance. |
| Full-eval accepted candidate | `src/gepa/core/engine.py:175 _run_full_eval_and_add` | `validate_candidate` plus state update | Must update validation frontier, candidate table, lineage, discovery counts. |
| Final result | `src/gepa/core/result.py:16 GEPAResult`, `:246 from_state` | GEPA detailed result/report facade | `Optimized<A>` is not enough for parity audits. |

## 11. Reference State Model

Implementing GEPA from the paper alone is too underspecified. The useful
reference is the state model in `GEPAState`.

### 11.1 Upstream State Fields And Meaning

These upstream fields must have Leaven equivalents:

| Upstream field | Meaning | Leaven equivalent |
| --- | --- | --- |
| `program_candidates` | accepted candidates, seed first | `Vec<CandidateId>` plus artifact lookup in graph |
| `parent_program_for_candidate` | accepted candidate lineage by candidate index | `Vec<Vec<CandidateIndexOrId>>` in GEPA state/report |
| `program_full_scores_val_set` | aggregate validation score for each accepted candidate | `Vec<f64>` or finite scalar score newtype |
| `prog_candidate_val_subscores` | per-validation-id score map for each accepted candidate | `Vec<BTreeMap<CaseId, ScalarEvidence>>` |
| `pareto_front_valset` | best score per validation id | `BTreeMap<CaseId, ScalarEvidence>` |
| `program_at_pareto_front_valset` | candidates tied for best per validation id | `BTreeMap<CaseId, BTreeSet<GepaCandidateIndex>>` |
| `num_metric_calls_by_discovery` | total metric calls at candidate discovery | `Vec<u64>` |
| `total_num_evals` | evaluator metric calls used so far | GEPA-owned metric-call counter derived from evaluation reports |
| `num_full_ds_evals` | count of full validation dataset evaluations | `u64` |
| `best_outputs_valset` | best validation outputs per validation id | optional report/debug store |
| `full_program_trace` | iteration trace/debug log | durable GEPA trace/report entries |
| `evaluation_cache` | per candidate/example cache | engine/GEPA per-case eval cache |

Do not store artifacts twice in GEPA private state. Leaven graph truth owns the
artifact for each `CandidateId`. GEPA private state stores candidate identity,
candidate index, frontier math, lineage, sampler/selector/policy state, and
report summaries.

### 11.2 Candidate Index Versus Candidate Id

Upstream GEPA uses integer program indices. Leaven uses `CandidateId`. For
parity reports, Leaven still needs a stable GEPA candidate index because:

- upstream result tables and Pareto membership are index-based;
- candidate discovery order matters;
- parents refer to discovered candidates by index;
- deterministic comparisons need to say "candidate 0 is seed, candidate 3 was
  discovered after 117 metric calls".

Required Leaven state:

```rust
struct GepaCandidateRecord {
    index: u32,
    candidate: CandidateId,
    parents: Vec<u32>,
    discovery_metric_calls: u64,
    validation_score: Option<FiniteF64>,
    validation_rows: Vec<AssessmentId>,
}

struct GepaReferenceState {
    records: Vec<GepaCandidateRecord>,
    candidate_to_index: BTreeMap<CandidateId, u32>,
    validation_subscores: Vec<BTreeMap<CaseId, ScalarEvidence>>,
    validation_frontier_scores: BTreeMap<CaseId, ScalarEvidence>,
    validation_frontier_candidates: BTreeMap<CaseId, BTreeSet<u32>>,
    total_metric_calls: u64,
    full_validation_evals: u64,
    rng: StoredRng,
    sampler_state: ...,
    selector_state: ...,
    part_selector_state: ...,
}
```

`records.len()` is the number of accepted candidates, not the number of
proposal attempts. Rejected children may exist in graph truth, but they do not
get GEPA candidate indices unless admitted by acceptance.

### 11.3 Validation Frontier Update Algorithm

When a candidate is full-validated:

```text
candidate_index = records.len()
records.push(candidate)
validation_subscores.push({})

for each validation row:
    case_id = row.case
    score = row.scalar_score
    validation_subscores[candidate_index][case_id] = score

    old_best = validation_frontier_scores.get(case_id)
    if old_best is missing or score > old_best:
        validation_frontier_scores[case_id] = score
        validation_frontier_candidates[case_id] = {candidate_index}
        best_outputs[case_id] = [(candidate_index, output)] if tracking
    else if score == old_best:
        validation_frontier_candidates[case_id].insert(candidate_index)
        best_outputs[case_id].push((candidate_index, output)) if tracking
```

Then recompute aggregate validation score:

```text
aggregate = average(validation_subscores[candidate_index].values())
```

For `FullEvaluationPolicy`, coverage should equal validation set size for every
candidate. If a custom validation policy evaluates subsets, the best-candidate
policy must tie-break by coverage like upstream `FullEvaluationPolicy` does.

### 11.4 Pareto Weighted Selection Algorithm

The default parent selector is:

```text
input:
  validation_frontier_candidates: Map<CaseId, Set<CandidateIndex>>
  per_candidate_scores: Vec<f64>
  rng

fronts = clone(validation_frontier_candidates)
fronts = remove_dominated_programs(fronts, per_candidate_scores)

frequency = empty map
for each (_, candidate_set) in fronts:
    for candidate in candidate_set:
        frequency[candidate] += 1

sampling_list = []
for each (candidate, freq) in frequency:
    repeat freq times:
        sampling_list.push(candidate)

assert sampling_list is not empty
return rng.choice(sampling_list)
```

Dominance in upstream GEPA is set-based:

```text
candidate y is dominated if there exists another candidate or combination of
remaining candidates that covers every validation item where y appears on the
frontier. Dominated candidates are removed iteratively.
```

Do not replace this with "choose highest average score." That is the
`current_best` ablation.

### 11.5 Train Population Versus Validation Frontier

Leaven currently has `ParetoFrontier` in `leaven-population`. That is reusable
population machinery, but real GEPA's default selector uses validation frontier
state, not train screening scores.

Allowed implementation shapes:

1. Add a GEPA-owned validation-frontier state and make the default selector read
   it directly.
2. Extend `ParetoFrontier` so GEPA can maintain a validation partition frontier
   and expose frequency-weighted selection data.

Not allowed:

- keep a train-filtered `ParetoFrontier` as the only default population and
  return its deterministic `best()`;
- update only `validation_best`;
- use validation only for final report while parent selection uses train scores.

## 12. Exact Loop Pseudocode

This is the implementation target, using Leaven terms.

### 12.1 Run Initialization

```text
initialize(ctx):
    seed = graph.seed_candidate()
    assert seed exists

    validation_set = validation_policy.initial_set()
    assert validation_set is non-empty unless explicit train-as-val mode

    report = evaluate_independent_per_case(
        candidate = seed,
        set = validation_set,
        purpose = ValidationBaseline,
        cache = enabled_if_policy_allows,
    )

    rows = load_case_rows(report.assessment_ids)
    seed_index = add_candidate_record(
        candidate = seed,
        parents = [],
        discovery_metric_calls = metric_calls_spent_so_far + report.new_metric_calls,
    )
    update_validation_frontier(seed_index, rows)
    total_metric_calls += report.new_metric_calls
    full_validation_evals += 1

    emit gepa_seed_validated(...)
    checkpoint(...)
```

`report.new_metric_calls` cannot be inferred from `assessment_ids.len()` once
cache hits exist. The evaluation report needs enough cache detail to say how
many case evaluations were newly run.

### 12.2 One Reflective Mutation Iteration

```text
step(ctx):
    if stopper_tripped(total_metric_calls, budget):
        return Done

    checkpoint_before_iteration()
    iteration += 1
    emit gepa_iteration_started(iteration)

    parent_index = candidate_selector.select(validation_frontier, rng)
    parent = records[parent_index].candidate
    emit gepa_candidate_selected(iteration, parent_index, parent)

    batch = batch_sampler.sample(train_partition, rng/state)
    emit gepa_minibatch_sampled(iteration, batch.case_ids)

    parent_eval = evaluate_parent_with_trace(parent, batch)
    total_metric_calls += parent_eval.new_metric_calls
    cache_write(parent_eval)
    emit gepa_parent_evaluated(...)

    if parent_eval.rows.is_empty or parent_eval.reflective_examples.is_empty:
        emit gepa_proposal_skipped(reason = no_reflective_examples)
        return Continue

    if skip_perfect and all(row.score >= perfect_score for row in parent_eval.rows):
        emit gepa_proposal_skipped(reason = all_scores_perfect)
        return Continue

    part = part_selector.select(parent_artifact, surface, parent_eval.trace_refs)
    emit gepa_part_selected(part)

    request = reflective_dataset_builder.build(parent, part, parent_eval.rows)
    emit gepa_reflective_dataset_built(request.summary)

    proposal_result = reflector.reflect_candidate(ctx, surface, request)
    if proposal_result.failed:
        emit gepa_proposal_failed(...)
        return Continue

    child = proposal_result.candidate
    emit gepa_child_created(child)

    child_eval = evaluate_child(child, same batch)
    total_metric_calls += child_eval.new_metric_calls
    emit gepa_child_evaluated(...)

    decision = acceptance.decide(parent_eval.rows, child_eval.rows)
    emit gepa_acceptance_decided(decision)

    if decision.reject:
        return Continue

    validation_eval = evaluate_full_validation(child)
    total_metric_calls += validation_eval.new_metric_calls
    child_index = add_candidate_record(
        candidate = child,
        parents = [parent_index],
        discovery_metric_calls = total_metric_calls,
    )
    update_validation_frontier(child_index, validation_eval.rows)
    full_validation_evals += 1
    emit gepa_candidate_accepted_and_validated(...)

    if merge_enabled:
        merge_scheduler.note_new_candidate(child_index)

    return Continue
```

### 12.3 Important Ordering Constraints

The following ordering differences change behavior and must be tested:

- Seed validation must precede the first train minibatch.
- Parent candidate selection must precede train minibatch evaluation.
- Part selection happens after parent evaluation because trace-aware selectors
  may use parent trajectories.
- Reflection dataset is built before any LM call and is reused byte-identically
  by LM-backed and agent-backed reflectors.
- Child evaluation uses the same train minibatch as parent evaluation.
- Acceptance happens before full validation.
- Full validation happens before the accepted child enters the GEPA candidate
  table/frontier.
- Stop conditions are checked before starting a new iteration, not after the
  next parent evaluation has already begun.

## 13. Evaluation And Metric-Call Accounting

### 13.1 What Counts As A Metric Call

For GEPA parity, one metric call is one evaluator/scorer invocation for one
candidate on one case.

Examples:

- seed validation over 45 validation cases = 45 metric calls unless cached;
- parent train minibatch of size 3 = 3 metric calls unless cached;
- child train minibatch of size 3 = 3 metric calls unless cached;
- accepted child full validation over 45 cases = 45 metric calls unless cached;
- reflection LM call = 0 GEPA metric calls, but has LM cost;
- final held-out test report = not search metric calls unless explicitly folded
  into the search budget profile.

Leaven can additionally charge:

- LM token cost;
- wall time;
- dollars;
- proposal-stage cost.

Those are useful product improvements, but the `max_metric_calls` parity stop
must be based on evaluator/scorer case calls.

### 13.2 Needed Evaluation Report Fields

Current `EvaluationReport` exposes assessment IDs and cache status for the whole
request. GEPA parity needs more detail:

```rust
pub struct EvaluationReport {
    pub assessment_ids: Vec<AssessmentId>,
    pub cache: CacheStatus,
    pub cache_rows: EvaluationCacheRows,
    pub metric_calls_new: u64,
}

pub struct EvaluationCacheRows {
    pub hits: Vec<AssessmentId>,
    pub misses: Vec<AssessmentId>,
    pub bypassed: Vec<(CaseId, CacheBypassReason)>,
}
```

The exact type can differ, but the report must let GEPA count new metric calls
when a request is partially cached.

### 13.3 Parallel Overshoot

Upstream can overshoot a metric-call cap because it checks the stopper at loop
boundaries. Leaven should make this explicit:

- no new iteration after cap is observed;
- already in-flight parallel case jobs may complete;
- report `metric_calls_cap`, `metric_calls_spent`, and
  `metric_calls_overshoot`;
- do not hide overshoot by clamping the displayed number.

## 14. Cache Implementation Detail

### 14.1 Per-Case Cache Key

For independent per-case GEPA evaluations, the semantic cache key is:

```text
evaluator_fingerprint
cache_policy
case_set_version or case_content_fingerprint
case_id
candidate_cache_identity
assessment_granularity = PerCase
purpose class that changes evaluator behavior, if any
runner/scorer fingerprints
```

`purpose` should not split the cache if it is only a report label. It must split
the cache if evaluator/scorer logic reads it.

### 14.2 Partial Hit Algorithm

```text
resolve request to ordered case_ids
for each case_id:
    key = per_case_key(candidate, case_id)
    if cache has key and graph contains assessment:
        hits.push(assessment)
    else:
        misses.push(case_id)

evaluate only misses
record one assessment per miss
write each miss assessment under its per-case key
return assessment_ids in original case order
```

This preserves GEPA's per-example reuse while keeping Leaven's graph evidence
truth.

### 14.3 Request-Level Cache Can Remain

Leaven can keep request-level cache for non-GEPA and listwise/pairwise requests,
but GEPA independent per-case requests need per-case reuse. Do not remove
request-level cache unless a broader engine simplification is deliberately
chosen.

## 15. Callback And Progress Event Contract

The current generic engine events are not enough for live GEPA operation. GEPA
needs strategy-level events.

Required GEPA events:

```rust
GepaSeedValidationStarted
GepaSeedValidationCompleted
GepaIterationStarted
GepaStateSaved
GepaCandidateSelected { candidate_index, candidate_id, validation_score }
GepaMinibatchSampled { partition, case_ids }
GepaParentEvaluationStarted
GepaParentEvaluationCompleted { scores, metric_calls_new, cache_rows }
GepaProposalSkipped { reason }
GepaPartSelected { part_label }
GepaReflectiveDatasetBuilt { example_count, source_ref_count }
GepaReflectionStarted
GepaReflectionCompleted { cost, cache_status }
GepaProposalParsed
GepaProposalParseFailed
GepaChildEvaluationCompleted { scores, metric_calls_new, cache_rows }
GepaAcceptanceDecided { accepted, parent_score, child_score, reason }
GepaValidationStarted
GepaValidationCompleted { candidate_index, average_score, case_count, metric_calls_new }
GepaParetoUpdated { changed_case_count, frontier_candidate_count }
GepaBudgetUpdated { metric_calls_spent, delta, remaining }
GepaOptimizationEnded { best_index, total_metric_calls }
```

P8 may render these as terse stderr lines, but the durable event/report must be
structured enough to reconstruct the run without tailing logs.

## 16. Reflection Dataset Detail

### 16.1 Optimize-Anything SideInfo Shape

For optimize-anything, evaluator returns either:

```text
score
```

or:

```text
(score, side_info)
```

The AIME example returns:

```text
score
input
prompt
output
reasoning
execution_feedback
```

Leaven AIME parity should render at least:

```text
Input: problem text
Generated Output: parsed/generated answer text
Reasoning: solver reasoning if the runner captures it
Score: 0.0 or 1.0
Feedback: scorer feedback, including reference-solution-derived text when policy emits it
```

If Leaven's Rust-native solver does not expose a separate reasoning field, the
report must label that prompt/trace delta. A better Leaven path is to preserve
both the raw assistant content and parsed answer, then project both.

### 16.2 DSPy Trace Shape

DSPy reflective examples come from predictor traces:

```text
trace item = (Predictor, predictor_inputs, predictor_prediction)
```

For the selected predictor:

- filter trace items by matching predictor signature;
- unless `add_format_failure_as_feedback`, discard failed predictions;
- if any failed prediction remains, select it;
- otherwise choose one matching trace item by RNG;
- stringify predictor inputs;
- stringify predictor outputs;
- if output failed parsing, feedback is output-format instructions;
- otherwise feedback comes from the predictor feedback function;
- if predictor-level score differs from module score, warn and keep module
  score for GEPA scoring.

Leaven needs this behavior for DSPy-profile parity. The typed equivalent is a
trace projection trait that can select `TraceRef`s for a surface part.

### 16.3 Prompt Rendering Details

Upstream sample rendering turns nested dicts/lists into markdown headings.
Leaven's current renderer uses `# Example`, `## Input`, `## Score`, `## Output`,
and `## Feedback`. That is acceptable for core AIME optimize-anything parity if
the content is equivalent, but DSPy trace parity should preserve `Inputs`,
`Generated Outputs`, and `Feedback` labels because those are part of the DSPy
reflection prompt shape.

## 17. AIME Reproduction Detail

### 17.1 Dataset

Reference loader:

```text
load AI-MO/aimo-validation-aime, config default, split train
for each item:
    input = item["problem"]
    solution = item["solution"]
    answer = item["answer"]
shuffle with random.Random(0)
train = first half
validation = second half

load MathArena/aime_2025, config default, split train
for each item:
    input = item["problem"]
    answer = item["answer"]
test = all rows
```

The live report must include:

- train count;
- validation count;
- test count;
- source dataset names/configs/splits;
- materialized cache file hash;
- split seed `0`;
- whether the test set was repeated. The current `examples/aime_math/utils.py`
  path does not repeat test rows; another helper under `src/gepa/examples/aime.py`
  multiplies AIME 2025 by 5. Reports must say which denominator is used.

### 17.2 Solver

Reference solver:

```text
dspy.LM("gpt-4.1-mini", temperature=1.0, max_tokens=32000)
dspy.ChainOfThought(MathSolverSignature)
MathSolverSignature.input: The math problem to solve.
MathSolverSignature.answer: The final numerical answer.
```

Leaven Rust-native solver is allowed, but strict AIME optimize-anything parity
requires matching:

- system/developer/user prompt shape or a documented delta;
- temperature 1.0;
- max output tokens 32000;
- final parsed answer field;
- raw reasoning or assistant content capture;
- provider/model fingerprint.

### 17.3 Scorer

Reference scorer:

```text
correct_answer = int(example.answer)
try llm_answer = int(prediction.answer)
except:
    score = 0
    feedback = final answer must be valid integer; includes correct answer and solution suffix if available
else:
    score = float(correct_answer == llm_answer)
    feedback = "Your answer is correct/incorrect. The correct answer is ..."
    append full step-by-step solution when available
```

This means the train/validation scorer feedback intentionally exposes the
correct answer and written solution to reflection through feedback text. That is
allowed because it is scorer-produced optimizer-visible feedback. The raw target
must still not be read directly by the reflector.

For held-out test reporting, feedback may be computed for scoring/reporting, but
test feedback must not enter the search loop.

### 17.4 Reflection Model

Strict upstream AIME optimize-anything profile currently uses:

```text
reflection_lm = "openai/gpt-5.1"
```

Leaven's intended stronger proposal model profile may use:

```text
reflection_lm = "gpt-5.4-mini", reasoning = medium
```

That run should be labeled:

```text
reference-aime-gepa algorithm + Leaven stronger reflection model
```

It should not be labeled as strict model parity.

## 18. Leaven Abstraction Changes Required

### 18.1 `leaven-gepa`

Required changes:

- add `GepaReferenceState` with candidate index table and validation frontier
  maps;
- make `initialize` perform seed validation and state initialization;
- make default candidate selector read `GepaReferenceState`, not generic
  `Pop::best()`;
- keep `SelectBestCandidate` as an explicit ablation;
- add skip-perfect and no-reflective-example skip policies;
- add GEPA-specific events;
- add detailed checkpoint state for new reference state;
- add profile constructors or builder presets;
- add merge slot as explicit scaffold or behavior-bearing implementation with
  honest profile labels.

Potential module split:

```text
state.rs              GepaReferenceState, candidate records, checkpoint shape
frontier.rs           validation frontier update and weighted sampling helpers
events.rs             GEPA strategy events and report summaries
cache.rs              GEPA per-case cache adapter if not engine-owned
profiles.rs           reference/dspy/aime/leaven-plus presets
merge.rs              merge scheduler and proposer when implemented
```

Keep `lib.rs` as a map.

### 18.2 `leaven-engine`

Required changes:

- support per-case partial cache hits for independent per-case evaluations;
- report new metric calls separately from returned assessment row count;
- preserve original case order when mixing cache hits and misses;
- expose enough cache-row detail for GEPA progress and reports;
- keep graph mutation behind `RunContext`;
- keep engine free of GEPA-specific strategy policy.

### 18.3 `leaven-run`

Required changes:

- surface GEPA detailed results without forcing ordinary users to learn graph
  internals;
- distinguish search metric-call budget from final report evaluations;
- report evaluation-cache and LM-cache summaries by role;
- include profile labels and model deltas in P8 reports.

### 18.4 `leaven-population`

Possible changes:

- expose reusable dominance/frequency helpers if they are optimizer-neutral;
- keep GEPA-specific parent selector and candidate-index report state in
  `leaven-gepa`.

Do not move GEPA state into `leaven-population`.

### 18.5 `examples/p8_aime_gepa`

Required changes:

- remove train-filtered population from ordinary reference profile;
- install `Gepa::aime_reference()` or equivalent explicit preset;
- print profile, upstream snapshot, and model-delta labels;
- print dataset hash/counts;
- print GEPA progress from strategy events;
- preserve `AimePrompt::cache_identity`;
- preserve target-safe reflective dataset projection;
- include strict and Leaven-plus profile knobs.

## 19. Minimal Test Matrix That Proves Replicability

These are not optional polish tests. Without them, future implementers will
still need to re-read Python.

### 19.1 Seed And Frontier Tests

```text
test_seed_full_validation_runs_before_train_minibatch
test_seed_validation_initializes_candidate_zero
test_seed_validation_initializes_per_case_frontier
test_no_iteration_starts_when_metric_budget_already_spent_by_seed_validation
```

### 19.2 Selector Tests

```text
test_pareto_selector_repeats_candidates_by_frontier_frequency
test_pareto_selector_removes_dominated_candidates
test_pareto_selector_is_rng_deterministic_after_restore
test_select_best_candidate_is_explicit_ablation_not_default
```

Construct a frontier like:

```text
case A: {candidate 0}
case B: {candidate 1}
case C: {candidate 1}
```

The sampling list should contain `[0, 1, 1]` after dominance pruning.

### 19.3 Loop Ordering Tests

```text
test_parent_and_child_screen_on_same_train_cases
test_acceptance_precedes_child_full_validation
test_rejected_child_does_not_enter_gepa_candidate_table
test_accepted_child_updates_validation_frontier_before_next_selection
```

### 19.4 Cache Tests

```text
test_gepa_per_case_cache_reuses_overlapping_minibatches
test_gepa_full_validation_hits_previously_cached_cases
test_cache_hit_does_not_increment_metric_calls
test_missing_candidate_cache_identity_bypasses_with_reported_reason
test_resume_restores_cache_and_does_not_repeat_seed_validation
```

### 19.5 Reflection Tests

```text
test_no_reflective_examples_skips_lm_call
test_all_perfect_minibatch_skips_lm_call
test_reflection_prompt_matches_upstream_template_snapshot
test_fenced_parser_matches_upstream_extractor_cases
test_aime_reflection_examples_include_input_output_score_feedback
test_aime_reflection_examples_exclude_raw_target_except_scorer_feedback
test_dspy_trace_projection_prefers_failed_prediction_when_enabled
```

### 19.6 Result/Report Tests

```text
test_gepa_detailed_result_contains_candidate_indices_and_lineage
test_gepa_detailed_result_contains_val_subscores_and_frontier_membership
test_gepa_report_names_reference_profile_and_model_deltas
test_p8_live_report_distinguishes_search_and_final_report_metric_calls
```

## 20. Review Checklist

Use this checklist before calling a Leaven run "real GEPA":

- Did seed full validation happen before iteration 1?
- Are parent candidates selected from validation/Pareto frontier state?
- Is the default selector frequency-weighted and stochastic under stored RNG?
- Are train minibatches used only for reflection and acceptance?
- Are accepted candidates full-validated before entering the GEPA candidate
  table?
- Does the next iteration see the accepted candidate through validation
  frontier state?
- Are cache hits counted as zero new metric calls?
- Does overlapping per-case evaluation cache actually hit?
- Does reflection use the upstream fenced replacement prompt by default?
- Are skip-perfect and no-trajectory/no-example cases handled before LM calls?
- Is merge setting correct for the claimed profile?
- Does the final result expose enough candidate/frontier detail to compare with
  upstream `GEPAResult.to_dict()`?
- Does the report disclose every intentional model, prompt, dataset, or cache
  delta?

If any answer is "no", the run can still be useful, but it is a Leaven GEPA
variant, not real GEPA parity.
