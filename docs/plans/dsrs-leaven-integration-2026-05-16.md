# DSRs Leaven Integration: Plan

## Goal

Make DSRs a real Leaven optimization target after DSRs PR #87 lands. The bridge lives in `DSRs/crates/dsrs-leaven`, implements Leaven capability traits directly, and proves its first usable milestone by matching native `dsrs-gepa` behavior on a deterministic AIME-style train/validation fixture, while the Leaven-driven path additionally reports post-optimization test behavior before any `dsrs-gepa` sunset. Predictor state serialization is a hard prerequisite, not a follow-up.

This is a cross-repo plan. It does not implement code and does not reintroduce Leaven’s orphan `crates/leaven-dsrs` path.

## Background

### User decisions captured up front

- Adopt DSRs PR #87’s ownership model: `dsrs-leaven` lives in the DSRs workspace and implements Leaven capability traits directly.
- Assume PR #87’s split crates land first; do not plan compatibility shims for the current monolithic `dspy-rs` tree.
- Target DSRs GEPA parity on an AIME-style example inspired by DSPy, not a small compile-only bridge.

### DSRs PR #87 bridge baseline

- DSRs PR #87 (`krypticmouse/DSRs#87`, head `codex/dsrs-crate-split`) is still open as of the refinement pass on 2026-05-16; current head `88d0c887628ab41c5dfef16cbcb0676d60c5593e` introduces split crates plus a compile-only `dsrs-leaven` scaffold.
- The split-design doc frames DSRs as “a thing leaven optimizes” and decides that DSRs implements Leaven capability traits directly in `dsrs-leaven`: `DSRs/docs/plans/2026-05-08-dsrs-crate-split-design.md:20`, `:36`.
- The dependency DAG keeps Leaven types out of normal DSRs users’ paths; `dsrs-leaven` is the only crate that pulls Leaven crates into DSRs: `DSRs/docs/plans/2026-05-08-dsrs-crate-split-design.md:110-123`.
- `dsrs-leaven` currently depends on `dsrs-core`, `dsrs-evaluate`, `dsrs-predict`, `leaven-core`, `leaven-surface`, `leaven-engine`, and `leaven-evidence`: `DSRs/crates/dsrs-leaven/Cargo.toml:10-18` on PR #87.
- The current `dsrs-leaven` scaffold deliberately uses `unimplemented!()` bodies until the Leaven-side optimizer path is real: `DSRs/crates/dsrs-leaven/src/lib.rs:1-5` on PR #87.
- Scaffolded public names are `DsrsProgramArtifact`, `DsrsProgramChange`, `DsrsEvaluator`, `DsrsLeavenProblem`, `DsrsEvidence`, and `DsrsProgramSurface`: `DSRs/crates/dsrs-leaven/src/lib.rs:7-17` on PR #87.
- Current scaffold shapes are not mature enough for parity: `DsrsProgramChange` is raw address/replacement JSON, `DsrsEvidence` is raw JSON payload, and `DsrsLeavenProblem::Case` / `ProposalAnnotations` are `serde_json::Value`: `DSRs/crates/dsrs-leaven/src/change.rs:1-5`, `DSRs/crates/dsrs-leaven/src/evidence.rs:1-6`, `DSRs/crates/dsrs-leaven/src/evaluator.rs:26-46` on PR #87.

### DSRs seams the bridge must preserve

