# Engine/Eval Original Vision Refinement

## Short Answer

No. The engine/eval substrate is not aligned with the original Leaven vision yet.

The first-pass Layer 3 audit correctly identifies several invariant-bypass paths, especially raw stage contexts, missing proposer evidence access, hidden-split bypasses, and unsafe cache keys. But against the original vision, those are symptoms of a deeper contract gap: engine authors and optimizer authors were promised a substrate where `RunContext` finalizes all costful stage work, evaluation/data/environment concepts stay separate, evidence remains distinct from preference and population state, and GEPA is just one optimizer using the same engine/eval contracts that other optimizers can trust.

Today the code has many of the right nouns. It does not yet make those nouns carry the required laws end to end.

## Findings

### EEOV-001: `RunContext` is not yet the only public finalization boundary

- `id`: `EEOV-001`
- `severity`: blocker
- `vision promise`: The original engine contract says optimizer authors implement `Optimizer<P>` and do real work through `RunContext`; context methods handle graph writes, budget charges, cache lookup, callbacks, trust policy, error normalization, event metadata, and persistence hooks. `RunContext` is what lets optimizer authors avoid reimplementing the engine.
- `current audit coverage`: The first pass correctly records this as `L3-001` in `stage-contexts.md`, `L3-008` in `run-context-and-graph.md`, and the top finding in `agent-report-layer-3.md`.
- `gap`: The audit frames this mostly as "raw context factories are too public." The deeper mismatch is that the public substrate currently has two classes of stage APIs: finalizing `RunContext` APIs and legitimate-looking non-finalizing context APIs. That violates the vision-level law that engine users should not have to know which path preserves graph, cost, evidence, cache, trust, and events.
- `correction`: Hard cut to one public finalizing path per costful operation. Raw `ProposalContext`, `EvaluationContext`, `RenderContext`, and `MaterializeContext` construction should be crate-private, test-support-only, or explicitly named non-finalizing internals. Public optimizer-author docs should say: call `RunContext::propose`, `RunContext::evaluate`, `RunContext::render_with`, `RunContext::materialize_into`, or equivalent finalizers.
- `evidence`:
  - `docs/specs/initial_library.md:1793-1816` defines optimizer authors around `Optimizer<P>` and `RunContext`.
  - `docs/specs/initial_library.md:1827-1916` lists `RunContext` methods and states that context methods handle graph writes, budget, cache, callbacks, trust, errors, events, and persistence.
  - `docs/specs/initial_library.md:1923-1937` distinguishes stage contexts by capability, not by finalization authority.
  - `crates/leaven-engine/src/context/run_context.rs:191-208` has a finalizing `propose`.
  - `crates/leaven-engine/src/context/run_context.rs:344-440` and `442-500` have finalizing static/dyn evaluation paths.
  - `crates/leaven-engine/src/context/run_context.rs:286-325` publicly exposes raw proposal, evaluation, render, and materialize contexts.
  - `crates/leaven-engine/tests/stage_trait_contracts.rs:17-35` and `64-98` call dyn proposer/evaluator paths directly with raw contexts.

### EEOV-002: `ProposalContext` cannot support original reflective optimizer semantics

