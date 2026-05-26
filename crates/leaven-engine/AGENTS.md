## Boundary
This crate owns run execution: `Engine`, `RunGraph`, `RunContext`, graph views,
stage traits, case resolution, budget ledger, trust/read scopes, evaluation
cache, persistence envelopes, callbacks, reports, events, and the engine loop.

Graph mutation is private to this crate and exposed through `RunContext`.
External crates may observe through views and call stage/context APIs; they
must not reach into graph storage to make progress.

## Route Here
- Run records and views: candidates, proposal batches, apply attempts,
  assessments, evaluation requests, lineage, graph snapshots, and restore
  validation belong here.
- Mutation APIs: seed insertion, proposal recording/apply, evaluation,
  materialization/render contexts, cache use, event emission, and budget
  charging belong on `RunContext` or adjacent engine modules.
- Stage contracts: proposer, evaluator, preference relation, population,
  optimizer, stopper, callback, renderer, materializer, checkpointable
  optimizer, optimizer compatibility identities, type-erased optimizer report
  payloads, and dynamic trait adapters belong here.
- Runtime policy shared by all optimizers: trust/read scopes, case-set
  resolution, cache keys/status, persistence envelopes, reports, and stop/error
  event shapes belong here.
- Evaluation request records own runtime evaluator identity facts that are true
  inside the engine, including evaluator id, evaluator fingerprint, request
  shape, and resolved case-set identity. Public-seam job documents that add base
  revision, deadline, capability fingerprint, or worker transport facts must be
  produced by the appropriate seam/lowering owner instead of faked here.

## Route Away
- Cold proposal/evaluation/artifact/evidence/preference vocabulary belongs in
  `leaven-core`; do not define a second run-language here.
- Optimizer strategy state, search rhythm, GEPA gates/selectors, MIPRO,
  TextGrad, and trace policy belong in optimizer crates.
- Optimizer-specific report contents belong in optimizer crates. Engine may
  carry a type-erased report payload hook, but it must not know GEPA fields or
  synthesize optimizer strategy reports.
- Product-builder defaults, train/validation/test ergonomics, runner/scorer
  helpers, and default store wiring belong in `leaven-run`.
- Reusable evidence, preference, population, renderer, artifact, workspace,
  store, LM, and agent implementations belong in their owning crates. Engine
  owns traits and orchestration, not concrete providers or backends.

## Proof Anchors
- `tests/engine_contract.rs::graph_surface` proves graph mutation through `RunContext`,
  lineage/view behavior, snapshot restore validation, and proposal/evaluation
  record consistency.
- `tests/engine_contract.rs::context_services` is the fastest broad check for finalizing context
  services: evaluation requests, evidence storage, cache status, callbacks,
  trust refusals, and budget/event side effects.
- `tests/engine_contract.rs::context_services::evaluation_requests_record_evaluator_fingerprint_as_runtime_job_identity`
  proves `RunContext` records evaluator fingerprints on evaluation request
  records even when the evaluator ID is unchanged. It is not public-seam job
  closeout by itself; deadline, capability fingerprint, base revision, and
  public job projection still need their owning primitives.
- `tests/engine_contract.rs::stage_trait_contracts` proves static and dynamic stage trait
  contracts without moving strategy state into engine internals. It still uses
  raw stage contexts as dispatch fixtures; do not cite it alone as proof of the
  public finalization path.
- `tests/engine_contract.rs::engine_loop` proves run loop, callbacks, trust policy,
  checkpointing, persistence envelopes, and cache restoration.
- `tests/engine_contract.rs::{context_services,evaluator_registry,case_set_resolution,budget_laws,trust_policy}`
  are narrow proof anchors for their named services.
- `cargo test -p leaven-engine` proves engine-local execution contracts.
- `cargo test -p leaven --test topology_contract` proves dependency edges when
  this crate's manifest or public exports change.

## Decision Cards
- when: adding graph mutation or candidate/proposal recording behavior
  do: put mutation behind `RunContext` or an adjacent private engine service
  preserve: append-only graph records, causal vs informational lineage, and typed failed-apply evidence
  avoid: exposing `RunGraph` storage or constructors so callers/tests can mutate directly
  verify: run `cargo test -p leaven-engine --test engine_contract graph_surface`