- `dsrs-core::Module` is the typed execution contract with associated `Input`, `Output`, async `forward`, and default `call`: `DSRs/crates/dsrs-core/src/module.rs:56-88` on PR #87.
- `DynPredictor` is the mutable optimizer seam: schema, instruction read/write, demo read/write, and state dump/load: `DSRs/crates/dsrs-core/src/dyn_predictor.rs:13-65` on PR #87. Current `PredictState` is `Clone + Debug + Default` only; despite its doc comment saying “Serializable,” it lacks `Serialize` / `Deserialize` derives and must be made serializable before `dsrs-leaven` claims immutable artifact snapshots.
- `RawExample` is an alias for the raw example type that already derives `Serialize` / `Deserialize` / `Clone`, so `PredictState { demos: Vec<RawExample>, instruction_override: Option<String> }` can become serializable without inventing a parallel snapshot format: `DSRs/crates/dsrs-core/src/lib.rs:25`, `DSRs/crates/dsrs-core/src/example.rs:5-13` on PR #87.
- Predictor discovery walks Facet-shaped modules and yields dotted paths plus `&mut dyn DynPredictor`: `DSRs/crates/dsrs-core/src/dyn_predictor.rs:105-125` on PR #87.
- `Predict<S>` is the LM-calling optimizer leaf, stores tools/demos/instruction override/optional LM, implements `Module`, and implements `DynPredictor`: `DSRs/crates/dsrs-predict/src/predict.rs:48-106`, `:526-594` on PR #87.
- `dsrs-evaluate::TypedMetric<S, M>` exposes the clean per-example metric call the bridge needs: `evaluate(&Example<S>, &Predicted<M::Output>) -> Result<MetricOutcome>`. `evaluate_trainset` is only a sequential helper layered over that call: `DSRs/crates/dsrs-evaluate/src/evaluator.rs:10-37`, `:67-124` on PR #87.
- `dsrs-gepa::Optimizer::compile` mutates a typed module in place and remains trainset-only at the trait surface: `DSRs/crates/dsrs-gepa/src/lib.rs:48-65` on PR #87. The concrete `GEPA` type also exposes `compile_with_valset(trainset, valset, metric)`, where `valset` is used as the evaluation set for initial/child scoring and train minibatches still drive parent feedback: `DSRs/crates/dsrs-gepa/src/gepa.rs:271-329`, `:480-497` on PR #87.
- Native DSRs GEPA currently requires feedback-bearing outcomes, snapshots/restores predictor state for candidate evaluation, mutates instructions from feedback summaries, and finally installs the best frontier candidate: `DSRs/crates/dsrs-gepa/src/gepa.rs:169-226`, `:332-391` on PR #87.

### Current Leaven seams and constraints

- Product entrypoint is `leaven-run::optimize(seed)` with train/validation/test, runner, scorer, optimizer, budget, callbacks, store/cache policy, `.using(optimizer)`, and `.run()`: `crates/leaven-run/src/builder.rs:67-110`, `:221-239`, `:329-435`.
- Leaven run evidence for the product builder is `CasewiseEvidence<CaseAssessmentEvidence>` over `leaven_eval::Case<I, T>`: `crates/leaven-run/src/builder.rs:47-64`.
- `ScoringEvaluator` lowers runner/scorer output into per-case assessments with scalar score, generated output, and feedback: `crates/leaven-run/src/evaluator.rs:74-166`, `:229-260`.
- Engine orchestration calls `optimizer.initialize()` and `optimizer.step()` through `RunContext`; `RunContext` records proposals, invokes proposers, applies proposals, evaluates candidates, and stores assessments: `crates/leaven-engine/src/engine.rs:70-151`, `crates/leaven-engine/src/context/run_context.rs:146-277`, `:355-483`.
- `leaven_core::Artifact` owns typed `Change`, identity/cache identity, validation, and `apply_change`: `crates/leaven-core/src/artifact.rs:40-111`.
- `leaven_surface::EditSurface<A>` owns projection/lowering with `PartId`, external `Address`, borrowed `View`, surface-native `Edit`, `fingerprint`, `parts`, and `change_part`; surfaces do not apply changes: `crates/leaven-surface/src/edit_surface.rs:38-127`.
- `leaven-gepa::Gepa` is already a behavior-bearing optimizer shell with surface, population, reflector, candidate/part selectors, gate, sampler, validation policy, reflective dataset builder, counters, best state, and history: `crates/leaven-gepa/src/optimizer.rs:211-252`.
- GEPA’s step flow evaluates parent/child casewise, builds reflection requests, proposes a candidate through a reflector, applies typed proposals, gates by scalar score, updates population, and optionally validates: `crates/leaven-gepa/src/optimizer.rs:535-633`, `:808-857`, `:896-924`.
- Reflection examples can include case input, output, score, feedback, and source refs; default projection supports `CasewiseEvidence<ScalarEvidence>` and `CasewiseEvidence<CaseAssessmentEvidence>`: `crates/leaven-gepa/src/reflection.rs:43-174`, `:220-340`.
- Default LM-backed reflection currently parses plain text edits and requires `S::Edit = String`: `crates/leaven-gepa/src/reflection.rs:396-427`.

