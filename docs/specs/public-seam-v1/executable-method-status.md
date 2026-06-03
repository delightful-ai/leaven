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
| `leaven/stage.run` | Mock-configured and configured local | `leaven-seam-service::stage` | `MockRunner` returns deterministic runner text. `CommandRunner` dispatches a JSON-RPC `leaven/stage.run` request to a configured subprocess worker and services nested worker callbacks while the stage is active. Python SDK `lv.optimize(...).run()` uses this command-worker route for registered runner/proposer mechanics. |
| `leaven/lm.complete` | Mock-configured and live-provider configured | `leaven-seam-service::lm` plus provider crates | `Mock` uses `leaven-lm-mock` deterministic scripts. `OpenAi` uses `leaven-lm-openai` and requires the configured API-key environment variable. Missing credentials are an execution failure, not a mock success. |
| `leaven/workspace.materialize` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Allocates a local workspace and writes configured seed files. Workspace handles are local to the executing Plan document/callback flow. |
| `leaven/workspace.release` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Releases a workspace handle materialized earlier in the same Plan document/callback flow and returns a released workspace handle with receipts. |
| `leaven/workspace.snapshot`, `leaven/workspace.list`, `leaven/workspace.read_file`, `leaven/workspace.stat`, `leaven/workspace.digest`, `leaven/workspace.capture_artifacts` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Executes finite reads against the local workspace view with capability-scoped read ops. `capture_artifacts` currently returns listing entries and byte counts; blob byte retrieval still needs a Rust-owned artifact/blob read path before the workspace closeout row is done. |
| `leaven/workspace.git_log`, `leaven/workspace.git_diff`, `leaven/workspace.git_status` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Executes bounded Git commands inside an initialized local workspace and returns source-ref-bound `workspace_diff` values. The CLI proof uses a seed commit plus tracked post-commit edit so log, diff, and porcelain status all carry real Git output. |
| `leaven/graph.query`, `leaven/case.load`, `leaven/case.input`, `leaven/case.target`, `leaven/case.metadata` | Configured local | `leaven-seam-service::service` | Reads configured graph items and case records through Plan IR `graph_query` / `case_query.load`, enforces case-read capability scope before host reads, returns method-specific typed `graph_set` / `case_record` primaries, and is covered through `leaven seam serve --stdio`. This is configured service read state, not yet Rust `RunGraph` checkpoint readback. |
| `leaven/agent.run` | Live-provider configured when paired with materialized workspace | `leaven-seam-service::service` plus `leaven-agent-codex-cli` | Uses configured Codex CLI runtime against a materialized local workspace and projects transcript/command output blob refs into the public seam result. Without `SeamAgentConfig::CodexCli`, the service returns an explicit unsupported-provider error. |
| `leaven/sandbox.exec` | Configured local | `leaven-seam-service::service` plus `leaven-workspace-local` | Executes the lowered workspace command inside a materialized local workspace, captures stdout/stderr and declared output files, and returns byte-bound blob refs plus a sandbox call receipt through `leaven seam serve --stdio`. |
| `leaven/proposal.submit_batch` | Configured local write receipt | `leaven-seam-service::service` | Produces a validated proposal-batch receipt for submitted proposal payloads. This proves public-seam write plumbing, not Rust optimizer admission or proposal application. |
| `leaven/proposal.apply` | Configured local write receipt | `leaven-seam-service::service` | Executes Plan IR `apply_proposal_batch` through the configured service and returns a receipt-bound `apply_receipt` with created-candidate ids. This proves public-seam apply receipt plumbing, not Rust optimizer frontier admission. |
| `leaven/assessment.submit` | Configured local write receipt plus RunContext callback proof | `leaven-seam-service::service`; `leaven-acp-stage-bridge::graph_host` | `leaven seam serve --stdio` executes Plan IR `submit_assessments` through the configured service, enforces `assessment.submit` capability scope, emits a receipt with the assessment-scope request hash, and returns a validated `assessment_batch_receipt`. The bidirectional stage bridge additionally proves a worker-initiated `leaven/assessment.submit` callback is lowered by a host-owned typed assessment parser into `RunContext::submit_assessments`, stores typed evidence, emits evaluation completion, returns a receipt-bound extension result, and persists the assessment in `RunGraph`. |
| `leaven/evaluation.request` | Configured local write receipt | `leaven-seam-service::service` | Executes Plan IR `request_evaluation` through the configured service, enforces `evaluation.request` capability scope, constructs and validates an `EvaluationJobDocument`, validates the context-bound request-evaluation receipt, and returns an ACP-facing `evaluation_request_receipt` through `leaven seam serve --stdio`. This proves public-seam evaluation request plumbing, not durable RunContext evaluation-job persistence. |
| `leaven/event.emit` | Configured local write receipt plus RunContext callback proof | `leaven-seam-service::service`; `leaven-acp-stage-bridge::graph_host` | `leaven seam serve --stdio` emits a typed local run-event receipt through Plan execution and returns a receipt-bound `emit_run_event` value. The bidirectional stage bridge additionally proves a worker-initiated `leaven/event.emit` callback is lowered into `RunContext::emit(RunEvent::ExternalEventEmitted { ... })` and persists in `RunGraph` event history. |

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
