# Layer 1 Fix Priority Map

Status: canonical Layer 1 audit doc.

This is an ordered hard-cutover map for restoring the ordinary-user surface. It
is not a compatibility plan and does not authorize product-code changes from the
audit tree.

## Priority 0: Quarantine False Public Proof Before Adding More Surface

- severity: blocker
- root causes addressed: `RC-L1-001`, `RC-L1-007`
- ideal contract: ordinary examples and default imports should expose only real
  ordinary contracts. Fixtures are allowed only when named as fixtures.
- current implementation: `p8_aime_gepa` uses the public builder shell but wires a
  deterministic `ReflectiveMutation` fixed edit (`examples/p8_aime_gepa/src/main.rs:75-99`;
  `crates/leaven-gepa/src/proposer.rs:21-47`) and shells live OpenAI calls out to
  Python (`examples/p8_aime_gepa/src/main.rs:271-301`;
  `examples/p8_aime_gepa/scripts/openai_solver.py:24-45`). The ordinary prelude
  imports engine-author names beside Layer 1 names (`crates/leaven/src/prelude.rs:3-25`).
- blocker/gap: coverage can currently run `p8_aime_gepa` as a milestone package
  (`scripts/coverage-gate.py:13-24`), but that only ratifies the proxy path.
- user impact: future implementors can keep "proving" GEPA without proving
  reflection, provider-neutral LM, cache, or runtime role wiring.
- correction direction: rename/move fixed edit helpers to explicit fixture/test
  surfaces; reserve `ReflectiveMutation` for real evidence-aware reflection; mark
  proxy examples as demos until they use the real surface; split ordinary and
  advanced preludes.
- required proof/tests:
  - compile-pass test: a Layer 1 ordinary example builds with only
    `use leaven::prelude::*`;
  - compile-fail or export-check test: `leaven::prelude::*` does not expose
    `RunContext`, `RunGraphView`, `TrustPolicy`, `EvaluationRequest`,
    `Population`, `Proposer`, `Evaluator`, or cache store/key types;
  - compile-fail or typed-error test: missing optimizer is either an intentional
    typestate contract or a documented pre-run refusal, not an accidental
    method-resolution failure;
  - example gate: no product proof may use `ReflectiveMutation::new(fixed_edit)`
    unless the type/path says fixture/demo;
  - coverage gate: product proof must distinguish demo/proxy milestone from
    capability proof.

## Priority 1: Replace Sync Runner/Scorer With The Canonical Execution Contract

- severity: blocker
- root causes addressed: `RC-L1-003`, `RC-L1-004`
- ideal contract: `.runner(...)` lowers to async `CandidateRunner`, and
  `.score(...)` lowers to async `Scorer` returning `Result<Metered<Score>,
  ScoreError>` (`docs/specs/gepa_public_private_surface.md:987-1039`).
- current implementation: runner/scorer are sync `Fn` callbacks
  (`crates/leaven-run/src/builder.rs:28-29`), exposed by sync `.runner` and
  `.score` methods (`crates/leaven-run/src/builder.rs:136-153`), and called
  serially inside `ScoringEvaluator::evaluate` (`crates/leaven-run/src/evaluator.rs:65-128`).
- blocker/gap: the current API cannot honestly represent LM calls, model judges,
  subprocess runners, agentic sandboxes, compiler/profiler harnesses, or
  metered scoring cost, all of which the original evaluator surface names as
  expected workloads (`docs/specs/initial_library.md:2128-2165`).
- user impact: users reach for process escapes or hidden runtimes before they can
  run a normal LM/program/agent optimizer.
- correction direction: hard-cut the builder and `ScoringEvaluator` to one async,
  result-bearing execution/scoring path. Scalar/bool/sync conveniences lower into
  that path.
- required proof/tests:
  - `crates/leaven-run/tests`: async runner success, async scorer success,
    runner failure records execution error, score-on-error policy,
    scorer error is not `0.0`, metered scorer cost charges once;
  - concurrency scenario: multiple cases can evaluate with bounded concurrency;
  - public example: mocked LM/program runner uses `.runner(...).score(...).run().await`
    with no nested `block_on` and no shell-provider escape.

## Priority 2: Introduce Stable Work Inputs And Single-Task Mode

- severity: blocker
- root causes addressed: `RC-L1-002`, `RC-L1-005`
- ideal contract: Layer 1 accepts single-task/no-dataset work, train/search
  cases, optional validation, optional test, and domain task suites that lower to
  stable case ids and split roles. Mode inference is ordinary:
  no train/validation/test means single-task, train-only means multi-task/search,
  train+validation/test means generalization
  (`docs/specs/gepa_public_private_surface.md:722-734`).
