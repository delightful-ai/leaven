# Layer 1 Vision Comparison

Status: canonical Layer 1 audit doc.

This compares the original ordinary-user vision against the current repo state.
The central truth is the current specs and code; refinement reports were used
only as background.

## Original Layer 1 Vision

Leaven is a Rust library for writing optimizers over arbitrary artifacts whose
behavior can be assessed (`docs/specs/initial_library.md:404-408`). GEPA is one
optimizer value, not the engine (`docs/specs/initial_library.md:425-443`). End
users should get a short path through `optimize(seed).train(...).validation(...).score(...).using(...).budget(...).run().await`
and should not have to understand every internal trait
(`docs/specs/initial_library.md:453-468`).

The Layer 1 public/private spec makes the ordinary story even sharper:

```text
Give Leaven a candidate, training work, a scoring function, an optimizer, and a budget.
Optionally give it validation/test work and swap GEPA strategies.
```

That story is explicitly not evaluation specs, split usage rules, actor scopes,
visibility policy, or graph internals (`docs/specs/gepa_public_private_surface.md:20-47`).

## Comparison Matrix

| Area | Ideal contract | Current reality | Gap | User impact | Correction direction | Required proof/tests |
| --- | --- | --- | --- | --- | --- | --- |
| Product denominator | Optimize arbitrary LM/program/agent artifacts; GEPA is one optimizer, not the whole library (`docs/specs/initial_library.md:404-443`; `docs/specs/initial_library.md:4751-4759`). | Current Layer 1 proof centers on GEPA/AIME, and the AIME path is fixture-backed (`examples/p8_aime_gepa/src/main.rs:75-99`). | AIME-shaped numeric movement can substitute for "optimizer library works." | Future fixes can make AIME less fake while still missing single-task, pairwise, agentic, and optimizer-family neutrality. | Treat AIME as one proof, not the denominator. Layer 1 acceptance is "ordinary user can run a real optimizer over real work." | Add pressure proofs for scalar single-task, train/validation/test GEPA, and at least one non-GEPA or pairwise-shaped public path after substrate fixes (`docs/specs/initial_library.md:4628-4685`). |
| Import model | Layer 1 users should touch seed/program, work input, scorer/evaluator, runner, GEPA, budget, result/report, and not `RunGraph`, `TrustPolicy`, `EvaluationRequest`, `Population`, selectors, or `EvidenceStore` (`docs/specs/gepa_public_private_surface.md:51-83`). | `leaven::prelude::*` exports core and engine machinery with ordinary names (`crates/leaven/src/prelude.rs:3-25`). | The ordinary import path teaches internals as default surface. | Users and examples solve product problems by reaching through the builder. | Split ordinary, GEPA customizer, engine author, and cache/runtime imports. | Compile/import contract and deny-list tests for ordinary prelude. |
| Work modes | Single-task, train-only, and generalization should all feel native (`docs/specs/gepa_public_private_surface.md:101-136`; `docs/specs/gepa_public_private_surface.md:722-734`). | The builder fixes case type only through `.train(Vec<C>)` (`crates/leaven-run/src/builder.rs:92-114`) and rejects held-out cases without train (`crates/leaven-run/src/builder.rs:207-213`). | No explicit single-task/no-dataset public path. | Users fake datasets for benchmarks, environments, live evals, and single objectives. | Add single-task/no-dataset work input and stable cases/suites. | Scenario tests for no-dataset single-task, train-only, and generalization modes. |
| Execution | Runner contract is async, can represent typed output, trace, attachments, cost, and failures (`docs/specs/gepa_public_private_surface.md:987-1028`). | Runner is sync `Fn(&A, &C) -> RunOutput` (`crates/leaven-run/src/builder.rs:28-29`) and `RunOutput` is string plus trace strings (`crates/leaven-run/src/evidence.rs:3-20`). | No honest LM/subprocess/agent execution contract. | Users must block, shell out, or bypass Leaven. | Hard-cut to async `CandidateRunner` lowered through `leaven-run`. | Async runner success/failure, cost, trace, attachment, and concurrency tests. |
| Scoring | `.score(...)` is the ordinary concept and lowers scalar or rich feedback into `Score`; `ScoreContext` is a typed view and must not expose graph/trust internals (`docs/specs/gepa_public_private_surface.md:894-985`; `docs/specs/gepa_public_private_surface.md:1029-1081`). | Scorer is sync `Fn(ScoreContext) -> Score`, `ScoreContext` has public fields only for artifact/case/output, and score is `f64 + String + Vec<(String, String)>` (`crates/leaven-run/src/builder.rs:146-153`; `crates/leaven-run/src/evidence.rs:23-54`). | Rich feedback, model-judge rationale, errors, history, budget, and attachments are absent. | Reflection and reporting lose the evidence users meant to provide. | Make `.score(...)` the rich path; keep scalar/bool only as adapters. | Rich score normalization, score error, metered cost, and attachment staging laws. |
| Cases and splits | Cases are work items with stable ids, optional targets, metadata, unique ids across default splits, and final-test-only default (`docs/specs/gepa_public_private_surface.md:773-820`). | `run()` concatenates vectors, builds `Dataset::from_ordered`, and generates dense positional case ids (`crates/leaven-run/src/builder.rs:214-222`; `crates/leaven-run/src/builder.rs:302-356`). | User identity, duplicate detection, and split evidence policy are too weak. | Reports cannot reliably identify or reproduce case-level behavior. | Primary path accepts stable `Case`/suite inputs; vector path is explicit dense-id convenience. | Dataset/split law tests plus public builder trust-policy scenario tests (`docs/specs/eval_lowering_detail.md:790-818`). |
| Runtime/cache | LM boundary is provider-neutral and response cache is separate from engine evaluation cache (`docs/specs/lm_runtime_and_response_cache.md:54-88`). Ordinary roles need policy by solver/reflector/scorer/agent. | `CachedLm` is a public wrapper type (`crates/leaven-lm-cache/src/cached.rs:6-17`) and prelude export (`crates/leaven-lm-cache/src/lib.rs:15-19`); `leaven-run` evaluator always uses `CachePolicy::Never` (`crates/leaven-run/src/evaluator.rs:61-63`). | There are pieces, not an ordinary runtime role story. | Users manage caching outside the run or learn wrappers too early. | Add role-based runtime/cache config; keep wrappers advanced. | Mock LM cache scenario by role and cache/cost summary in result. |
| Live provider proof | Live solver/reflector should use Leaven `Lm`, provider adapters, and cache policy. | AIME live solver shells to Python and raw OpenAI Responses API (`examples/p8_aime_gepa/src/main.rs:293-301`; `examples/p8_aime_gepa/scripts/openai_solver.py:24-45`), and the example lacks LM crate deps (`examples/p8_aime_gepa/Cargo.toml:12-16`). | Live path bypasses the library substrate it claims to prove. | A user can run "live Leaven AIME" without exercising Leaven LM/cache. | Route live solver and reflector through Leaven runtime/LM/cache. | Feature-gated live smoke whose provider swap is only runtime/provider construction. |
| Reflection | GEPA reflective mutation consumes feedback assessment ids, evidence, traces, selected part, objective/background, and records provenance (`docs/specs/gepa_optimizer_surface.md:322-357`; `docs/specs/gepa_optimizer_surface.md:463-483`). | `SurfaceProposer` receives artifact/surface/part only (`crates/leaven-gepa/src/proposer.rs:6-19`); `ReflectiveMutation` returns one stored edit (`crates/leaven-gepa/src/proposer.rs:21-47`). | The public name says reflection, but implementation is a fixed edit fixture. | Score movement can happen without reflective learning. | Move fixed edits to fixture/demo paths; implement async evidence-aware reflector. | Mock-LM reflection test consumes actual scored feedback and improves a simple artifact. |
| Result/report | Result exposes optional best, stop reason, budget, graph-backed report, public events, GEPA summary, final-test semantics, and no ordinary `RunGraph` requirement (`docs/specs/gepa_public_private_surface.md:1184-1228`). | `OptimizeResult` requires `best`, clones artifacts, and `OptimizationReport` holds aggregate floats and event strings (`crates/leaven-run/src/result.rs:6-61`). Missing train evidence becomes `0.0` (`crates/leaven-run/src/builder.rs:452-457`; `crates/leaven-run/src/result.rs:64-71`). | The report is a thin snapshot and collapses absence/failure into numeric values. | Users cannot tell what won, why, what stopped, what evidence exists, or whether test influenced selection. | Make result/report graph-backed and truth-preserving. | Optional best, stop reason, absent/failed score, case delta, final-test-only, cache/cost, and public event tests. |

