# Public Seam V1 Executable Method Status

This document records what the durable `leaven seam serve --stdio` route can
execute today. It is part of the active public-seam V1 package because method
availability is product truth for SDK authors, not only implementation trivia.

`leaven seam serve --stdio` is the public SDK server route. It validates
line-delimited JSON-RPC through `leaven-seam-stdio`, dispatches locked
`leaven/*` methods through `leaven-seam-runtime`, and executes configured
services through `leaven-seam-service`.

`leaven serve --stdio --plan --out` is legacy bridge-demo/provenance. It is not
the SDK substrate and must not be used as closeout evidence for Python SDK or
Codex/agentic public-seam readiness.

## Status Classes

- **Validated-only:** the locked method/schema/envelope is accepted by the
  runtime dispatcher, but `NullSeamService` or an unconfigured service returns a
  method-unavailable or explicit unsupported-provider error.
- **Mock-configured:** the configured service executes the method family with
  deterministic local providers. This is mechanics evidence only.
- **Configured local:** the configured service executes against local resources
  such as subprocess stage workers or local workspaces.
- **Live-provider configured:** the configured service can call a real provider
  when credentials and opt-in runtime config are present.
- **Unsupported in service:** the V1 contract may define the method, but the
  current configured service intentionally returns an unsupported error.

## Current Executable Families