- current implementation: `.train(Vec<C>)` is the only non-unit case-type fixing
  entry (`crates/leaven-run/src/builder.rs:92-114`); the default `C = ()` path
  can reach `run()` but lowers no train/validation/test input to an empty dataset
  and empty `CaseSet`, not an unscoped task
  (`crates/leaven-run/src/builder.rs:208-222`). `leaven-eval` already has
  explicit `Case` / `NoTarget` and duplicate-id dataset construction
  (`crates/leaven-eval/src/dataset.rs:9-24`;
  `crates/leaven-eval/src/dataset.rs:95-100`), but the ordinary builder does
  not re-export or use it. `run()` concatenates vectors and builds positional
  `Dataset::from_ordered`/`CaseId::from_index` state
  (`crates/leaven-run/src/builder.rs:214-222`;
  `crates/leaven-run/src/builder.rs:302-356`).
- blocker/gap: product builders must reject duplicate case ids, default to
  disjoint splits, create stable fingerprints, and lower split-use intent into
  engine trust policy (`docs/specs/eval_lowering_detail.md:650-673`).
- user impact: one-task search looks like API abuse, and real benchmark reports
  cannot cite original case identity or detect split leakage.
- correction direction: make `Case<I, T = NoTarget>` / `CaseSuite` or equivalent
  stable work inputs the primary path. Keep dense vectors only as explicit
  convenience with clear identity semantics.
- required proof/tests:
  - law tests: duplicate id rejection, missing case rejection, overlap refusal,
    stable dataset/split fingerprints;
  - scenario tests: single-task GEPA/keep-best run that evaluates a real
    unscoped/singleton task rather than an empty case set; train-only run;
    generalization run with hidden validation/test content; final-test-only default;
  - report tests: case-level output uses user-stable ids, not vector positions.

## Priority 3: Make Score/Evidence/Report Truth Rich Enough For Reflection

- severity: high
- root causes addressed: `RC-L1-004`, `RC-L1-008`
- ideal contract: `Score` carries comparable score axes, metrics, feedback,
  structured records, staged attachments, metadata, and evidence refs
  (`docs/specs/gepa_public_private_surface.md:1115-1182`). Reports cite graph ids
  and evidence refs, and do not copy artifacts or hidden payloads
  (`docs/specs/eval_lowering_detail.md:780-789`).
- current implementation: `RunOutput` is string plus trace strings, `Score` is
  `f64` plus one feedback string and `(String, String)` pairs, and
  `ScoreContext` is three public fields (`crates/leaven-run/src/evidence.rs:3-54`).
  Report projection flattens score/evidence into aggregate floats, feedback
  strings, trace strings, and string event names
  (`crates/leaven-run/src/builder.rs:637-647`;
  `crates/leaven-run/src/result.rs:35-61`).
- blocker/gap: score normalization must preserve evidence until explicit report
  projection (`docs/specs/eval_lowering_detail.md:331-342`), and score errors
  must not silently become score `0.0`
  (`docs/specs/gepa_public_private_surface.md:1077-1081`).
- user impact: reflection has no durable typed substrate to read, and ordinary
  users cannot inspect why a run worked or failed.
- correction direction: implement rich `Score`, accessor-based `ScoreContext`,
  staged attachments, explicit scoring errors, and graph-backed report views in
  one path.
- required proof/tests:
  - score law tests for finite comparable values, metrics with direction,
    unscored diagnostics, attachment staging, and metadata non-decision behavior;
  - result tests for optional best, stop reason, absent/failed evidence not zero,
    final-test-only markers, public events, cost/cache summary, and evidence refs;
  - GEPA reflection test proving selected feedback/evidence can be rendered into
    a reflector request without hidden validation/test content.

## Priority 4: Add Ordinary Runtime/Cache Roles

- severity: high
- root causes addressed: `RC-L1-006`, `RC-L1-007`
- ideal contract: solver/program runner, reflector, scorer/model judge, and agent
  runtime roles each have provider, cache, and budget policy. Raw providers do
  not read/write response cache, and `CachedLm`/cache stores remain lower-level
  cache capabilities (`docs/specs/lm_runtime_and_response_cache.md:59-88`;
  `docs/specs/lm_runtime_and_response_cache.md:154-207`).
- current implementation: `leaven-lm`, `leaven-lm-openai`, and
  `leaven-lm-cache` exist, but the public spec teaches `CachedLm::read_write`
  wrapper stacking (`docs/specs/lm_runtime_and_response_cache.md:15-31`), and
  `leaven-run` scoring evaluator always returns `CachePolicy::Never`
  (`crates/leaven-run/src/evaluator.rs:61-63`). `OpenAiLm::from_env` documents a
  default model argument for fingerprint stability but ignores it
  (`crates/leaven-lm-openai/src/client.rs:27-37`).
- blocker/gap: Layer 1 has no role-based runtime API and no way to say "use this
  cached LM for solver, this cached LM/agent for reflector, and this cached judge
  for scoring." It also lacks a public role identity contract for provider,
  model, cache policy, and budget/cost policy.
- user impact: live examples bypass Leaven LM/cache, or users learn cache
  internals before they can run an optimizer.
- correction direction: expose ordinary runtime-role configuration and keep cache
  wrappers/stores in advanced/cache docs. OpenAI swap must be provider
  construction, not a Python/script architecture change.