## Original Vision vs Current State

### What Is Real

- `leaven-run` does expose a high-level `optimize(seed)` builder and a real
  `.run().await` path (`crates/leaven-run/src/builder.rs:54-72`;
  `crates/leaven-run/src/builder.rs:201-286`).
- The builder requires explicit budget and scorer before execution
  (`crates/leaven-run/src/builder.rs:208-213`; `crates/leaven-run/src/error.rs:7-16`),
  and tests cover those current refusals (`crates/leaven-run/tests/optimize_builder.rs:23-52`;
  `crates/leaven-run/tests/optimize_builder.rs:90-105`).
- The builder lowers train/validation/test into engine partitions and hides
  validation/test from proposers with `TrustPolicy::hide_from_proposers`
  (`crates/leaven-run/src/builder.rs:214-240`).
- `leaven-lm` has a provider-neutral async `Lm` trait (`crates/leaven-lm/src/model.rs:9-22`),
  and `leaven-lm-openai` has an `OpenAiLm` provider implementation surface
  (`crates/leaven-lm-openai/src/client.rs:10-37`).

### What Is Not Yet Layer 1

- The runner/scorer contract is not the async, rich, metered contract the specs
  require.
- The work input contract does not support ordinary single-task/no-dataset mode
  or stable case ids by default.
- The public score contract cannot preserve real feedback evidence.
- Runtime/cache policy is not wired through solver/reflector/scorer roles.
- The canonical AIME example proves a fixed edit and provider bypass.
- The result facade is not the ordinary run-truth surface.
- The default import surface still leaks internals.

## The Root Delta

The original vision uses approachable Layer 1 words while preserving the
lower-level truth behind them. Current Layer 1 exposes some approachable words,
but the behavior underneath is too often a narrower proxy:

```text
optimize builder shell  != ordinary optimizer product
sync closure            != async LM/program/agent runner
f64 + String            != score/evidence/feedback contract
Vec position            != stable case identity
CachedLm wrapper        != runtime/cache role policy
fixed edit              != reflective mutation
aggregate floats        != graph-backed report
prelude of everything   != Layer 1 public surface
```

The correction is not to patch around the AIME example. The correction is to make
the public builder, score, runtime, reflection, and result contracts real enough
that the example becomes thin.