- `id`: `EEOV-002`
- `severity`: blocker
- `vision promise`: Strategy swappability includes how proposals and reflection are produced, and what feedback the proposer/reflector consumes via rendering. Proposers build rich views from `ctx.graph()` while keeping requests owned and lightweight. GEPA reflection needs parent, selected part, scored feedback, trace/evidence, objective/background, and provenance.
- `current audit coverage`: The first pass correctly records that `ProposalContext` can see evidence refs through graph views but cannot load scoped payloads (`agent-report-layer-3.md` `L3-002`, `evidence-trust-budget-cache.md` `L3-004`). The known findings also record that GEPA uses a local `SurfaceProposer` and fixed `ReflectiveMutation` path.
- `gap`: The audit leaves the correction as an either/or: add evidence loading to `ProposalContext`, or preload evidence into proposer requests. The original vision requires a more explicit contract: someone must own evidence selection, someone must own rendering/materialization, and `informed_by` provenance must be recorded without giving the proposer graph mutation authority. Without that, GEPA and any trace-aware optimizer will keep inventing local reflector contracts.
- `correction`: Define the reflector/proposer contract before fixing GEPA. Minimum shape: optimizer selects parent/part/minibatch/assessments; renderer/materializer produces allowed feedback and trace views; proposer receives an owned request plus scoped context for budget/render helpers; `RunContext` finalizes proposals and records causal and `informed_by` refs. If `ProposalContext` gets evidence loading, it must enforce `ReadScope::visible_evidence` and hidden partitions.
- `evidence`:
  - `docs/specs/guiding_principles.md:127-139` makes proposal production, reflection, and consumed feedback swappable strategy decisions.
  - `docs/specs/initial_library.md:2172-2198` defines `Proposer<P>` and says rich views are constructed inside `propose` from `ctx.graph()`.
  - `docs/specs/gepa_public_private_surface.md:521-533` lists the minimum GEPA strategy-slot contracts, including reflector/proposer, population, and merge.
  - `docs/specs/gepa_public_private_surface.md:892-933` makes `ScoreContext` the public trace/state object and requires feedback/evidence attachment support.
  - `crates/leaven-engine/src/context/proposal_context.rs:8-62` exposes graph, read scope, budget, render context, and materialize context, but no evidence reader.
  - `crates/leaven-engine/src/context/run_context.rs:640-653` keeps typed evidence payload loading on `RunContext`.
  - `crates/leaven-gepa/src/proposer.rs:6-19` defines GEPA-local `SurfaceProposer` with only artifact, surface, and part.
  - `crates/leaven-gepa/src/proposer.rs:21-48` names a fixed edit fixture `ReflectiveMutation`.

### EEOV-003: Eval, dataset, and environment separation is specified but not implemented tightly enough

- `id`: `EEOV-003`
- `severity`: high
- `vision promise`: Evals are first-class, but evals are not datasets and not environments. `leaven-eval` owns lowered dataset/split/use/report vocabulary. `leaven-engine` executes evaluator calls. Agentic/workspace/environment semantics stay in agentic and workspace crates. Ordinary users should see train/validation/test cases, runner, score/evaluator, optimizer, and budget, not an implementation-facing "evaluation spec."
- `current audit coverage`: The first pass catches hidden split bypasses and some Layer 1 leakage, but it does not make the eval/data/environment split a central engine/eval finding.
- `gap`: The implementation has a partial `leaven-eval` crate, but it lacks the spec's `plan`, `request`, `suite`, and adapter-trait modules. `leaven-run` hardcodes dense case IDs, split construction, `CaseSet` creation, trust policy, final evaluations, and report assembly directly in the builder. That may be acceptable as an early slice, but it is not yet the maintainable lowered eval layer promised by the vision.
- `correction`: Make `leaven-eval` the single lowered data/report vocabulary: `Dataset`, `DatasetSplits`, `SplitUsePolicy`, `EvaluationPlan`, `EvaluationRequestTemplate`, `EvaluationSuite`, and report schemas. Keep execution in engine. Keep environment/task/workspace semantics outside eval. Make `leaven-run` lower public inputs once into those values, then into engine `CaseSet` and `TrustPolicy`; do not let builder-local split conventions become the hidden product law.
- `evidence`:
  - `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:29-49` records the requirement for train/test/validation and "evals != datasets != environments."
  - `docs/specs/eval_lowering_detail.md:24-37` defines user input, lowered eval data, execution, and environment as separate layers.
  - `docs/specs/eval_lowering_detail.md:49-65` says `leaven-eval` owns dataset/splits/split-use/report vocabulary and does not own evaluator execution, workspaces, optimizer rhythm, or domain case semantics.
  - `docs/specs/eval_lowering_detail.md:101-145` specifies the planned `leaven-eval` module graph and public re-exports, including `plan`, `request`, `suite`, and `traits`.
  - `crates/leaven-eval/src/lib.rs:7-19` currently exports only dataset, error, report, split, and use-policy modules.
  - `crates/leaven-run/src/builder.rs:214-240` builds dataset/splits/case set/trust policy directly inside `.run()`.
  - `crates/leaven-run/src/builder.rs:321-355` hardcodes `TRAIN`, `VALIDATION`, and `TEST` split construction.