| Method or family | Current service status | Owner | Notes |
|---|---|---|---|
| `leaven/stage.run` | Mock-configured and configured local | `leaven-seam-service::stage` | `MockRunner` returns deterministic runner text. `CommandRunner` dispatches a JSON-RPC `leaven/stage.run` request to a configured subprocess worker and services nested worker callbacks while the stage is active; it forwards the validated request unchanged, so scorer dispatch (`ScoreContext` payload, reward-vector result) flows through the same command-worker route. The Python SDK worker serves the scorer stage as of 2026-06-10: it reads the case target through capability-gated, receipted `leaven/case.target` callbacks and runs the registered `@lv.reward` rubric vector. Python SDK `lv.optimize(...).run()` drives the real GEPA loop through `leaven/optimize.run`, which dispatches runner and scorer stages back to this command-worker route. |
| `leaven/optimize.run` | Mock-configured and configured local | `leaven-seam-service::optimize_run_service` plus `leaven-gepa` | Client->host optimization dispatch executing the real `leaven-gepa` loop. `ConfiguredSeamService` lowers the locked `leaven.optimize_run.v1` request into the `leaven-run` builder, runs the GEPA frontier/reflection/admission loop, dispatches per-case runner and scorer stages to the configured `SeamStageConfig::CommandRunner` worker over `leaven/stage.run`, services nested `leaven/lm.complete` callbacks and capability-gated case reads, and projects the durable `Optimized` result plus GEPA frontier into a `leaven.optimize_run.v1` result document that revalidates before return. Deterministic tests prove a changed child is applied and re-evaluated onto the frontier with the exact reference metric-call count (1 seed + 3 parent + 3 child + 1 validation = 8), target isolation (runner refuses `case.target`, scorer serves it with a receipt), worker effect-cost aggregation, and budget-truthful stopping. The seed and frontier artifact records parse into a typed payload discriminated by `artifact_type`: a `prompt` template, or an `agent_kit` projection of a Git-backed AgentKit revision (system_prompt slot content plus path-validated skill files mounted under the Codex `.agents/skills` mount); the wire layer rejects an agent_kit projection missing `system_prompt` or carrying a skill path that is absolute or contains parent traversal, and the host owns repository construction and readback so the wire record is a projection, not the artifact. V1 executes only the `prompt` artifact type, the `instance` objective, and `lm` reflection; the `agent_kit` artifact projection (the Git-backed AgentKit host slice lands later), other artifact types, objectives, and the `agentic` reflection kind are refused with a message naming what is supported. `optimizer.population_size` is honored as the candidate-pool cap (a stop condition over the seed plus loop-authored children, stopping the loop like budget exhaustion) and `optimizer.minibatch_size` is honored as the train screening minibatch override; a `population_size` of 1 is refused naming the `>= 2` service bound because a cap of 1 admits only the seed and can never author a child. Budget facts: `optimizer.max_metric_calls` always bounds the loop, and the optional `optimizer.max_cost_usd_micro` adds a `usd_micro` cost-axis ceiling enforced by the engine budget ledger; a deterministic test (2026-06-10) proves a usd ceiling below one child's screening spend stops the loop with the seed as best. A missing stage-worker argv returns the method-unavailable style. OpenAI-backed reflection through `SeamLmConfig::OpenAi` executes under the host's tokio loop runtime; a deterministic no-network test drives the full loop with the `OpenAi` reflector against a loopback fake provider. A credentialed live run through the Python SDK (`sdk/python/examples/14_live_optimize_aime.py`, 2026-06-11) drove the full loop on real AIME data with a `gpt-4.1-mini` solver and `gpt-5.4-mini` reflection served by one `SeamLmConfig::OpenAi` provider (the provider honors the per-call model the solver `lm.complete` and the reflection request each carry, since `SeamLmConfig::OpenAi` has no pinned model): the seed scored 0.0, GEPA authored a changed reasoning instruction, screened it past the parent on the train minibatch, admitted it, and re-validated it to 1.0 onto the frontier in 12 metric calls. USD cost reports 0.0 because Leaven meters token usage, not per-model dollar pricing, so the run is bounded by `metric_calls`, not the optional usd ceiling. Unlike worker-profile methods, this is a client->host dispatch, not a worker callback or stage dispatch, so the worker profile does not advertise it. |
| `leaven/lm.complete` | Mock-configured and live-provider configured | `leaven-seam-service::lm` plus provider crates | `Mock` uses `leaven-lm-mock` deterministic scripts. `OpenAi` uses `leaven-lm-openai` and requires the configured API-key environment variable. Missing credentials are an execution failure, not a mock success. |
| `leaven/workspace.materialize` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Allocates a local workspace and writes configured seed files. Workspace handles are local to the executing Plan document/callback flow. |
| `leaven/workspace.release` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Releases a workspace handle materialized earlier in the same Plan document/callback flow and returns a released workspace handle with receipts. |
| `leaven/workspace.snapshot`, `leaven/workspace.list`, `leaven/workspace.read_file`, `leaven/workspace.stat`, `leaven/workspace.digest`, `leaven/workspace.capture_artifacts` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Executes finite reads against the local workspace view with capability-scoped read ops. `capture_artifacts` returns requested file entries with byte counts, SHA-256, inline base64 bytes, and matching `blob_ref` metadata through `leaven seam serve --stdio`. |
| `leaven/workspace.git_log`, `leaven/workspace.git_diff`, `leaven/workspace.git_status` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Executes bounded Git commands inside an initialized local workspace and returns source-ref-bound `workspace_diff` values. The CLI proof uses a seed commit plus tracked post-commit edit so log, diff, and porcelain status all carry real Git output. |
| `leaven/graph.query`, `leaven/case.load`, `leaven/case.input`, `leaven/case.target`, `leaven/case.metadata` | Configured local | `leaven-seam-service::service` | Reads configured graph items, case records, and schema-valid serve-process graph write summaries through Plan IR `graph_query` / `case_query.load`, enforces case-read capability scope before host reads, returns method-specific typed `graph_set` / `case_record` primaries, and is covered through `leaven seam serve --stdio`. This proves configured read-after-write state inside one serve process, not yet Rust `RunGraph` checkpoint readback. |
| `leaven/agent.run` | Live-provider configured when paired with materialized workspace | `leaven-seam-service::service` plus `leaven-agent-codex-cli` | Uses configured Codex CLI runtime against a materialized local workspace and projects transcript/command output blob refs into the public seam result. Without `SeamAgentConfig::CodexCli`, the service returns an explicit unsupported-provider error. |
| `leaven/sandbox.exec` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Executes the lowered workspace command inside a materialized local workspace, captures stdout/stderr and declared output files, and returns byte-bound blob refs plus a sandbox call receipt through `leaven seam serve --stdio`. |
| `leaven/proposal.submit_batch` | Configured local write receipt with configured graph readback | `leaven-seam-service::service` | Produces a validated proposal-batch receipt for submitted proposal payloads and records a schema-valid serve-process graph summary readable by later `graph.query`. This proves public-seam write/readback plumbing, not Rust optimizer admission. |
| `leaven/proposal.apply` | RunContext-backed configured local graph write | `leaven-seam-service::run_context_service` | `leaven seam serve --stdio` can route an explicitly configured proposal batch through `RunContext::apply_batch`, project the graph-backed apply report through `leaven-run` public-seam receipts, validate the extension result, and read back a schema-valid graph summary showing the seed-to-child candidate advance. |
| `leaven/assessment.submit` | RunContext-backed configured local graph write | `leaven-seam-service::run_context_service`; `leaven-store-inline` | `leaven seam serve --stdio` can route assessment submission for the service-owned evaluation request through `RunContext::submit_assessments`, store typed evidence in an inline evidence store, project the graph-backed assessment report through `leaven-run`, validate the extension result, and read back the recorded assessment ids. |
| `leaven/evaluation.request` | RunContext-backed configured local graph write | `leaven-seam-service::run_context_service` | `leaven seam serve --stdio` can route an explicitly configured evaluation request through `RunContext::request_evaluation`, using service-owned typed request lowering and a concrete case set, then project the graph-backed job/receipt through `leaven-run` and read back the real `evalreq_<uuid>` used by later assessment submission. |
| `leaven/event.emit` | RunContext-backed configured local graph write | `leaven-seam-service::run_context_service` | `leaven seam serve --stdio` can route an explicit `run_context.checked` event through `RunContext::emit(RunEvent::ExternalEventEmitted { ... })`, return a receipt-bound extension result, and read back the emitted event metadata plus engine event count from the service-owned RunContext graph state. |