### Prior decisions and proof denominator

- Root `AGENTS.md` marks `crates/leaven-dsrs` as an orphan placeholder/bait, not a workspace member. New DSRs work must not route there unless the crate is deliberately reintroduced with topology/spec/local guidance updates.
- Leaven commits removed `leaven-dsrs` because DSRs owns the integration surface through `crates/dsrs-leaven`.
- `docs/plans/2026-05-15-gepa-aime-parity/requirements-summary.md` requires an ordinary public path through `leaven::optimize(...).train(...).validation(...).test(...).runner(...).score(...).using(Gepa...).budget(...).run()`, with durable/resumable GEPA behavior and honest train/validation/test/case evidence. This plan must not claim P8-level proof for DSRs until that path truly exists.
- `docs/specs/gepa_aime_paper_parity.md:108-132` records AIME reference settings and treats published GEPA CAIS numbers as reproduction targets until Leaven proves them live.
- `examples/p8_aime_gepa/AGENTS.md` says P8 owns the public high-level AIME GEPA proof and distinguishes deterministic builder mechanics from live provider quality.

### DSPy AIME GEPA reference

As of 2026-05-16, DSPy’s AIME GEPA reference shape is:

- Build a simple math solver signature (`problem -> answer`) wrapped in `dspy.ChainOfThought`.
- Optimize with `dspy.GEPA` against AIME 2022–2024 train/validation and evaluate on AIME 2025.
- Metric parses `prediction.answer` as an integer and returns score plus textual feedback containing parse/correctness details and, when available, full solution guidance.
- Tutorial settings include `openai/gpt-4.1-mini` solver, temperature `1`, `32000` max tokens; reflection LM `gpt-5`; `auto="light"`, `num_threads=32`, `track_stats=True`, and `reflection_minibatch_size=3`.
- Sources: <https://dspy.ai/tutorials/gepa_aime/>, <https://github.com/stanfordnlp/dspy/blob/main/docs/docs/api/optimizers/GEPA/overview.md>, <https://github.com/gepa-ai/gepa>, <https://arxiv.org/abs/2507.19457>.

## Approach

Use `DSRs/crates/dsrs-leaven` as a DSRs-owned adapter to Leaven’s generic optimizer contracts. The first working bridge should not widen `leaven-run`; `leaven-run` is optimized around Leaven’s public runner/scorer path and `CaseAssessmentEvidence`, while DSRs already has a typed `Module` + `TypedMetric` contract that should not be flattened into a string-runner API just to prove the bridge.

### Why not `leaven-run` first

Use a custom `Evaluator<P>` route before any `leaven-run` redesign because:

1. `leaven-run` output lowering is text-first; DSRs metrics want typed module output and typed prediction context.
2. `leaven-run`’s runner/scorer split is Leaven-public-path-native, while DSRs already owns “call typed module, then score typed prediction” as one coherent contract.
3. The current `dsrs-leaven` scaffold is too loose; raw JSON changes/evidence/cases should be fixed at the DSRs bridge boundary, not hidden behind `RunOutput`.
4. Leaven GEPA’s immutable-candidate search needs a fresh-module/snapshot strategy that DSRs’ current `compile(&mut M, trainset, metric)` surface does not expose.

### Recommended end-to-end path