### EEOV-004: Current scoring lowers too early into scalar casewise evidence

- `id`: `EEOV-004`
- `severity`: high
- `vision promise`: Evidence-shape neutrality is a hard requirement. Evaluation may produce scalar, multi-axis, pairwise, listwise, or mixed evidence. Preference is a separate relation over evidence; population/frontier state is optimizer-owned state. Public `.score(...)` can be ergonomic, but it must lower into assessment/evidence/preference without collapsing the library into one scalar score path.
- `current audit coverage`: The first pass records public evidence placeholders and GEPA context loss, but it does not fully call out that the product scoring facade currently narrows the whole public builder to scalar casewise evidence.
- `gap`: `RunProblem` fixes `P::Evidence` to `CasewiseEvidence<ScoredFeedbackEvidence>`, `Score` is only `f64 + feedback + structured text pairs`, `ScoreContext` is public fields over artifact/case/output, and report assembly averages scalar case scores. This is much weaker than the promised score facade with comparable axes, natural language, structured records, attachments, metadata, typed failures, and metered cost.
- `correction`: Treat `Score` as a Layer 1 facade only, not core truth. It should be rich enough to preserve primary comparable score, metric axes/directions, feedback, attachments/evidence refs, metadata, typed errors, and scorer cost. Lower it into graph assessments and evidence records. Acceptance, population updates, and reports should consume declared comparable axes or explicit preference relations, not implicit `f64` averages.
- `evidence`:
  - `docs/specs/guiding_principles.md:114-125` requires scalar, multi-axis, pairwise, listwise, and mixed evidence, with preference separate from evidence.
  - `docs/specs/eval_lowering_detail.md:315-343` says score normalization must preserve evidence refs, attachments, metadata, diagnostics, and comparable score axes.
  - `docs/specs/gepa_public_private_surface.md:1120-1179` defines a richer `Score` shape and attachment rules.
  - `crates/leaven-run/src/builder.rs:48-51` fixes `RunProblem::Evidence` to casewise scored feedback.
  - `crates/leaven-run/src/evidence.rs:23-32` defines `Score` as `value: f64`, `feedback: String`, and structured string pairs.
  - `crates/leaven-run/src/evidence.rs:46-54` exposes `ScoreContext` as public fields over artifact, case, and output only.
  - `crates/leaven-run/src/evaluator.rs:97-128` runs the sync runner/scorer and normalizes to scalar casewise feedback evidence.
  - `crates/leaven-run/src/builder.rs:439-491` and `607-620` build reports by reading scalar-style summaries.

### EEOV-005: Trust policy is partition-expression based, not split-use/evidence-use safe

- `id`: `EEOV-005`
- `severity`: high
- `vision promise`: Validation/test data must be held out by default. Split-use policy says which split may drive proposer feedback, parent selection, part selection, acceptance, population observation, report, evaluator-only use, and final test. Engine trust/read scopes are the enforcement layer behind product lowering.
- `current audit coverage`: The first pass correctly catches the explicit-case-ID bypass (`L3-005`) and cites the eval lowering spec's warning.
- `gap`: The deeper issue is that split-use and evidence-use are not yet engine-enforced laws. `TrustPolicy` checks whether an `EvaluationSet` expression references hidden partitions, but `EvaluationSet::Cases`, `Tagged`, and `Recent` do not map back to partition membership. `leaven-run` hides validation/test only from proposers by default. The engine does not check `EvaluationPurpose` against `SplitUsePolicy`, and reports infer split from the unresolved request expression.
- `correction`: Enforce trust after resolution, not only before. `CaseSet` or resolved sets need partition-membership metadata so explicit case IDs and dynamic sets can be checked against hidden partitions. Product lowering must turn `SplitUsePolicy` into engine-readable trust/read/evidence rules. Reports should cite graph/resolved-set truth and policy summaries, not infer split role from only `EvaluationSet::Partition`.
- `evidence`:
  - `docs/specs/eval_lowering_detail.md:344-397` defines split-use policy and says actor/read enforcement is lowered into engine `TrustPolicy`/`ReadScope`.
  - `docs/specs/eval_lowering_detail.md:675-678` warns product paths to use `EvaluationSet::Partition` until engine trust can map explicit case IDs back to hidden partitions.
  - `docs/specs/eval_lowering_detail.md:744-757` repeats the split invariant for final-test and explicit case IDs.
  - `crates/leaven-engine/src/trust.rs:119-145` checks hidden partitions on the unresolved request.
  - `crates/leaven-engine/src/trust.rs:154-182` treats `Cases`, `Tagged`, `Recent`, and `Unscoped` as non-partition references.
  - `crates/leaven-engine/src/case_set.rs:64-70` resolves explicit case IDs by checking existence only.
  - `crates/leaven-run/src/builder.rs:233-240` hides validation/test from proposers but not optimizers.
  - `crates/leaven-run/src/builder.rs:580-605` infers report split only from partition-shaped unresolved requests.