- required proof/tests:
  - `leaven-lm-cache` policy/key contract tests remain green;
  - Layer 1 mocked LM scenario shows cache hit/miss/cost summary by solver,
    reflector, and scorer roles;
  - provider identity test proves ordinary OpenAI role construction records the
    model/fingerprint it claims to configure;
  - OpenAI request/response mapping tests remain no-credential;
  - `p8` live-provider smoke depends on Leaven LM crates, not Python provider
    bypass.

## Priority 5: Replace Fixed GEPA Reflection With Evidence-Aware Reflection

- severity: blocker
- root causes addressed: `RC-L1-007`, plus downstream Layer 2 GEPA gaps
- ideal contract: GEPA reflection consumes selected parent, selected part,
  assessment ids, casewise evidence, optional attribution, objective/background,
  transcript refs, validation/apply errors, and prior summaries, then records
  typed causal and `informed_by` provenance (`docs/specs/gepa_optimizer_surface.md:322-357`;
  `docs/specs/gepa_optimizer_surface.md:463-483`).
- current implementation: GEPA calls a `SurfaceProposer` that receives only
  artifact, surface, and part (`crates/leaven-gepa/src/proposer.rs:6-19`), and
  the built-in `ReflectiveMutation` ignores those inputs (`crates/leaven-gepa/src/proposer.rs:35-47`).
  `Gepa::propose_candidate` calls that narrow proposer path
  (`crates/leaven-gepa/src/optimizer.rs:533-563`).
- blocker/gap: this cannot read feedback, traces, candidate history, budget, or
  runtime/cache roles.
- user impact: AIME score movement can happen without GEPA reflection. The
  ordinary product proof remains fake even after runner/scorer and cache work
  land unless reflection is replaced.
- correction direction: implement real async reflector/proposer request lowering.
  Use Leaven LM/agent/runtime roles and evidence/rendering views. Keep fixed edit
  fixtures out of production-looking public API.
- required proof/tests:
  - `leaven-gepa` law/example: reflective proposer turns casewise feedback into a
    surface edit using `leaven-lm-mock`, and invalid output becomes typed
    proposal error (`docs/specs/gepa_optimizer_surface.md:621-637`);
  - product scenario: mock-LM AIME-like run improves by reading scored feedback,
    not by pre-authored replacement;
  - trust scenario: validation/test content is hidden from reflector by default.

## Priority 6: Rebuild The Canonical Layer 1 Product Proof

- severity: blocker
- root causes addressed: all Layer 1 root causes
- ideal contract: `p8` or its replacement proves the minimum ordinary surface:
  async runner/scorer, rich score/evidence, stable cases, single-task or
  train/validation/test mode as applicable, runtime/cache roles, real mock
  reflector consuming feedback, graph-backed result report, and no engine
  internals in user code. AIME is one proof, not the whole denominator; the
  Layer 1 bar also needs a scalar single-task proof and, after the substrate is
  honest, at least one non-GEPA or pairwise-shaped ordinary path so GEPA/AIME
  cannot stand in for "optimizer library works."
- current implementation: `p8_aime_gepa` says it exercises the high-level API and
  reports split scores/events (`examples/p8_aime_gepa/README.md:1-9`), but also
  admits deterministic mode is not evidence of live AIME improvement
  (`examples/p8_aime_gepa/README.md:23-33`).
- blocker/gap: examples are the acceptance surface users will trust. A demo that
  moves numbers through a fixed edit cannot be the canonical product proof.
- user impact: the repo will keep looking healthier than the library product is.
- correction direction: rebuild the canonical proof after priorities 0-5. The
  first proof may use mocked LM/agent components, but it must use the same public
  APIs that a live provider run uses.
- required proof/tests:
  - `cargo run -p p8_aime_gepa` or successor proves mock LM/agent reflection over
    the real ordinary surface;
  - scalar single-task proof exercises the no-dataset mode without fake
    train/validation/test vectors;
  - non-GEPA or pairwise-shaped ordinary proof exercises the same builder/result
    facade once runner/scorer/case/report substrate is in place;
  - feature-gated live smoke swaps in `OpenAiLm`/runtime config in under one
    provider-construction change;
  - `just milestone-p8`, `just test`, and final `just check` remain the gates
    before claiming Layer 1 behavior complete (`docs/testing/README.md:7-17`;
    `docs/testing/README.md:36-39`).

## Ordered Proof Gates

Use narrow gates during implementation, then the canonical gate before claiming
completion:

1. Public import/maturity gate for ordinary prelude compile-pass/compile-fail
   coverage and fixture quarantine.
2. `cargo nextest run -p leaven-run` for async runner/scorer, work-input, score,
   and result contracts.
3. `cargo nextest run -p leaven-lm-cache` and provider mapping tests for runtime
   role/cache behavior.
4. `cargo nextest run -p leaven-gepa` for evidence-aware reflection and split
   policy laws.
5. `cargo nextest run -p leaven --test gepa_parity` only after the parity test
   uses real reflection, not fixed edit proof.
6. `just milestone-p8` for the canonical ordinary-user example.
7. `just check` before any completion claim; the repo testing contract defines it
   as the full local gate (`docs/testing/README.md:7-17`).
