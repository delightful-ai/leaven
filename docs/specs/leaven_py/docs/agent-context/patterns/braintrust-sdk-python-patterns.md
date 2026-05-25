# Braintrust Python SDK Patterns — Tracing + Evals Reference

**Date:** 2026-05-24
**Vendored at:** `repos/braintrust-sdk-python/` (`braintrustdata/braintrust-sdk-python@main`)
**Scope:** Python tracing, eval authoring, scorer wiring, framework instrumentation, and
OpenTelemetry bridging patterns worth reading when refining Leaven's Python run/report
and stage telemetry surface.

Braintrust is a hosted/local SDK for evaluating and shipping AI products. In this
repo it is read-only reference material, not a Leaven runtime dependency.

## What to read first

| File | Why |
|------|-----|
| `repos/braintrust-sdk-python/README.md` | Repository package map and integration inventory. |
| `repos/braintrust-sdk-python/py/README.md` | Core Python SDK usage, eval entry points, logging, and tracing examples. |
| `repos/braintrust-sdk-python/py/src/braintrust/framework.py` | `Eval(...)` orchestration and scorer/task/data shape. |
| `repos/braintrust-sdk-python/py/src/braintrust/logger.py` | Experiment/logger object model and row logging behavior. |
| `repos/braintrust-sdk-python/py/src/braintrust/trace.py` | Span creation, parent/child context, and trace payload shaping. |
| `repos/braintrust-sdk-python/py/src/braintrust/otel/` | OpenTelemetry bridge patterns. |
| `repos/braintrust-sdk-python/py/src/braintrust/integrations/` | Auto-instrumentation for agent/model frameworks. |
| `repos/braintrust-sdk-python/py/src/braintrust/wrappers/pytest_plugin/` | Test-runner integration pattern. |

## Patterns to compare against Leaven

### Eval shape

Braintrust's core authoring shape is `Eval(project, data=..., task=..., scores=...)`.
This is useful pressure on Leaven's `lv.optimize(..., train=..., runner=..., scorer=...)`
surface because it keeps dataset, execution function, and scoring functions visibly
separate.

Do not copy the exact shape into Leaven core: Braintrust evaluates an application task,
while Leaven composes optimizer inputs, stage handlers, runtime capabilities, and durable
engine receipts.

### Trace-first instrumentation

Braintrust has rich span/logging APIs and integrations that wrap existing model or agent
frameworks. Read these when deciding how `@lv.runner`, `@lv.scorer`, `@lv.evaluator`,
and `cx.*` calls should emit trace events without forcing user code to manually log every
step.

The useful idea is "instrument boundaries users already touch"; the unsafe idea is
allowing trace logs to become proof. In Leaven, receipts and evidence envelopes remain the
audit currency.

### Framework integrations

The repo includes integrations for DSPy, Pydantic AI, LangChain, OpenAI Agents, Claude
Agent SDK, AutoGen, CrewAI, Strands, AgentScope, Temporal, and provider SDKs. These are a
good map of ecosystem naming and monkey-patch/patcher approaches, but Leaven adapters
should stay explicit under `lv.x.*` unless the spec deliberately accepts auto-patching.

### OpenTelemetry bridge

The OTel bridge is relevant to future trace export/import. If Leaven ingests or emits
standard traces, preserve Leaven-specific receipt identity, capability tokens, case IDs,
and stage IDs as structured attributes instead of flattening them into log messages.

## Anti-patterns

- Do not add `braintrust` or `autoevals` as runtime dependencies just because this repo is
  vendored.
- Do not import from `repos/braintrust-sdk-python/` in scaffold code or examples.
- Do not let hosted-eval semantics replace Leaven's local/durable run store, case
  visibility law, or receipt validation.
- Do not auto-patch user frameworks by default. Auto-instrumentation is ergonomic, but it
  can obscure where evidence and costs are actually produced.