### EEOV-006: Evaluation cache can return assessment IDs that do not belong to the new request

- `id`: `EEOV-006`
- `severity`: high
- `vision promise`: Engine-owned evaluator caching must be semantically safe. Cache keys should identify evaluator behavior, resolved evaluation set, request shape, candidate identities, and semantics that affect assessment meaning. Reports point at graph truth; they do not invent a parallel truth.
- `current audit coverage`: The first pass correctly records that cache keys omit request shape, granularity, purpose, and pair-order semantics (`L3-006` / `L3-004` depending on file).
- `gap`: There is a second mismatch: the engine records a new evaluation request before checking the cache, but a cache hit returns old `assessment_ids` without recording new assessment records for the new request. The report for request N can therefore carry assessment IDs whose graph records belong to request M. That breaks graph-backed report semantics even if the cache key were otherwise safe.
- `correction`: Cache hits need an explicit graph contract. Either record cache-hit assessment aliases/derived records for the new request, or make reports explicitly say "request N reused assessments from request M" with graph-visible linkage. Also expand `EvaluationCacheKey` to include resolved request kind, granularity, purpose if evaluator-visible, pair-order/symmetry, and assessment shape.
- `evidence`:
  - `docs/specs/initial_library.md:1984-1996` says `EvaluationReport` returns assessment IDs and reports point at graph truth.
  - `docs/specs/gepa_public_private_surface.md:558-579` says evaluator fingerprint/cache policy are part of the cache contract and returned assessment shape must match request shape.
  - `crates/leaven-engine/src/context/run_context.rs:398-421` resolves, builds cache key, records a new evaluation request, and can return cached report before evaluator execution.
  - `crates/leaven-engine/src/context/run_context.rs:537-568` returns cached assessment IDs with the new request id and zero cost, without recording new assessments for that request.
  - `crates/leaven-engine/src/cache.rs:46-59` defines a cache key that omits request kind, granularity, purpose, pair order, and assessment shape.
  - `crates/leaven-engine/src/context/run_context.rs:781-794` constructs keys from evaluator, policy, case-set version, case IDs, and candidate cache identities only.

### EEOV-007: Renderer and materializer are correctly split, but not yet enforceable as budgeted engine work

- `id`: `EEOV-007`
- `severity`: high
- `vision promise`: Rendering and materialization are separate because value-returning presentation and workspace side effects are different operations. Both can be async and costful. They must work for LM prompts, debug views, agent workspaces, sandboxed evaluators, traces, and large artifacts without bypassing budget/trust/event accounting.
- `current audit coverage`: The first pass records missing render/materialize finalizers as `L3-002` in `stage-contexts.md` and `L3-005` in `agent-report-layer-3.md`.
- `gap`: The current code has the split traits, but public callers can only obtain contexts and call the traits themselves. `MaterializeContext` carries a `BudgetSnapshot`, not a mutable stage budget handle, so materializers cannot charge through the central ledger via context. This is not just a missing convenience method; it makes materialized agentic stages invisible to budget enforcement unless every caller remembers to charge returned cost somewhere else.
- `correction`: Add `RunContext` finalizers for rendering and materialization. `MaterializeContext` should either carry a charge-capable handle or be used only inside a finalizer that charges returned `Metered` cost. Events should record at least stage id, target/value kind if available, cost, and failure.
- `evidence`:
  - `docs/specs/initial_library.md:67-69` splits `Renderer` and `Materializer`.
  - `docs/specs/initial_library.md:2257-2274` says rendering/materialization are async, costful, and intentionally different.
  - `docs/specs/gepa_public_private_surface.md:840-875` requires every spending stage to charge the central budget ledger.
  - `crates/leaven-engine/src/stage/renderer.rs:8-26` defines `Renderer` and `Materializer` returning `Metered`.
  - `crates/leaven-engine/src/context/run_context.rs:312-325` exposes raw render/materialize contexts but no finalizer.
  - `crates/leaven-engine/src/context/proposal_context.rs:46-62` and `crates/leaven-engine/src/context/evaluation_context.rs:46-62` build render/materialize contexts directly.
  - `crates/leaven-engine/tests/materializer_contract.rs:40-47` calls `materialize_into` directly.

