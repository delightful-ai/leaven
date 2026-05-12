# Layer 3 Engine/Eval Vision Comparison

Status: canonical Layer 3 original-vision comparison.

This doc compares the original optimizer-author/engine/eval vision against
current repo reality. It is area-specific to Layer 3 and does not rely on parent
synthesis as central truth.

## Summary

The current code has many of the right nouns: `Optimizer<P>`, `RunContext`,
`EvaluationRequest`, `ResolvedEvaluationRequest`, `Proposer<P>`,
`Evaluator<P>`, `RunGraphView`, `EvidenceStore`, `TrustPolicy`,
`EvaluationCache`, `Dataset`, `DatasetSplits`, and `SplitUsePolicy`.

The mismatch is that those nouns do not yet carry all required laws end to end.
Optimizer authors can still choose public APIs that bypass finalization; eval
lowering is not complete enough to stop local split/report semantics; trust is
expression-based instead of resolved-membership based; cache hits can break graph
truth; and evidence exists in the graph/store but is not honestly consumable by
reflective proposers. The high-level score facade also currently collapses too
much public scoring into scalar casewise evidence, which can mislead future
engine/eval work into treating one ergonomic path as the optimizer substrate.

## Original Layer 3 Vision

### Optimizer Authors Are First-Class

Vision:

Layer 3 users implement `Optimizer<P>` and drive the run through
`RunContext`: `docs/specs/initial_library.md:1793-1916`.
The optimizer owns algorithm rhythm, while the engine owns graph, budget, trust,
cache, events, reports, and persistence. The GEPA public/private spec preserves
the full substrate for optimizer authors:
`docs/specs/gepa_public_private_surface.md:539-579`.

Current reality:

`Optimizer<P>` exists and is async over `RunContext`:
`crates/leaven-engine/src/stage/optimizer.rs:9-24`. `Engine::run` constructs a
fresh `RunContext` for initialize and every step:
`crates/leaven-engine/src/engine.rs:60-114`. That part matches the vision.

Gap:

The same surface exposes non-finalizing raw contexts:
`crates/leaven-engine/src/context/run_context.rs:286-325`. Tests use those raw
contexts directly: `crates/leaven-engine/tests/stage_trait_contracts.rs:17-98`.
This means optimizer authors are first-class in type names, but not yet safe by
default in public API.

Correction:

Keep optimizer authors powerful, but make public power finalizing. Raw contexts
are implementation details passed into stages by finalizers, not objects Layer 3
users construct to run stages themselves.

Required proof:

An external optimizer crate can implement a custom optimizer using finalizing
`RunContext` APIs only, and cannot accidentally call raw dispatch paths.

### The Engine Is Dumb; Optimizers And Strategies Are Smart

Vision:

The engine coordinates the loop, manages the graph, dispatches strategies, and
bounds execution; decisions live in strategies:
`docs/specs/guiding_principles.md:301-309`. Every load-bearing loop decision is
swappable without forking the engine:
`docs/specs/guiding_principles.md:127-139`.

Current reality:

`Engine::run` is small and delegates to optimizer `initialize`, `step`, and
`best_candidate`: `crates/leaven-engine/src/engine.rs:60-149`.
`Proposer<P>`, `Evaluator<P>`, `Population<P>`, and `PreferenceRelation<P>`
exist as stage/capability traits:
`crates/leaven-engine/src/stage/proposer.rs:22-46`,
`crates/leaven-engine/src/stage/evaluator.rs:11-33`,
`crates/leaven-engine/src/stage/population.rs:8-38`,
`crates/leaven-engine/src/stage/preference.rs:8-17`.

Gap:

The strategy traits exist, but some critical strategy use cases are incomplete:
render/materialize are not finalizing engine work; `PopulationView` is still an
empty shell; proposer evidence access is unresolved; and GEPA uses a local
surface proposer instead of the graph-aware proposer path:
`crates/leaven-engine/src/context/proposal_context.rs:8-62`,
`crates/leaven-gepa/src/proposer.rs:6-56`,
`crates/leaven-gepa/src/optimizer.rs:536-594`.

