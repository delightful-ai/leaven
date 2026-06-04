# Run-Bound Durable RunContext Service

Status: topology decision recorded; product-route implementation pending
Updated: 2026-06-04T03:25:00Z

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
- `RunBoundGraphEffectService` now implements `leaven-seam-runtime::SeamService`.
  Its stdio-focused test routes all four graph-write methods through
  `SeamRuntime` and `leaven-seam-stdio::serve_reader_writer`, so the proof no
  longer stops at direct module calls.
- `engine_lifecycle_mounts_run_bound_service_and_checkpoint_readback_sees_graph_truth`
  mounts `RunBoundGraphEffectService` from inside an actual
  `Optimizer::step` where the engine has installed case set, evidence store,
  callbacks, trust policy, and persistence on the live `RunContext`. The test
  sends all four graph-write methods through `SeamRuntime` plus stdio framing,
  lets the engine finish and advance latest checkpoint state, and restores the
  checkpoint to assert graph truth survived.

## Why This Is Not Done

The configured service path owns an in-memory `SeamTextProblem` graph and
aliases such as `pb_configured_run_context`, `eval_run_context`, and
`run_context.checked`. It is mechanics proof for the public server route, not
the generalized SDK service.

The remaining missing proof is wiring this service shape into a product public
SDK route. The current module proves the generic service, runtime/stdio delivery,
durable checkpoint mechanics, and engine optimizer-lifecycle mounting, but not a
product API/CLI route that external-language SDK workers can launch for an
ordinary run.

The important topology consequence is that the next row-closing change is not a
larger `ConfiguredSeamService` patch and not a `leaven-run -> leaven-seam-service`
dependency. The run/stage owner must hold the live `RunContext`, case set,
evidence store, persistence, optimizer checkpoint state, and typed
problem-specific lowerers. `leaven-seam-service` may provide the adapter that
implements `SeamService`, but it must be constructed by the run/stage owner while
that run is active.

Do not close `run_bound_durable_runcontext_service` with:

- `SeamGraphState` read-after-write rows;
- `SeamTextProblem` configured service summaries;
- `leaven-acp-stage-bridge` callback tests alone;
- top-level `leaven serve --stdio --plan --out` bridge-demo behavior;
- public-seam receipt projection tests alone;
- Python `optimized.json` or Python-only inspection.

## Topology Decision

Ownership stays split:

- `leaven-engine`: `RunContext`, `RunGraph`, mutation, checkpoint request
  construction, and `RunPersistence` capability.
- `leaven-run`: user-facing durable run setup, `LocalOptimizeStore`,
  `StoreRunPersistence`, evidence stores, optimizer lifecycle composition, and
  public-seam receipt projection over engine-backed reports. It already prepares
  the durable run store, starts/resumes the engine, installs persistence, runs
  the optimizer, advances latest checkpoints, and writes public run reports.
  It must not directly depend on `leaven-seam-service`, because
  `leaven-seam-service` already depends on `leaven-run` for receipt projection.
- `leaven-seam-service`: configured executable service composition. It may host
  a run-bound service adapter when a public SDK server needs to service worker
  callbacks for a specific run, but it must not inspect graph internals or
  become an optimizer strategy home.
- `leaven-seam-runtime`: method/profile dispatch and response validation only.
- `leaven-seam-stdio`: line-delimited transport only.
- `leaven-acp-stage-bridge`: bidirectional stage callback proof and reusable
  host-effect shape, not the durable public SDK server route.
- a new behavior-bearing composition layer above `leaven-run` and
  `leaven-seam-service`: product SDK-run orchestration when one route must own
  both ordinary run lifecycle and public-seam worker serving. This layer may
  depend on both crates and on `leaven-seam-runtime`/`leaven-seam-stdio`, but it
  must have its own boundary docs, topology rows, tests, and public maturity
  classification before it is treated as the row-closing route.

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
3. Add an explicit run-lifecycle mount point for worker callbacks during a real
   run/stage, not a free-floating configured proof graph. The implementation
   must respect the existing dependency direction: `leaven-run` owns the product
   lifecycle facts, while `leaven-seam-service` owns the service adapter and
   already depends on `leaven-run`. The foundation mount inside
   `leaven-seam-service` tests is done; the row-closing route now needs a
   behavior-bearing composition layer above both crates so it can own builder
   lifecycle and seam serving without creating a dependency cycle.
