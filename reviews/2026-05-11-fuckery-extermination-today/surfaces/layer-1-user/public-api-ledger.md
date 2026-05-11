# Layer 1 Public API Ledger

Status: active findings recorded.

Layer 1 covers ordinary users who want to run an optimizer over a program,
cases, and a score function without learning engine internals.

## Findings

### L1-001: The ordinary prelude exports engine internals

- severity: medium
- evidence: `crates/leaven/src/prelude.rs:3-23`,
  `docs/specs/gepa_public_private_surface.md:55-83`
- promised behavior: Layer 1 users should touch `optimize(seed)`, train /
  validation / test inputs, runner, scorer, budget, GEPA defaults, and result
  facades. They should not need `RunGraph`, `RunContext`, `TrustPolicy`,
  `Evaluator`, `Population`, or low-level evaluation requests.
- actual behavior: `leaven::prelude::*` re-exports `RunContext`,
  `RunGraphView`, `TrustPolicy`, `Evaluator`, `MaterializeContext`,
  `Renderer`, `Population`, `Proposer`, and other engine-author names beside
  ordinary `optimize`, `Score`, and `RunOutput`.
- why it matters: the first import surface teaches implementation machinery as
  the default product surface. Users can reasonably assume those names are
  normal setup requirements.
- correction direction: hard-cut the ordinary prelude to the Layer 1 story.
  Move engine-author names to an explicit advanced or engine prelude.

### L1-002: The builder does not support the single-task search mode

- severity: high
- evidence: `crates/leaven-run/src/builder.rs:56-114`,
  `docs/specs/gepa_public_private_surface.md:101-110`,
  `docs/specs/gepa_public_private_surface.md:726-734`
- promised behavior: users should be able to optimize one candidate against a
  task or environment without first constructing train / validation / test
  datasets. The mode is inferred from the inputs.
- actual behavior: the only public type-fixing entrypoint after
  `optimize(seed)` is `.train(...)`. There is no `.task(...)`,
  `.environment(...)`, or equivalent single-task path.
- why it matters: the public surface forces every optimization into dataset
  semantics even when the intended use is an agentic task or interactive
  environment.
- correction direction: add the single-task lowering path or remove the
  promise. Do not make users fake a one-item training set to reach the engine.

### L1-003: Runner and scorer callbacks are sync-only

- severity: blocker
- evidence: `crates/leaven-run/src/builder.rs:28-29`,
  `crates/leaven-run/src/evaluator.rs:15-16`,
  `crates/leaven-run/src/evaluator.rs:97-102`,
  `docs/specs/gepa_public_private_surface.md:141-153`,
  `docs/specs/gepa_public_private_surface.md:1029-1049`
- promised behavior: ordinary users can run LM, subprocess, workspace, or
  agent programs and score their outputs through async-capable runner/scorer
  contracts.
- actual behavior: `Runner` and `Scorer` are synchronous `Fn` callbacks.
  Evaluation calls runner and scorer serially in-process.
- why it matters: real LM calls, model judges, subprocesses, and agents force
  hidden runtimes, blocking, or shell escapes. The AIME example demonstrates
  this by leaving Leaven for Python.
- correction direction: hard-cut to async `CandidateRunner` and async scorer
  lowering. Sync closures can remain only as adapters into the async path, not
  as the underlying product contract.

### L1-004: The public builder does not expose runtime/cache policy

- severity: high
- evidence: `crates/leaven-run/src/builder.rs:122-198`,
  `crates/leaven-run/src/evaluator.rs:61`,
  `docs/specs/lm_runtime_and_response_cache.md:15-31`
- promised behavior: repeated LM/agent runs should be cacheable through an
  ordinary run/runtime policy, while cache traits/backends remain swappable for
  power users.
- actual behavior: the builder has no solver/reflector runtime-cache policy.
  `ScoringEvaluator::cache_policy()` always returns `CachePolicy::Never`.
- why it matters: expensive optimizer runs re-spend by default, and users must
  learn implementation wrappers instead of configuring the run they asked for.
- correction direction: wire response-cache policy through the ordinary run
  builder for solver and reflector roles. Keep lower-level cache stores behind
  explicit advanced APIs.
