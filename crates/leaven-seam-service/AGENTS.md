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
  mutation stays behind `RunContext` through the `leaven-run` builder. Two
  artifact types are executable: `prompt` (LM reflection) and `agent_kit`
  (Git-backed agentic reflection). The `instance` objective is the executable
  objective; every other config value is refused with a message naming what is
  supported. The artifact type fixes the reflection kind: `prompt` requires `lm`
  reflection and refuses `agentic` naming `lm`; `agent_kit` requires `agentic`
  reflection and refuses `lm` naming `agentic`. `population_size` lowers into the
  GEPA candidate-pool cap (`Gepa::max_candidates`) as a stop condition over the
  seed plus loop-authored children, and `minibatch_size` lowers into the GEPA
  train screening minibatch override (`Gepa::train_minibatch_size`, applied after
  `with_profile` and order-independent). A service law refuses `population_size`
  of 1 naming the `>= 2` bound (a cap of 1 admits only the seed); the wire schema
  enforces `>= 1`, so the `>= 2` bound is service-layer law, like `objective` !=
  `instance`. The
  `applied_proposals` receipts are opaque service-issued `wrec_` ids bound 1:1
  to the run's durable candidate-apply records; they name graph truth, not
  inline writes.
- `optimize_run_service/agent_kit/`: the Git-backed AgentKit optimization path.
  It owns the `agent_kit` wire-record-to-Git-file projection (mapping the wire
  `{system_prompt, skills}` projection onto a `manifest.toml` /
  `system_prompt.md` / `skills/<path>` file map and back), the kit GEPA loop over
  a `GitProgramArtifact` with the `GepaGitProgramAgenticReflector`, the kit
  candidate snapshot, and the kit result projection. The run-scoped Git seed
  construction (deterministic commit identity) and revision readback are NOT owned
  here: they live in `leaven-agentic-git` (`build_program_seed` /
  `read_revision_files`); the real `EditSurface<GitProgramArtifact>`
  (`GitProgramPathSurface`) and agentic reflector live in
  `leaven-gepa-agentic-git`. Each kit candidate is projected per-revision into the
  runner payload under the `candidate_agent_kit` key (sibling of the prompt path's
  `candidate_template`), so a worker reads the flat kit content. The agent runtime
  is resolved from `SeamAgentConfig::CodexCli` for the live path; deterministic
  tests inject a scripted `FakeAgentRuntime` through a `#[cfg(test)]`-only service
  slot (never a serde config or public scaffold). An agentic `agent_kit` run with
  no configured agent runtime is refused with the method-unavailable style.

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

The Git-backed AgentKit loop law is
`optimize_run_service::tests::optimize_run_drives_the_real_git_backed_agent_kit_loop_with_agentic_reflection`:
it seeds a run-scoped Git repo from the wire kit projection, runs the agentic
reflection loop with a scripted `FakeAgentRuntime` that authors a changed
`system_prompt.md`, and proves the changed kit child is applied and re-evaluated
onto the frontier (best != seed, parent == seed, evolved system prompt visible in
the `agent_kit` projection, unchanged skill preserved, exact 8 metric calls,
applied_proposals non-empty). It also proves the first-class
feedback-reaches-reflection requirement: the scorer-provided per-case feedback
text appears in the rendered reflection instructions the agent read (captured via
the test-support task-capture hook). Companion refusal laws cover prompt+agentic
(naming `lm`), agent_kit+lm (naming `agentic`), and agentic agent_kit without a
configured agent runtime (unavailable).

If dependencies or crate boundaries change (the kit path adds the
`leaven-seam-service -> leaven-agentic-git`/`leaven-artifact-git`/`leaven-gepa-agentic-git`/`leaven-agentic`
edges on top of the prompt path's `leaven-gepa`/`leaven-eval`/`leaven-surface`
edges), also run:

```bash
cargo test -p leaven --test topology_contract
```
