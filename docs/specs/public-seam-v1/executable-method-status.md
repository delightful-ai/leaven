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
| `leaven/agent.run` | Live-provider configured when paired with materialized workspace | `leaven-seam-service::service` plus `leaven-agent-codex-cli` | Uses configured Codex CLI runtime against a materialized local workspace and projects transcript/command output blob refs into the public seam result. Without `SeamAgentConfig::CodexCli`, the service returns an explicit unsupported-provider error. |
| `leaven/proposal.submit_batch` | Configured local write receipt | `leaven-seam-service::service` | Produces a validated proposal-batch receipt for submitted proposal payloads. This proves public-seam write plumbing, not Rust optimizer admission or proposal application. |
| `leaven/event.emit` | Configured local write receipt | `leaven-seam-service::service` | Emits a typed local run-event receipt through Plan execution and returns a receipt-bound `emit_run_event` value. This is configured local receipt behavior, not yet durable RunGraph event persistence. |

## Validated But Not Executed By The Current Service

The runtime dispatcher exposes every locked worker-profile method and validates
request/response envelopes before and after service calls. The configured
service does not yet provide runtime behavior for these V1 families:

- graph and case reads: `leaven/graph.query`, `leaven/case.load`,
  `leaven/case.input`, `leaven/case.target`, `leaven/case.metadata`
- Git-backed workspace queries: `leaven/workspace.git_log`,
  `leaven/workspace.git_diff`, `leaven/workspace.git_status`
- remaining effects and graph writes: `leaven/sandbox.exec`,
  `leaven/proposal.apply`, `leaven/assessment.submit`,
  `leaven/evaluation.request`
- watch behavior remains deferred to a future V1.x slice

Some of these families have public-seam contract validators, Plan IR lowering
helpers, or representative harness evidence in `leaven-public-seam`; that is
not the same as configured service execution through `leaven seam serve
--stdio`.

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