1. `dsrs-leaven` snapshots predictor state from the caller’s typed DSRs module.
2. It builds an immutable `DsrsProgramArtifact` seed.
3. It builds a DSRs-native `OptimizationProblem`, custom evaluator, typed evidence, and `DsrsProgramSurface`.
4. It runs Leaven engine + `leaven-gepa::Gepa` directly.
5. GEPA selects a predictor part, builds reflection examples from DSRs evidence, and proposes instruction replacement edits through Leaven’s normal surface/edit path.
6. `Artifact::apply_change` returns new immutable predictor-state snapshots.
7. The evaluator materializes a fresh DSRs module for each candidate, installs the candidate snapshot, calls the typed module, calls the typed metric, and stores casewise DSRs evidence.
8. On successful completion, `dsrs-leaven` installs the best snapshot back into the original caller-owned `&mut M`.

### Bridge vocabulary to preserve in implementation

The implementation plan should name these pieces explicitly so agents do not rediscover them:

- `DsrsModuleFactory<M>` lives in `dsrs-leaven` for phase 1 and provides fresh module instances for candidate evaluation. The first AIME fixture may use an example-local factory/config, but the trait itself belongs to the bridge if more than one module path uses it.
- `PredictorPath` is a typed wrapper around the discovered dotted predictor path. Use it for surface part ids, addresses, change targets, and snapshot map keys rather than raw `String`.
- `DsrsPredictorSnapshot` preserves instruction text plus serializable DSRs `PredictState`. The bridge should not define a parallel JSON blob if `PredictState` can honestly derive `Serialize` / `Deserialize`; serialization belongs on the state type itself.
- `DsrsProgramState` is the immutable map from `PredictorPath` to predictor snapshot.
- `DsrsProgramLayout` records ordered predictor parts, layout fingerprint inputs, and any schema/signature metadata needed to validate state against a fresh module.
- `DsrsProgramArtifact` owns or references the module factory, layout, and immutable state.
- `DsrsProgramSurface` uses `PartId = PredictorPath`, `Address = PredictorPath`, `View<'a> = &'a str`, and `Edit = String` for the first milestone so Leaven GEPA’s plain-text edit parser can be reused.

### Resolved route choices

- **Bridge ownership:** `dsrs-leaven` owns all DSRs-specific code; Leaven stays generic.
- **Evaluator route:** use a custom `Evaluator<P>` and custom reflective dataset builder. Treat the default GEPA reflection projection as not reusable for DSRs unless implementation proves otherwise; the safe baseline is custom because default projection details are Leaven-internal and only cover existing Leaven evidence shapes.
- **State semantics:** use immutable artifacts internally and final install-back at the DSRs API boundary.
- **Surface shape:** support multiple predictor parts structurally, but prove the first milestone with one `Predict` / `ChainOfThought` leaf.
- **Parity denominator:** use native `GEPA::compile_with_valset` for deterministic train/validation parity, because current PR #87 concrete GEPA supports a validation set even though the public `Optimizer::compile` trait remains trainset-only. Test-set behavior is evaluated after optimization for both paths and reported honestly; it is not part of the native optimizer API contract.
- **Leaven changes:** none by default. If implementation exposes a real generic seam gap during Items 2–5, make the narrow Leaven change at that point with a DSRs-agnostic test and simultaneous `dsrs-leaven` consumption. Do not leave this as a terminal “cleanup” step.

## Preflight Before Implementation

Before starting implementation, reconcile this plan against the landed PR #87 code:

- confirm exact `dsrs-leaven`, `dsrs-core`, `dsrs-evaluate`, `dsrs-predict`, and `dsrs-gepa` paths;
- confirm `PredictState` still has `demos: Vec<RawExample>` and `instruction_override: Option<String>`; if so, add `Serialize` / `Deserialize` directly to `PredictState` before bridge artifacts;
- confirm `TypedMetric::evaluate` remains public and per-example; if it does, no DSRs-side metric helper is needed;
- confirm native `GEPA::compile_with_valset` remains available; if so, use it for deterministic train/validation parity and keep test as post-hoc report evaluation;
- adjust names/paths in this plan if the PR landed differently, but do not add compatibility wrappers for the old monolith or for draft-name drift.