The RunContext-backed configured local graph-write rows above prove that the
locked backbone method names no longer stop at validation or configured receipt
plumbing. They do not, by themselves, prove the long-term SDK worker service
shape: a serve process bound to a real optimizer/run/stage lifecycle, real
problem-specific lowering, durable run/checkpoint stores, and checkpoint
readback of the mutated run. That generalized service proof is tracked in the
production goal ledger and must not be replaced by the configured
`SeamTextProblem` path.

## Validated But Not Executed By The Current Service

The runtime dispatcher exposes every locked worker-profile method and validates
request/response envelopes before and after service calls. No locked V1
`leaven/*` worker-profile method remains validated-only in the configured
service.

- watch behavior remains deferred to a future V1.x slice

Public-seam contract validators, Plan IR lowering helpers, and representative
harness evidence in `leaven-public-seam` are still not enough on their own; a
locked V1 method must execute through `leaven seam serve --stdio` or leave the
locked profile.

## Proof Anchors

- `crates/leaven-seam-runtime/tests/runtime_contract.rs` proves every locked
  method reaches the transport-neutral service boundary after validation.
- `cargo test -p leaven-seam-runtime` proves runtime dispatch and
  response-validation behavior.
- `cargo test -p leaven-seam-stdio` proves line-delimited stdio transport over
  the runtime dispatcher.
- `cargo test -p leaven-seam-service` proves configured execution for the
  method families listed as executable above, including command-worker callback
  loops for LM, agent, and proposal submission.
- Live-provider acceptance still requires credentialed examples or tests that
  opt into spend and run through `leaven seam serve --stdio`.