### EEOV-008: Evidence, preference, and population are still too uneven to be the trusted optimizer substrate

- `id`: `EEOV-008`
- `severity`: high
- `vision promise`: Evidence is measurement. Preference interprets evidence. Population/frontier state is optimizer-owned live strategy state. GEPA and non-GEPA optimizers should be able to combine these concepts without collapsing everything into scalar scores or local ad hoc structs.
- `current audit coverage`: The first pass catches public placeholder evidence types (`X-001`) but does not fully tie evidence/preference/population readiness to the "minimum contract before GEPA" question.
- `gap`: `leaven-evidence` exports several empty public evidence names. `leaven-preference` includes mostly placeholder or scalar helper shapes rather than implemented graph-backed preference relations. `leaven-population` has useful concrete starts, but many root-exported names are skeletons, and concrete populations often expose local observe methods instead of implementing the engine `Population<P>` trait. This leaves GEPA tempted to bind directly to concrete scalar/casewise helper methods rather than a general evidence/preference/population contract.
- `correction`: Before trusting GEPA, establish the minimum standard contracts: real casewise scalar evidence; real pairwise evidence; one graph-backed scalar preference relation; one graph-backed pairwise/tournament preference or fitted population; one population implementation that observes graph assessment IDs through `RunGraphView`; and no root re-exported placeholder evidence/preference/population names as production vocabulary.
- `evidence`:
  - `docs/specs/guiding_principles.md:114-125` separates evidence shapes from preference relations.
  - `docs/specs/initial_library.md:70-71` states fitted preference relations live on `Population` impls and stateless preferences implement `PreferenceRelation`.
  - `docs/specs/initial_library.md:184-186` says population observation is optimizer-driven and engine records assessments into graph.
  - `crates/leaven-evidence/src/lib.rs:1-77` declares the evidence crate a skeleton and root-exports empty placeholder names.
  - `crates/leaven-engine/src/stage/preference.rs:8-17` defines the graph-backed `PreferenceRelation<P>` trait.
  - `crates/leaven-engine/src/stage/population.rs:8-38` defines the engine `Population<P>` trait.
  - `crates/leaven-preference/src/pareto.rs:1` is only `pub struct ParetoPreference;`.
  - `crates/leaven-population/src/pareto_frontier.rs:53-80` is a concrete casewise scalar frontier, not an implementation of the engine population trait.
  - `crates/leaven-population/src/keep_best.rs:48-84` observes direct `ScalarEvidence`, not graph assessment IDs through `Population<P>`.

### EEOV-009: Minimum GEPA trust bar is engine/eval contract first, GEPA strategy slots second

- `id`: `EEOV-009`
- `severity`: blocker
- `vision promise`: GEPA is one optimizer value, not a privileged engine path. The same substrate must support pairwise tournament, TextGrad, MIPRO, AlphaEvolve-style search, agentic skill optimization, and GEPA. The original implementation order put pairwise tournament before GEPA parity specifically to stress non-scalar, non-GEPA substrate.
- `current audit coverage`: The first pass correctly flags the current GEPA fixed-reflection and manual proposal recording. The integrated refinement docs already say Phase 3 should seal engine/eval invariants, but the implementation sequence currently lists "Restore GEPA" before "Seal Engine And Eval Invariants."
- `gap`: If GEPA is fixed before the engine/eval substrate is sealed, GEPA will likely get another local path: local reflector request, local evidence lowering, local cache assumptions, local population updates. That repeats the failure mode the audit exists to prevent.
- `correction`: The minimum contract before GEPA can be trusted is:
  1. `RunContext` finalizers are the only public way to run metered proposal/evaluation/render/materialize stages.
  2. Reflection has one evidence-selection/rendering/provenance contract.
  3. Hidden split trust is enforced after evaluation-set resolution.
  4. Cache hits preserve graph/request truth.
  5. `leaven-eval` owns lowered dataset/split/use/report vocabulary without executing evals or owning environments.
  6. Public `.score(...)` lowers rich score outputs without erasing evidence/preference/population distinctions.
  7. At least scalar casewise and pairwise evidence paths have working preference/population contract tests through graph assessment IDs.
