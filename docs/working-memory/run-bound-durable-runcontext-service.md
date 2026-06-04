# Run-Bound Durable RunContext Service

Status: implementation foundation in progress
Updated: 2026-06-04T01:35:31Z

## Intent

The production goal needs more than configured method execution. The retained
`leaven/*` graph-write methods must be serviceable for external-language SDK
workers while bound to a real run/stage lifecycle, with graph mutation through
`RunContext`, durable checkpoint persistence, and Rust-owned checkpoint readback
of the same mutated facts.

## Current Proven Pieces

- `leaven seam serve --stdio` enters through `crates/leaven-cli/src/seam.rs`,
  then `leaven-seam-stdio` line framing and `leaven-seam-runtime` method/profile
  validation dispatch to `ConfiguredSeamService`.
- `crates/leaven-seam-service/src/run_context_service.rs` proves the retained
  graph-write method names can route into `RunContext` instead of stopping at
  validation:
  - `leaven/proposal.apply` calls `RunContext::apply_batch`.
  - `leaven/evaluation.request` calls `RunContext::request_evaluation`.
  - `leaven/assessment.submit` calls `RunContext::submit_assessments`.
  - `leaven/event.emit` calls `RunContext::emit`.
- `crates/leaven-cli/tests/seam_stdio_server.rs` proves the process-level CLI
  route can exercise those configured methods and read back schema-valid summary
  state.
- `crates/leaven-acp-stage-bridge/src/graph_host.rs` proves worker-initiated
  stage callbacks can cross into `RunContext` through host-owned typed lowerers.
- `crates/leaven-run/src/public_seam/*` owns the graph-backed public-seam
  receipt projections and rejects forged graph facts.
- `RunContext` already checkpoints after mutating operations when constructed
  with `.with_persistence(...)`.
- `crates/leaven-seam-service/src/run_bound_service/` now provides a generic
  run-bound graph-effect service. The public surface lives in `mod.rs`; JSON
  parsing, receipt projection, errors, and focused tests live in private sibling
  modules. Its focused test binds a real
  `RunContext<P>` with `StoreRunPersistence<FileStore>`, records a proposal
  batch, services `proposal.apply`, `evaluation.request`, `assessment.submit`,
  and `event.emit`, validates each extension result through
  `leaven-public-seam`, advances a latest checkpoint at a clean run boundary,
  and restores the checkpoint to prove candidate/evaluation/assessment/event
  graph facts survived durable readback.

## Why This Is Not Done

The configured service path owns an in-memory `SeamTextProblem` graph and
aliases such as `pb_configured_run_context`, `eval_run_context`, and
`run_context.checked`. It is mechanics proof for the public server route, not
the generalized SDK service.

The remaining missing proof is wiring this service shape into the public SDK
server route during a real optimizer run/stage lifecycle. The current module
proves the generic service and durable checkpoint mechanics, but not
`leaven seam serve --stdio` process delivery for a run-bound service.

Do not close `run_bound_durable_runcontext_service` with:

- `SeamGraphState` read-after-write rows;
- `SeamTextProblem` configured service summaries;
- `leaven-acp-stage-bridge` callback tests alone;
- public-seam receipt projection tests alone;
- Python `optimized.json` or Python-only inspection.

## Topology Decision

Ownership stays split:

- `leaven-engine`: `RunContext`, `RunGraph`, mutation, checkpoint request
  construction, and `RunPersistence` capability.
- `leaven-run`: user-facing durable run setup, `LocalOptimizeStore`,
  `StoreRunPersistence`, evidence stores, and public-seam receipt projection
  over engine-backed reports.
- `leaven-seam-service`: configured executable service composition. It may host
  a run-bound service adapter when a public SDK server needs to service worker
  callbacks for a specific run, but it must not inspect graph internals or
  become an optimizer strategy home.
- `leaven-seam-runtime`: method/profile dispatch and response validation only.
- `leaven-seam-stdio`: line-delimited transport only.
- `leaven-acp-stage-bridge`: bidirectional stage callback proof and reusable
  host-effect shape, not the durable public SDK server route.

The durable implementation should reuse the `RunContextGraphEffectHost` shape:
public JSON stays at the boundary, host-owned typed lowerers convert to concrete
engine values, and graph mutations route through `RunContext` finalizers.

## Implementation Shape

1. Define a run-bound service state in `leaven-seam-service` that is generic over
   a concrete `OptimizationProblem` or is constructed by a typed owner with:
   `&mut RunContext<P>`, case set, evidence store, optional evaluation cache, and
   persistence. **Done as foundation in `run_bound_service/`; still needs a
   public server entrypoint.**
2. Reuse or move the host-effect lowering pattern from
   `leaven-acp-stage-bridge::RunContextGraphEffectHost` without making
   `leaven-acp-stage-bridge` a dependency of the durable server route.
3. Add an explicit public seam service entrypoint for worker callbacks during a
   real run/stage, not a free-floating configured proof graph.
4. After each graph-write callback, rely on `RunContext` checkpointing through
   `.with_persistence(...)`; where optimizer-private state must advance latest,
   call `checkpoint_with_optimizer_state(...)` from the optimizer owner.
5. Add a focused scenario that runs a real small `RunProblem`, services
   proposal/evaluation/assessment/event callbacks, then reads back the latest
   checkpoint through Rust-owned inspection and asserts the same child
   candidate, evaluation request, assessment ids, emitted event, receipts,
   lineage, and cost facts.
6. Add a negative proving that a fallback configured receipt or `SeamGraphState`
   summary cannot satisfy the run-bound route.

## Focused Verification Target

Minimum closeout for the first implementation slice:

- `CARGO_INCREMENTAL=0 cargo test -p leaven-seam-service <run-bound test name>`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-run inspection`
- `cargo test -p leaven --test topology_contract`
- YAML evidence update under `run_bound_durable_runcontext_service`

Broader closeout must add the Python SDK inspection/open proof and the live
Codex evolution proof from the main goal.