4. After each graph-write callback, rely on `RunContext` checkpointing through
   `.with_persistence(...)`; where optimizer-private state must advance latest,
   call `checkpoint_with_optimizer_state(...)` from the optimizer owner.
5. Add a focused scenario that runs a real small problem through the
   engine/optimizer lifecycle, mounts the run-bound public-seam service while
   the run is active, services proposal/evaluation/assessment/event callbacks
   through the runtime/stdio path, then reads back the latest checkpoint through
   Rust-owned inspection and asserts the same child candidate, evaluation
   request, assessment ids, emitted event, receipts, lineage, and cost facts.
   **Done as engine-lifecycle proof in `run_bound_service/tests.rs`; still needs
   product-route/API proof for external-language SDK workers.**
6. Add a negative proving that a fallback configured receipt or `SeamGraphState`
   summary cannot satisfy the run-bound route.

## Research Result

The next implementation should preserve this dependency direction:

- `leaven-run` constructs or owns the real lifecycle scenario because it already
  prepares `PreparedStore`, durable run dirs, `StoreRunPersistence`, engine
  start/resume, optimizer execution, final checkpoint advancement, and report
  readback. It cannot directly import `leaven-seam-service` under the current
  topology.
- `leaven-seam-service::run_bound_service` remains a reusable adapter over a
  borrowed live `RunContext<P>` plus host-owned typed lowerers.
- A future behavior-bearing composition crate above `leaven-run` and
  `leaven-seam-service` is preferable to reversing the existing dependency if
  the product route must own both ordinary run builder lifecycle and seam server
  mounting.
- `leaven-cli::seam serve --stdio` remains the configured operator server for
  service-mode method execution. It cannot by itself own an optimizer/run/stage
  lifecycle without becoming a product-run implementation bucket.
- top-level `leaven serve --stdio --plan --out` is legacy bridge-demo/provenance.
  It proves bidirectional stage dispatch but not `leaven-run` product durability,
  `RunContext` graph truth, or Rust-owned inspection. A successor should live
  under the public seam command family (for example a hard-cut `leaven seam run
  --stdio ...`) or in an explicitly named composition crate that the CLI calls.
- `leaven-acp-stage-bridge` remains bridge evidence and reusable design input,
  not a dependency of the durable public SDK server route.

## Product Route Decision

The row-closing implementation should introduce a real composition owner rather
than stretching existing crates:

1. Add a behavior-bearing public-seam run composition crate above
   `leaven-run` and `leaven-seam-service`. It owns the launchable SDK-run route:
   run plan/config loading, external worker process/session launch, stage
   dispatch, worker callback service mounting, run directory selection, and final
   proof readback. It does not own optimizer strategy, graph mutation, wire
   schemas, stdio framing, provider protocols, or Python SDK ergonomics.
2. Wire CLI entry through the `leaven seam ...` family, not the old top-level
   bridge-demo `serve` command. The new route should be a hard cutover for
   production evidence; the old command can remain only as explicitly labelled
   provenance until removed.
3. The first behavior-bearing proof should be deterministic and no-spend:
   a small prompt-like problem, a checked-in external worker that issues at least
   one graph-write callback, and a durable local run dir. It must restore the
   latest Rust checkpoint and assert the candidate/evaluation/assessment/event
   facts from graph truth rather than a projected result file.
4. Once the Rust route exists, Python `lv.optimize(...).run()` should move toward
   spawning that route instead of orchestrating a standalone configured
   `leaven seam serve --stdio --config` process and writing `optimized.json` as
   the source of truth.

## Focused Verification Target

Minimum closeout for the composition-route slice:

- `CARGO_INCREMENTAL=0 cargo test -p <composition-crate> <run-bound route test name>`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-seam-service run_bound_service`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-run inspection`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-cli <new seam route test name>`
- `cargo test -p leaven --test topology_contract`
- YAML evidence update under `run_bound_durable_runcontext_service`

Broader closeout must add the Python SDK inspection/open proof and the live
Codex evolution proof from the main goal.