- when: adding a costful stage path or letting optimizer authors call a stage
  do: route through a `RunContext` finalizer such as `propose`, `record_proposal_batch`, `apply_batch`, `evaluate`, or `evaluate_with`
  preserve: one place for graph writes, budget charge, cache status, event emission, trust checks, evidence storage, and checkpointing
  avoid: making `proposal_context`, `evaluation_context`, `render_context`, or `materialize_context` the ordinary public path; those are current public holes and should be treated as raw/non-finalizing until sealed
  verify: run `cargo test -p leaven-engine --test engine_contract context_services` and `cargo test -p leaven-engine --test engine_contract stage_trait_contracts`

- when: adding a cache, checkpoint, trust, or budget rule shared by optimizers
  do: implement the execution policy here and keep strategy-specific decisions in optimizer crates
  preserve: typed refusal and durable records for failed or hidden operations
  avoid: moving GEPA selectors, provider cache keys, or store backend layout into engine
  verify: run the narrow engine test named for the service, then `cargo test -p leaven --test topology_contract` if dependencies changed

- when: changing evaluation-set visibility or split hiding
  do: prove the resolved data exposure, not just the syntax of `EvaluationSet`
  preserve: validation/test hiding for optimizer and proposer search, including explicit case-id requests
  avoid: relying on `EvaluationSet::Cases` as a bypass around hidden partitions; the audit marks unresolved-shape trust checks as incomplete
  verify: run `cargo test -p leaven-engine --test engine_contract trust_policy`, `cargo test -p leaven-engine --test engine_contract case_set_resolution`, and `cargo test -p leaven-engine --test engine_contract context_services`

- when: changing engine evaluation cache behavior
  do: key by semantic request shape and make cache hits graph-visible for the current request
  preserve: request kind, granularity, purpose, pair/listwise semantics, case-set identity, evaluator fingerprint, and candidate cache identities
  avoid: treating engine `EvaluationCache` as the LM response cache or as proof of Layer 1 runtime/cache roles
  verify: run `cargo test -p leaven-engine --test engine_contract context_services` and `cargo test -p leaven-engine --test engine_contract evaluator_registry`

## Local Bait
- `docs/specs/public-seam-v1/` locks the engine-bearing facts of the public
  seam: capability-token authorization kernel, data-class taint propagation,
  replay-by-receipt semantics, plan evaluation, and the `RunContext`-as-only-
  graph-mutation-authority judgment. Treat it as the durable target for
  trust/cache/budget/event behavior visible across the worker boundary; the
  lowering work is not yet implemented here.
- `RunGraph` has public view types and snapshots, but mutation still goes
  through `RunContext`. Do not expose graph storage fields or constructors just
  to satisfy callers or tests.
- The engine depends on `leaven-surface` as part of the current topology, but
  concrete surface policy still belongs in artifact/optimizer crates unless
  the rule is a generic execution concern.
- `RunContext::propose` is the canonical proposer helper stack: build
  `ProposalContext`, call the async `Proposer<P>`, charge returned `Metered`
  cost, record the batch, emit proposal events, and checkpoint. GEPA's current
  local `SurfaceProposer` path bypasses that helper stack; do not copy it into
  new optimizer product paths.
- Persistence here is the run checkpoint envelope and codec boundary. Concrete
  byte stores and backend details belong in `leaven-store-*`.
- `SqliteEvaluationCache` is a focused exception for leaven-env.10: it persists
  the engine-owned `EvaluationCacheKey -> AssessmentId` index inside this crate
  because there is no behavior-bearing SQLite store crate. Do not expand it
  into LM response cache, optimizer state, evidence storage, or run graph schema;
  move the backend out when a real cache-capability crate exists.
- `Population` and `PreferenceRelation` traits live here because the engine
  calls them. Reusable implementations and fitted model state do not.
- `CachePolicy::Never` is the safe default for stochastic evaluators. Switching
  an evaluator to deterministic caching is a semantic claim about replayability,
  not a performance tweak.