## Work Items

### Item 1 — Make predictor state serializable at the owning DSRs seam

**Goal:** Resolve the snapshot question by making the existing DSRs predictor state type serializable instead of inventing an adapter-local JSON format.

**Done when:**

- `PredictState` derives or otherwise implements `Serialize` and `Deserialize` in `dsrs-core`.
- The serialization round trip preserves demos and instruction override exactly enough for `DynPredictor::load_state` to reconstruct behavior.
- `RawExample` serialization remains the representation for demos; no parallel `dsrs-leaven` demo snapshot format is introduced.
- A law/example test proves `dump_state -> serde_json -> load_state -> dump_state` is stable for a `Predict<S>` with instruction override and at least one demo.

**Key files:**

- `DSRs/crates/dsrs-core/src/dyn_predictor.rs:55-65` on PR #87.
- `DSRs/crates/dsrs-core/src/example.rs:5-13` on PR #87.
- `DSRs/crates/dsrs-predict/src/predict.rs:625-650` on PR #87.

**Dependencies:** PR #87 merged or checked out as implementation base; preflight complete.

**Size:** Small.

### Item 2 — Replace raw scaffold boundaries with typed bridge vocabulary and factory support

**Goal:** Turn `dsrs-leaven` from compile-only stubs into a typed public bridge surface by replacing JSON-shaped placeholders with names that preserve DSRs and Leaven truth, including the fresh-module factory needed by immutable Leaven search.

**Done when:**

- `DsrsProgramChange` is typed around `PredictorPath` plus a predictor edit, not raw JSON replacement.
- `DsrsEvidence` is replaced or narrowed into typed case-assessment evidence carrying scalar score, rendered output, and feedback.
- `DsrsLeavenProblem::Case` is no longer an unclassified `serde_json::Value` placeholder.
- `DsrsModuleFactory<M>` or the landed equivalent has an owning home and is usable by artifact/evaluator/orchestration code.
- Scaffold panic behavior is retired as each owning behavior lands; there is no final public raw-JSON bridge route left for compatibility.

**Key files:**

- `DSRs/crates/dsrs-leaven/src/change.rs:1-5` on PR #87.
- `DSRs/crates/dsrs-leaven/src/evidence.rs:1-6` on PR #87.
- `DSRs/crates/dsrs-leaven/src/evaluator.rs:26-46` on PR #87.
- `DSRs/crates/dsrs-leaven/src/lib.rs:1-17` on PR #87.
- New `DSRs/crates/dsrs-leaven/src/factory.rs`, if the landed scaffold does not already provide an equivalent home.

**Dependencies:** Item 1.

**Size:** Medium.

### Item 3 — Implement immutable DSRs program artifacts over mutable predictor state

**Goal:** Make DSRs predictor state satisfy Leaven’s `Artifact` contract without mutating the caller’s module during search.

**Done when:**

- `DsrsProgramArtifact` owns or references `DsrsModuleFactory`, `DsrsProgramLayout`, and `DsrsProgramState`.
- Artifact identity/cache identity are derived from layout plus predictor snapshot content.
- `apply_change` produces a new artifact snapshot and never mutates shared state or the caller’s module.
- Validation refuses missing predictor paths, malformed loaded state, and layout/state mismatches explicitly.
- Failure during final install-back restores the caller’s original snapshot before returning an error.

**Key files:**

- `DSRs/crates/dsrs-leaven/src/artifact.rs:7-53` on PR #87.
- New DSRs-side helper module for predictor state capture/install, if needed.
- `DSRs/crates/dsrs-core/src/dyn_predictor.rs:13-65`, `:105-125` on PR #87.
- `crates/leaven-core/src/artifact.rs:40-111`.

**Dependencies:** Items 1–2.

**Size:** Large.

### Item 4 — Implement `DsrsProgramSurface` as the GEPA edit surface

**Goal:** Expose DSRs predictor instructions as Leaven-editable parts while keeping mutation in artifact changes, not in the surface.