Correction:

Seal the generic substrate first. GEPA should become one optimizer value using
the same finalizers, evidence, trust, budget, and proposal/provenance contracts
as any other optimizer.

Required proof:

Before GEPA is the product proof, a non-GEPA pairwise tournament optimizer must
compile and run over public Layer 3 primitives.

### Eval, Dataset, And Environment Are Separate Concepts

Vision:

User alignment explicitly says evals are not always datasets or environments:
`reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:31-49`.
The eval lowering spec formalizes the split:
user input, lowered eval data, execution, and environment are separate layers:
`docs/specs/eval_lowering_detail.md:24-37`. `leaven-eval` owns lowered data and
report vocabulary, while engine execution and workspaces stay elsewhere:
`docs/specs/eval_lowering_detail.md:49-65`.

Current reality:

`leaven-eval` correctly states it does not execute evaluations:
`crates/leaven-eval/src/lib.rs:1-5`. It has real dataset/split/use-policy starts:
`crates/leaven-eval/src/dataset.rs:29-115`,
`crates/leaven-eval/src/split.rs:59-150`,
`crates/leaven-eval/src/use_policy.rs:85-123`.
`leaven-run` currently constructs datasets, splits, engine case sets, trust
policy, and split reports locally:
`crates/leaven-run/src/builder.rs:214-240`,
`crates/leaven-run/src/builder.rs:321-355`,
`crates/leaven-run/src/builder.rs:580-605`.

Gap:

The planned eval module graph is not implemented. `plan`, `request`, `traits`,
and `suite` are missing from the current module map:
`docs/specs/eval_lowering_detail.md:101-145`,
`crates/leaven-eval/src/lib.rs:7-19`. Without those modules, there is no
canonical implementation home for evaluation plans, request templates, suites,
and complete report schemas. Builder-local lowering is already becoming the
hidden product law for split names, trust, final reports, and unresolved
request-shape-based report inference.

Correction:

Complete `leaven-eval` as lowered vocabulary only. Do not move evaluator
execution or environment semantics into it. Do not let GEPA or product builders
hide split/report laws in local code.

Required proof:

An eval suite can lower to engine `CaseSet`, `EvaluationRequest`, and
`TrustPolicy` values while `leaven-eval` keeps its dependency boundary clean,
and split reports derive from resolved split/eval truth rather than only from
the syntactic `EvaluationSet::Partition` form.

### Evidence Is Shape-Neutral And Separate From Preference

Vision:

Evaluation may produce scalar, multi-axis, pairwise, listwise, or mixed
evidence, and preference is a separate relation over evidence:
`docs/specs/guiding_principles.md:114-125`. The core assessment model mirrors
request shape and carries opaque `P::Evidence`:
`crates/leaven-core/src/evaluation.rs:354-405`.

Current reality:

`EvaluationRequest` has independent, pairwise, and listwise shapes:
`crates/leaven-core/src/evaluation.rs:170-224`. `ResolvedEvaluationRequest`
preserves shape, set, granularity, and purpose:
`crates/leaven-core/src/evaluation.rs:226-243`. `leaven-evidence` has useful
casewise and pairwise starts:
`crates/leaven-evidence/src/casewise.rs:36-78`,
`crates/leaven-evidence/src/pairwise.rs:17-91`.

Gap:

`leaven-evidence` also root-exports empty placeholder evidence names:
`crates/leaven-evidence/src/lib.rs:1-77`. Engine `PreferenceRelation<P>` is
defined, but the minimum graph-backed preference/population proof is not yet the
center of the test contract:
`crates/leaven-engine/src/stage/preference.rs:8-17`,
`crates/leaven-engine/src/stage/population.rs:8-38`.

Correction:

Keep `P::Evidence` opaque in core. Promote only implemented standard evidence
shapes. Add graph-backed preference/population contract tests before relying on
GEPA population behavior as product evidence.

Required proof:

At least one scalar casewise path and one pairwise/tournament path update
optimizer-owned population/preference state through graph assessment IDs.

### Public Score Is A Facade, Not Engine Truth

Vision:

Ordinary users should be able to pass a scoring function, but score
normalization must preserve comparable axes, natural-language feedback,
attachments, metadata, and diagnostics until explicit projection:
`docs/specs/eval_lowering_detail.md:315-343`,
`docs/specs/gepa_public_private_surface.md:1126-1155`. Typed evaluators remain
the power-user path when the task is pairwise, listwise, batch-shaped, or
domain-specific: `docs/specs/gepa_public_private_surface.md:924-933`.

Current reality:

`RunProblem` fixes `P::Evidence` to `CasewiseEvidence<ScoredFeedbackEvidence>`:
`crates/leaven-run/src/builder.rs:43-51`. `Score` is only `value: f64`,
`feedback: String`, and structured string pairs:
`crates/leaven-run/src/evidence.rs:23-32`. `ScoreContext` is public fields over
artifact, case, and output:
`crates/leaven-run/src/evidence.rs:46-54`. `ScoringEvaluator` requires per-case
independent requests and lowers every result into scalar feedback evidence:
`crates/leaven-run/src/evaluator.rs:65-128`.

Gap:

The public score path is useful scaffolding, but it is currently narrower than
the original score/reward contract and narrower than the Layer 3 evidence
substrate. If future engine/eval work treats this as internal truth, pairwise
tournaments, multi-axis metrics, attachment-heavy agent traces, and
feedback-only diagnostics will be forced into scalar averages.

Correction:

Keep `.score(...)` as a Layer 1 convenience that lowers into richer evidence and
report data. Layer 3 APIs must continue to center `Assessment<P>`, opaque
`P::Evidence`, `PreferenceRelation<P>`, and `Population<P>`, not `Score`.

Required proof:

A scalar `.score(...)` smoke remains short, but a rich score with feedback,
metric axes, metadata, and attachments lowers without losing durable references;
pairwise/listwise evaluators bypass `.score(...)` and still fit naturally
through the same engine graph/evidence/report contracts.

### Stage Neutrality Includes Agentic And Costful Stages

Vision:

A stage can be an LM call, typed pipeline, deterministic algorithm, or full
agent in a sandbox; the framework cannot assume it is fast, deterministic,
in-memory, side-effect-free, or token-bounded:
`docs/specs/guiding_principles.md:150-154`. Stages are async by default:
`docs/specs/guiding_principles.md:229-231`. The renderer/materializer split is
what lets small LM prompt cases and large agentic workspaces use the same
machinery: `docs/specs/guiding_principles.md:104-112`.

Current reality:

`Proposer<P>`, `Evaluator<P>`, `Renderer`, and `Materializer` are async-capable:
`crates/leaven-engine/src/stage/proposer.rs:27-46`,
`crates/leaven-engine/src/stage/evaluator.rs:27-33`,
`crates/leaven-engine/src/stage/renderer.rs:8-26`.

Gap:

Render/materialize do not have public engine finalizers:
`crates/leaven-engine/src/context/run_context.rs:312-325`. Materializer context
cannot charge directly and tests call materializers directly:
`crates/leaven-engine/src/context/materialize_context.rs:8-50`,
`crates/leaven-engine/tests/materializer_contract.rs:25-47`.

Correction:

Add render/materialize finalizers and tests. Make agentic-style materialization
observable, budgeted, and read-scoped before using it as proof that the engine
supports agents.

Required proof:

A materializing stage with nonzero cost produces workspace output while central
budget, event stream, and hidden evidence rules all hold.

### Trust Separation Is A Real Engine Law, Not A Public Burden

Vision:

Trust separation for agentic stages is not optional:
`docs/specs/guiding_principles.md:186-193`. Public docs should keep actor/trust
language out of ordinary user surfaces and in lowered/engine sections:
`docs/specs/gepa_public_private_surface.md:877-890`.

Current reality:

`TrustPolicy` and `ReadScope` exist:
`crates/leaven-engine/src/trust.rs:8-40`. Engine run and evaluate paths attach
trust policy to contexts:
`crates/leaven-engine/src/engine.rs:72-80`,
`crates/leaven-engine/src/engine.rs:96-105`,
`crates/leaven-engine/src/engine.rs:161-168`.

Gap:

Trust is checked on unresolved expressions, while explicit cases and dynamic
sets can avoid hidden-partition detection:
`crates/leaven-engine/src/trust.rs:119-182`,
`crates/leaven-engine/src/case_set.rs:64-70`. Current tests bless explicit cases
as okay under hidden policy: `crates/leaven-engine/tests/trust_policy.rs:113-130`.

Correction:

Move enforcement to resolved membership and split-use rules. Keep trust in
engine/lowered docs, but make it impossible for Layer 3 users to bypass by
choosing a different `EvaluationSet` syntax.

Required proof:

Hidden case IDs are rejected after resolution regardless of request syntax.

### Cache And Reports Must Preserve Graph Truth

Vision:

Cache correctness depends on content identity and evaluator semantics:
`docs/specs/guiding_principles.md:213-217`. Cache is default off for stochastic
evaluators and deterministic only with explicit evaluator fingerprint and policy:
`docs/specs/initial_library.md:4718-4720`. Reports point at graph truth:
`docs/specs/initial_library.md:1984-1996`.

Current reality:

`CachePolicy::Never` is default:
`crates/leaven-engine/src/cache.rs:9-21`. Evaluator fingerprint and policy are
part of the trait:
`crates/leaven-engine/src/stage/evaluator.rs:11-25`. Evaluation reports are
ID-only: `crates/leaven-engine/src/reports.rs:47-54`.

Gap:

`EvaluationCacheKey` omits request semantics:
`crates/leaven-engine/src/cache.rs:46-59`. Cache hits return old assessment IDs
for new requests without graph-visible reuse lineage:
`crates/leaven-engine/src/context/run_context.rs:537-568`.

Correction:

Make cache key semantics complete and make cache-hit reuse explicit in graph
truth, events, and reports.

Required proof:

No semantically distinct request can share a cache entry accidentally, and every
cache hit is visible as reuse rather than fresh assessment ownership.

### GEPA Is One Optimizer, Not The Whole Library

Vision:

GEPA is one optimizer value:
`docs/specs/initial_library.md:3312-3330`.
The optimizer-author path stays unchanged and should let custom optimizers own
their rhythm through `RunContext`:
`docs/specs/gepa_optimizer_surface.md:89-103`. The target literature set
requires GEPA, GEPA+merge, MIPRO, TextGrad, pairwise tournament, agentic skill
optimization, and other patterns without engine rewrites:
`docs/specs/guiding_principles.md:156-178`.

Current reality:

`leaven-gepa` has a real optimizer shape and checkpoint state starts, but its
proposal path calls GEPA-local `SurfaceProposer` and manual batch recording:
`crates/leaven-gepa/src/optimizer.rs:536-594`. Its public root exports fixed
fixtures and placeholders beside production-looking names:
`crates/leaven-gepa/src/lib.rs:13-25`,
`crates/leaven-gepa/src/proposer.rs:21-56`.

Gap:

If GEPA is fixed before Layer 3 is sealed, GEPA will likely grow another local
reflection/evidence/cache path. That would prove GEPA-ish behavior while failing
the optimizer-author substrate.

Correction:

Gate trusted GEPA restoration on P0-P8 in `fix-priority-map.md`.

Required proof:

GEPA uses the same finalizers, evidence/reflection contract, trust, cache,
budget, and graph-report semantics that a non-GEPA optimizer uses.