- `evidence`:
  - `docs/specs/guiding_principles.md:40-50` defines the common optimizer skeleton and says variation is in strategy choices.
  - `docs/specs/guiding_principles.md:127-139` requires every load-bearing loop decision to be swappable without forking the engine.
  - `docs/specs/initial_library.md:85-90` explains that pairwise tournament was intentionally moved before GEPA parity because it stresses the new design.
  - `docs/specs/initial_library.md:3312-3320` states GEPA is one optimizer value.
  - `reviews/2026-05-11-fuckery-extermination-today/refinement/implementation-sequence.md:53-76` currently puts GEPA restoration before engine/eval invariant sealing.
  - `reviews/2026-05-11-fuckery-extermination-today/refinement/implementation-sequence.md:78-99` lists the engine/eval sealing work that should gate trusted GEPA.

## Refinement Edits Recommended

Update these integrated docs after folding agent reports:

1. `reviews/2026-05-11-fuckery-extermination-today/refinement/vision-comparison.md`
   - Add a dedicated engine/eval section stating that the first-pass audit is right but incomplete: the substrate is contract-substituted, not merely missing local fixes.
   - Promote cache-hit graph/request mismatch and score/evidence/preference collapse into the root diagnosis.

2. `reviews/2026-05-11-fuckery-extermination-today/refinement/surface-requirements.md`
   - Tighten Layer 3 requirements around public finalizers for proposal/evaluation/render/materialize.
   - Add the exact minimum engine/eval contract from `EEOV-009`.
   - Clarify that `Score` is a Layer 1 facade and cannot become the internal optimizer truth.

3. `reviews/2026-05-11-fuckery-extermination-today/refinement/implementation-sequence.md`
   - Move "Seal Engine And Eval Invariants" before or as a hard prerequisite to "Restore GEPA As One Honest Optimizer."
   - Add cache-hit graph-link correction and `leaven-eval` missing module/lowering completion to Phase 3.
   - Add evidence/preference/population contract tests to the Phase 3 exit criteria.

4. `reviews/2026-05-11-fuckery-extermination-today/refinement/open-design-questions.md`
   - Resolve or sharpen Q3 so it asks for the exact evidence-to-reflector contract, not just three possible options.
   - Add a question or decision record for cache-hit graph semantics.
   - Add a question or decision record for whether public `.score(...)` returns a rich `Score` facade only, with typed evaluator as the power-user route.

5. `reviews/2026-05-11-fuckery-extermination-today/internals/layer-3-engine-author/stage-contexts.md`
   - Fold `EEOV-001` and `EEOV-007` into the existing findings.
   - Make raw-context publicness a blocker for trusted optimizer-author APIs, not just a high-severity local smell.

6. `reviews/2026-05-11-fuckery-extermination-today/internals/layer-3-engine-author/evidence-trust-budget-cache.md`
   - Add the cache-hit assessment/request mismatch from `EEOV-006`.
   - Expand hidden split finding to include post-resolution split-use enforcement, not just explicit case IDs.

7. `reviews/2026-05-11-fuckery-extermination-today/internals/layer-3-engine-author/run-context-and-graph.md`
   - State the positive contract: graph truth is preserved only if all costful stage work has finalizing `RunContext` entrypoints and cache hits have graph-visible lineage.

8. `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/stub-placeholder-ledger.md`
   - Add or update entries for public evidence/preference/population placeholders and production-looking names that do not implement their promised laws.