**Done when:**

- `parts()` returns one part per discovered predictor path, with current instruction text as the view.
- `PartId` and `Address` are `PredictorPath`, `View<'a>` is the current instruction string view, and `Edit` is `String` for the first milestone.
- `change_part()` lowers a replacement instruction string into a typed DSRs program change.
- Multiple predictor paths are supported structurally even though the first proof uses one leaf.

**Key files:**

- `DSRs/crates/dsrs-leaven/src/surface.rs:7-63` on PR #87.
- `DSRs/crates/dsrs-core/src/dyn_predictor.rs:105-125` on PR #87.
- `crates/leaven-surface/src/edit_surface.rs:38-127`.
- `crates/leaven-gepa/src/reflection.rs:396-427`.

**Dependencies:** Items 2–3.

**Size:** Medium.

### Item 5 — Implement DSRs-native evaluator, problem, evidence projection, and reflective dataset

**Goal:** Preserve DSRs typed execution and metric semantics while feeding Leaven GEPA enough score/output/feedback to select and reflect.

**Done when:**

- `DsrsLeavenProblem` uses `DsrsProgramArtifact` as its artifact and casewise DSRs evidence as its evidence.
- `DsrsEvaluator` materializes a fresh module per candidate evaluation, installs the candidate snapshot, calls the typed module, calls the typed metric, and records score/output/feedback.
- `DsrsEvaluator` calls the existing public per-example `TypedMetric::evaluate`; no DSRs-side metric helper is planned unless the landed PR removes that seam.
- `GepaScoreEvidence` is implemented for the DSRs casewise evidence shape, projecting scalar score only.
- A custom DSRs reflective dataset builder projects case input, candidate output, scalar score, feedback text, and source refs without exposing held-out answers except through intentionally returned scorer feedback.
- The implementation uses Leaven trust/read-scope machinery or an equivalent engine-visible policy to keep validation/test hidden from proposer/reflection input.
- Phase-1 evaluation is sequential unless the landed DSRs APIs already provide a proven concurrent contract.

**Key files:**

- `DSRs/crates/dsrs-leaven/src/evaluator.rs:26-46` on PR #87.
- `DSRs/crates/dsrs-leaven/src/evidence.rs:1-6` on PR #87.
- `DSRs/crates/dsrs-evaluate/src/evaluator.rs:10-37`, `:67-81` on PR #87.
- `crates/leaven-engine/src/context/run_context.rs:355-483`.
- `crates/leaven-gepa/src/optimizer.rs:35-70`, `:896-924`.
- `crates/leaven-gepa/src/reflection.rs:43-174`, `:220-340`.

**Dependencies:** Items 2–4.

**Size:** Large.

### Item 6 — Add the DSRs-owned Leaven optimization entrypoint and minimum report

**Goal:** Provide a DSRs-side orchestration API that can run train/validation/test GEPA through Leaven without pretending it is the current DSRs `Optimizer::compile` surface or the Leaven `optimize(...)` public builder.

**Done when:**

- The entrypoint accepts the caller’s mutable module, fresh-module factory, train/validation/test examples, typed metric, input/output render hooks, GEPA config, and budget.
- The orchestration snapshots original state, builds seed artifact/surface/evaluator/problem, configures train/validation/test visibility, runs Leaven engine + GEPA, performs final evaluations, installs the best state back into the caller module, and returns a DSRs-owned report.
- The report includes at least enough to judge the milestone: baseline train score, optimized train score, validation score, test score, stop reason, budget snapshot, final predictor instruction/state summary, and per-case feedback summaries.
- The report does not expose Leaven engine internals as the stable DSRs public result.

**Key files:**

- New `DSRs/crates/dsrs-leaven/src/optimize.rs` or `src/run.rs`.
- New `DSRs/crates/dsrs-leaven/src/report.rs`.
- `crates/leaven-engine/src/engine.rs:70-151`.
- `crates/leaven-engine/src/context/run_context.rs:146-277`, `:355-483`.
- `crates/leaven-gepa/src/optimizer.rs:535-633`.

