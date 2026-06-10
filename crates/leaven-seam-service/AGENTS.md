## Boundary

`leaven-seam-service` owns configured executable service implementations behind
the public seam runtime. It may compose the locked `leaven-public-seam` Plan IR
executor, provider-neutral effect traits, configured local subprocess stage
workers, and concrete local/mock provider crates that are explicitly configured
for a serve process.

It must not own stdio framing, CLI argument parsing, graph internals, schema
validation policy, or provider protocol details. Transport stays in
`leaven-seam-stdio`, dispatch and response validation stay in
`leaven-seam-runtime`, and concrete provider adapters stay in their provider
crates. Subprocess stage workers are a configured service implementation here,
and worker-initiated callback requests are serviced through the configured
service while the stage is active. Their public wire remains the locked
`leaven/stage.run` and `leaven/*` JSON-RPC methods.

RunContext-backed service execution also belongs here when the public SDK server
needs real graph mutation through `leaven seam serve --stdio`. This crate may
depend on `leaven-core`, `leaven-engine`, `leaven-run`, and store crates for
that configured execution mode, but only to compose `RunContext` mutation and
the existing public-seam receipt projection helpers. It must not inspect or
mutate `RunGraph` internals directly, become an optimizer strategy home, or
route durable SDK server behavior through `leaven-acp-stage-bridge`.

Current executable method status is product-facing and is recorded in
`../../docs/specs/public-seam-v1/executable-method-status.md`. Update that file
in the same change when this crate adds or removes configured service behavior,
changes a method from mock-only to live-provider-backed, or deliberately marks a
method unsupported.

## Map

- `service.rs`: configured Plan IR service composition for LM, workspace, and
  agent effects.
- `graph_state.rs`: private serve-process graph readback state for configured
  Plan IR graph writes. It may record schema-valid public graph rows for
  read-after-write proof inside one `leaven seam serve --stdio` process; it is
  not Rust `RunGraph` or durable checkpoint storage.
- `run_context_service.rs`: configured `RunContext`-backed graph-write service
  mode for durable public-seam execution. It composes engine mutation and
  `leaven-run` public-seam receipt projection; it must not use `SeamGraphState`
  as graph-truth evidence.
- `lm.rs`: configured LM provider selection for mock and OpenAI-backed
  `leaven/lm.complete`. Public dependencies are `leaven-lm` and configured
  provider crates; provider protocol details stay in the provider crates.
- `stage.rs`: runner/proposer stage service configuration, dispatch, and
  callback loop. Public dependencies are `leaven-public-seam` stage/effect
  semantics and the standard library subprocess boundary; private helpers stay
  in this module. `command_runner_result` is the reusable subprocess
  stage-dispatch transport; the optimize.run host reuses it for runner and
  scorer dispatch instead of duplicating the worker loop.
- `optimize_run_service/`: the GEPA-over-seam host for `leaven/optimize.run`.
  It lowers the locked optimize-run request into the `leaven-run` builder, runs
  the real `leaven-gepa` loop, dispatches per-case runner and scorer stages to
  the configured command worker, services nested `leaven/lm.complete` callbacks
  and capability-gated case reads, and projects the durable `Optimized` result
  plus GEPA frontier into the locked `leaven.optimize_run.v1` result document.
  It owns lowering, worker composition, and projection only. GEPA search policy
  stays in `leaven-gepa`, wire law stays in `leaven-public-seam`, and graph
  mutation stays behind `RunContext` through the `leaven-run` builder. The
  `prompt` artifact type, `instance` objective, and `lm` reflection kind are the
  executable V1 surface; every other config value is refused with a message
  naming what is supported (population_size/minibatch_size are refused because
  V1 uses the fixed per-case Pareto frontier and reference minibatch). The
  `applied_proposals` receipts are opaque service-issued `wrec_` ids bound 1:1
  to the run's durable candidate-apply records; they name graph truth, not
  inline writes.

## Verification

When changing executable service behavior, run:

```bash
cargo test -p leaven-seam-service
```

The optimize.run host loop law is
`optimize_run_service::tests::optimize_run_drives_the_real_gepa_loop_to_a_changed_re_evaluated_child`:
it proves a changed child is applied and re-evaluated onto the frontier, asserts
the exact reference metric-call count (1 seed + 3 parent + 3 child + 1
validation = 8), and validates the projected document. The companion target
isolation law refuses `leaven/case.target` during runner dispatch and serves it
with a receipt during scorer dispatch. When changing host lowering, worker
dispatch, or projection, keep these laws killing the wrong implementation.

The optimize.run loop runs under a tokio current-thread runtime so OpenAI-backed
reflection has a reactor (`futures::executor::block_on` would panic with no
reactor). Worker `leaven/lm.complete` callbacks build their own nested provider
runtime, so the host's `lm_handler` runs each callback on a scoped helper thread
to keep that nested runtime off the loop runtime's thread (avoiding "Cannot start
a runtime from within a runtime"). Two laws guard this: the no-network
`openai_backed_reflection_executes_through_the_optimize_run_executor` test drives
the full loop through a `SeamLmConfig::OpenAi` reflector against a loopback fake
provider, and `worker_effect_cost_aggregates_into_result_cost_totals` exercises a
worker that issues `leaven/lm.complete` under the loop runtime. If you change the
loop executor or the LM callback threading, keep both passing.

If dependencies or crate boundaries change (the host adds the
`leaven-seam-service -> leaven-gepa`/`leaven-eval`/`leaven-surface` edges), also
run:

```bash
cargo test -p leaven --test topology_contract
```