**Dependencies:** Items 3–5.

**Size:** Large.

### Item 7 — Prove deterministic AIME-style GEPA parity against native DSRs GEPA

**Goal:** Establish the first real success denominator: the Leaven-driven bridge matches native concrete `GEPA::compile_with_valset` on a deterministic train/validation fixture, while both paths also receive the same post-optimization test evaluation.

**Done when:**

- A DSRs-owned AIME-style example exists under `DSRs/crates/dsrs-leaven`, with one math solver signature, one optimizable `Predict` / `ChainOfThought` leaf, exact-integer metric, and feedback text suitable for reflection.
- The same deterministic train fixture, validation fixture, seed prompt, metric, and budget run through native concrete `GEPA::compile_with_valset` and through `dsrs-leaven` + Leaven GEPA.
- Parity is explicit: both paths improve over seed and match final train/validation behavior under deterministic conditions, or any intentional algorithmic difference is documented as blocking parity.
- The same held-out test fixture is evaluated after optimization for both final modules/artifacts and reported, but it is not treated as part of native `Optimizer::compile` semantics.
- The deterministic lane uses both a deterministic module-execution fixture/LM adapter and a deterministic proposal/reflection fixture; exact parity should not depend on provider calls.
- The proof explicitly distinguishes deterministic parity from provider-backed live quality.

**Key files:**

- New `DSRs/crates/dsrs-leaven/examples/aime_gepa.rs`.
- New `DSRs/crates/dsrs-leaven/tests/gepa_parity_deterministic.rs`.
- `DSRs/crates/dsrs-gepa/src/gepa.rs:169-226`, `:271-329`, `:332-391`, `:480-497` on PR #87.
- DSPy AIME reference: <https://dspy.ai/tutorials/gepa_aime/>.

**Dependencies:** Items 1–6.

**Size:** Large.

### Item 8 — Add proof tests and public maturity labels

**Goal:** Make the bridge hard to accidentally turn into proxy proof or public lies.

**Done when:**

- Artifact tests cover snapshot capture, instruction-only change application, install-back, missing predictor refusal, and caller-state restoration on error.
- Evaluator tests prove typed module output reaches the typed metric and evidence stores score/output/feedback.
- Split-visibility tests prove validation/test answers do not leak into reflection except through intentionally returned scorer feedback.
- Public docs classify deterministic parity, optional provider smoke, P8 non-equivalence, and phase-1 non-goals in one place.

**Key files:**

- New `DSRs/crates/dsrs-leaven/tests/artifact_roundtrip.rs`.
- New `DSRs/crates/dsrs-leaven/tests/split_visibility.rs`.
- New or updated `DSRs/crates/dsrs-leaven/README.md`, if PR #87 establishes per-crate docs.
- `docs/plans/2026-05-15-gepa-aime-parity/requirements-summary.md`.
- `examples/p8_aime_gepa/AGENTS.md`.

**Dependencies:** Items 3–7.

**Size:** Medium.

### Item 9 — Add provider-backed or cached AIME smoke after deterministic parity

**Goal:** Confirm that the bridge carries real runtime semantics without making provider variability the exact parity gate.

**Done when:**

- A provider-backed or recorded-cache run exercises the same DSRs module/evaluator/Leaven GEPA bridge.
- The run is labeled as runtime smoke, not exact algorithmic parity.
- Provider credentials, model names, token budgets, and cache behavior are documented as optional/operator-expensive.
- Deterministic parity remains the CI-friendly gate.

**Key files:**

- `DSRs/crates/dsrs-leaven/examples/aime_gepa.rs`.
- Optional DSRs-side fixture/cache docs, depending on existing conventions after PR #87.
- DSPy AIME reference: <https://dspy.ai/tutorials/gepa_aime/>.
- `docs/specs/gepa_aime_paper_parity.md:108-132`.

**Dependencies:** Item 7.

**Size:** Medium.

## Leaven Change Policy

Phase 1 starts with no planned Leaven production-code changes. If `dsrs-leaven` cannot use the existing generic seams, make the smallest DSRs-agnostic Leaven change at the point of discovery:

- `crates/leaven-gepa/src/reflection.rs` only for additive generic reflection projection/rendering hooks.
- `crates/leaven-engine/*` only if `dsrs-leaven` cannot construct a valid custom evaluator run through public engine APIs.
- `crates/leaven-run/*` only in a later, explicit typed-builder design after the bridge already works.

Any Leaven change must land with a generic Leaven test, no DSRs types in Leaven, and simultaneous `dsrs-leaven` consumption. If crate boundaries change, update topology docs/tests in the same change.

## Verification Plan

During implementation, use narrow gates first, then the owning full gates:

- DSRs bridge iteration:
  - `cargo check -p dsrs-leaven`
  - `cargo test -p dsrs-leaven`
  - native `GEPA::compile_with_valset` parity test against the deterministic AIME-style train/validation fixture
  - DSRs workspace check/test command after PR #87 defines the canonical split-crate gate
- Leaven only if generic Leaven code changes are made:
  - the owning crate test for the changed Leaven seam
  - `cargo test -p leaven --test topology_contract` if crate boundary/topology changes
  - `just check` before claiming Leaven-side completion

The plan does not require a Leaven production-code change. If implementation does change Leaven, update this plan or the nearest owning spec/docs in the same change so the public maturity claim remains truthful.

## Non-goals

- No compatibility path for the current monolithic `dspy-rs` tree.
- No reintroduction of Leaven-owned `crates/leaven-dsrs`.
- No phase-1 `leaven-run` typed-builder redesign.
- No claim that deterministic DSRs parity equals Leaven P8 public AIME proof.
- No generic durable/resumable DSRs module blueprint in phase 1.
- No demo-edit, tool-edit, routing, structural graph, or online adaptation edits in the first bridge; instruction replacement is enough for the AIME-style parity proof.

## Resolved Questions

Resolved against current PR #87 head `88d0c887628ab41c5dfef16cbcb0676d60c5593e` on 2026-05-16:

- **Predictor state:** `PredictState` is the exact dump/load type. It is cloneable but not serializable yet. Make it `Serialize` / `Deserialize` in `dsrs-core`; do not create a parallel `dsrs-leaven` JSON snapshot.
- **Metric call:** `dsrs-evaluate` already exposes clean per-example `TypedMetric::evaluate`; `DsrsEvaluator` should call it directly.
- **Native GEPA splits:** public `Optimizer::compile` is trainset-only, but concrete `GEPA::compile_with_valset` exists and should be the parity target for train/validation. Test remains post-hoc final evaluation/reporting for both paths.
- **Deterministic fixture:** use both deterministic module execution (LM adapter/fixture) and deterministic proposal generation (reflector fixture matching native feedback-append semantics).

## References

- DSRs PR #87: <https://github.com/krypticmouse/DSRs/pull/87>
- DSRs split design on PR #87: `DSRs/docs/plans/2026-05-08-dsrs-crate-split-design.md`
- DSRs split implementation on PR #87: `DSRs/docs/plans/2026-05-08-dsrs-crate-split-implementation.md`
- Leaven root contract: `AGENTS.md`
- Leaven AIME parity requirements: `docs/plans/2026-05-15-gepa-aime-parity/requirements-summary.md`
- Leaven AIME parity spec: `docs/specs/gepa_aime_paper_parity.md`
- Leaven P8 local guidance: `examples/p8_aime_gepa/AGENTS.md`
- DSPy AIME tutorial: <https://dspy.ai/tutorials/gepa_aime/>
- DSPy GEPA API overview: <https://github.com/stanfordnlp/dspy/blob/main/docs/docs/api/optimizers/GEPA/overview.md>
- GEPA repository: <https://github.com/gepa-ai/gepa>
- GEPA paper: <https://arxiv.org/abs/2507.19457>
