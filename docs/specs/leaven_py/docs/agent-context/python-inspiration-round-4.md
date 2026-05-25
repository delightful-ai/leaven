# Python Inspiration Round 4 — Additional Vendoring Candidates

**Date**: 2026-05-24
**Purpose**: Second-pass sweep over Python libraries we MISSED in the initial
inventory at `python-inspiration-inventory.md`. Recommends additions worth
vendoring as read-only reference material for the Leaven Python scaffold.

This pass deliberately excludes the existing Phase 1/2/3 list (DSPy, Inspect
AI, MCP SDK, LangGraph, OpenAI Evals, OSS Vizier, CrewAI, Modal), the explicit
skip list (Ray Tune, Pydantic, Optuna), and anything that duplicates a
property we already cover.

Sizes below are `gh repo view ... --json diskUsage` in KB (GitHub-reported
checked-out size), so the on-disk subtree after `--squash` is generally
similar. Numbers were sampled 2026-05-24.

---

## Top 5 highest-value additions

### 1. BoundaryML/baml

- **URL**: https://github.com/BoundaryML/baml
- **Default branch**: `canary`
- **Stars / size**: 8,290 / ~608 MB (full monorepo; subset vendoring required)
- **What we'd read**: BAML is the most direct living analogue to Leaven's
  promise — Rust core plus per-language generated SDKs (Python, TS, Ruby,
  Java, C#, Go) that all speak one schema-locked wire and project into native
  typed records. We'd read it for (a) `engine/baml-runtime` and
  `engine/baml-schema-ast` to see how schema-codegen feeds typed client
  surfaces in multiple languages without language-locking the wire, mapping
  to our `docs/specs/public-seam-v1/schemas/` + future `leaven-types`
  codegen story; (b) `engine/language_client_python/` for the per-language
  bridge that mirrors our Python SDK/Rust engine split; (c) BAML's
  test-fixture runner shape for ideas on how to test typed clients against
  a single locked schema corpus.
- **Recommendation**: **`subset:engine/language_client_python/,engine/baml-runtime/{src,baml-types},engine/baml-schema-ast/`**
  (skip TS/Ruby/Java/C#/Go clients, examples, integration tests, docs site)
- **Justification**: BAML is THE library whose architecture lines up with
  Leaven Python at the deepest level — Rust substrate, multi-language SDK,
  schema-codegen as the wire safety mechanism. The monorepo is too big to
  vendor whole, but a tight subset (~50-80 MB) gives us reference for every
  multi-language and codegen decision in front of us. Highest-leverage
  single addition.

### 2. pydantic/pydantic-ai

- **URL**: https://github.com/pydantic/pydantic-ai
- **Default branch**: `main`
- **Stars / size**: 17,260 / ~188 MB
- **What we'd read**: `pydantic-ai` is the closest peer to Leaven's
  "typed pydantic v2 records + async context + decorators + multi-provider
  LM" combination in the entire ecosystem. We'd read (a) `pydantic_ai/agent.py`
  and `pydantic_ai/_run_context.py` for `RunContext`-style context objects
  (literally the same name as ours), mapping directly to our
  `RunContext`/`StageContext`/`EvalContext` injection model; (b)
  `pydantic_ai/models/` for the provider-adapter shape across 15+ LM
  providers, comparable to `lv.lm.anthropic(...)` / `lv.lm.openai(...)`
  builders; (c) `pydantic_graph/` for typed-graph composition that parallels
  how we want stage composition to read; (d) the durable-execution
  integration with Temporal + Prefect for ideas on engine-side durability
  hooks that survive Python crashes.
- **Recommendation**: **`subset:pydantic_ai/,pydantic_graph/,docs/api/`**
  (skip `tests/`, `examples/`, `docs/blog/`, JS bits)
- **Justification**: This is the most directly comparable upstream design.
  Even the type names overlap (`RunContext`). Vendoring the core packages
  (~50 MB without tests/examples) gives a constant reference for typed
  context injection, multi-provider lowering, and structured-output validation
  done in pydantic v2 style — exactly the shape `leaven` Python is aiming for.

### 3. temporalio/sdk-python

- **URL**: https://github.com/temporalio/sdk-python
- **Default branch**: `main`
- **Stars / size**: 1,074 / ~17 MB
- **What we'd read**: Temporal Python is the canonical reference for
  durable Python decorators backed by a non-Python engine — exactly Leaven's
  shape (Python authoring surface, Rust engine, durable state, replay
  semantics). We'd read (a) `temporalio/workflow.py` and
  `temporalio/activity.py` for `@workflow.defn` / `@activity.defn` decorator
  registration patterns that we should learn from for `@lv.evaluator` /
  `@lv.runner` / `@lv.serve_stage`; (b) `temporalio/worker/` for the
  worker-process model that maps onto Leaven's "engine spawns Python stage
  workers over ACP stdio" architecture; (c) `temporalio/bridge/` for how
  they handle the Python<->Rust core boundary (they use a Rust core SDK
  under the hood — same architectural choice as Leaven); (d) replay /
  determinism enforcement, directly relevant to per-assessment
  replayability.
- **Recommendation**: **`full`** — repo is small (~17 MB), focused, and
  every package is on the load-bearing path for us.
- **Justification**: Temporal Python is the closest existing peer to
  Leaven's "Python decorator surface backed by Rust core with durable
  semantics" design. The whole SDK is small enough to vendor whole and the
  patterns are direct precedent — particularly worker registration, replay
  determinism, and the Python/Rust bridge boundary.

### 4. PrefectHQ/marvin

- **URL**: https://github.com/PrefectHQ/marvin
- **Default branch**: `main`
- **Stars / size**: 6,158 / ~67 MB
- **What we'd read**: Marvin 3.0 is "an ambient intelligence library" built
  on top of `pydantic-ai` by the Prefect team, which means it embodies both
  durable-workflow taste AND structured-output discipline. We'd read (a)
  `src/marvin/agents/`, `src/marvin/tasks/`, `src/marvin/teams/` for the
  agent-task-team decomposition that informs how our reflector / proposer /
  judge stages compose; (b) `src/marvin/handlers/` for streaming event
  patterns that map onto our progress-update wire; (c) `src/marvin/thread.py`
  for thread-state durability — closest analogue to our run-handle inspection
  surface; (d) the way Marvin layers a high-ergonomic API on top of a
  lower-level typed core (`pydantic-ai`), which is exactly the layering
  pressure on `leaven` over `leaven-acp`.
- **Recommendation**: **`full`** — 67 MB is reasonable; library is cohesive
  and we'd read most of it.
- **Justification**: Marvin shows the "high-ergonomic surface on top of
  typed-core" idiom done well in 2026. It teaches us how to write the
  user-facing API in front of a stricter wire and how durability/handles
  fit into a pythonic workflow style.

### 5. wandb/weave (subset)

- **URL**: https://github.com/wandb/weave
- **Default branch**: `master`
- **Stars / size**: 1,093 / ~1.6 GB (notebooks + docs site; subset only)
- **What we'd read**: Weave's `@weave.op()` decorator is the canonical
  "decorate any function and get a trace receipt for free" pattern. We'd
  read (a) `weave/trace/op.py` (the `@op` decorator) and
  `weave/trace/weave_client.py` for how arbitrary Python callables are
  wrapped into traceable units with deterministic input/output capture —
  directly relevant to how `@lv.scorer` / `@lv.runner` produce receipts;
  (b) `weave/scorers/` and `weave/flow/eval.py` for the eval framework
  decomposition (model + dataset + scorer + judge), which is a close peer
  to `lv.optimize(...).runner().scorer().run()`; (c) `weave/trace_server/`
  for the trace-server wire protocol if/when we need to compare receipt
  storage models.
- **Recommendation**: **`subset:weave/trace/,weave/trace_server/,weave/scorers/,weave/flow/`**
  (aggressively skip docs, notebooks, integrations, JS UI — this is 95% of
  the 1.6 GB)
- **Justification**: Weave's `@op` decorator pattern is exactly what we
  want `@lv.scorer` / `@lv.runner` / `@lv.judge` to feel like from the
  user's perspective; their trace/receipt model is a working production
  reference for the audit-currency-as-decoration mistake we want to avoid.
  Must be subset — full vendoring is 1.6 GB.

**Estimated total disk if all 5 vendored**: roughly 50 (BAML subset) + 50
(pydantic-ai subset) + 17 (Temporal full) + 67 (Marvin full) + 80 (Weave
subset) = **~260 MB**. Comparable to the existing Phase 1+2+3 vendoring
budget (~230 MB), so this roughly doubles the reference corpus.

---

## Worth-vendoring (additional candidates, brief)

- **anthropics/anthropic-sdk-python** (~5 MB) — `subset:src/anthropic/_streaming.py,src/anthropic/_models.py,src/anthropic/types/`. Reference for typed pydantic v2 SDK surface, streaming, retry/error patterns. Tiny; obvious win for `cx.lm.complete` design.
- **openai/openai-python** (~9 MB) — `subset:src/openai/{_streaming.py,_models.py,_types/,types/}`. Same rationale as anthropic-sdk-python; cross-check two leading typed LM SDKs to triangulate the right shape for `lv.lm.*`.
- **jupyter/jupyter_client** (~3 MB) — `full`. Reference stdio-JSON-protocol implementation with kernel-spawn lifecycle, message correlation, and stdin/stdout/iopub channel separation. Direct precedent for `leaven-acp` transport beyond MCP SDK; teaches multi-channel framing we deliberately omit.
- **python-lsp/python-lsp-jsonrpc** (~0.06 MB) — `full`. Minimal JSON-RPC 2.0 stream reader/writer pair; 64 KB of code that shows the absolute minimum-viable framing layer. Worth vendoring for "what is the smallest correct thing" reference when sizing `leaven-acp` Python client.
- **BerriAI/litellm** (~1 GB) — `subset:litellm/llms/,litellm/router.py,litellm/cost_calculator.py,litellm/types/`. The widest-spectrum provider router in the ecosystem; vendor the provider-adapter layer and cost-tracking only. Reference for how to keep 100+ providers behind one typed surface (which we explicitly don't want to be, but need to understand for `lv.lm.*` provider registry).
- **getsentry/sentry-python** (~87 MB) — `subset:sentry_sdk/integrations/,sentry_sdk/transport.py,sentry_sdk/client.py`. Reference multi-language SDK that solved retries / batching / transport / async safely across many integrations. We mirror this for `leaven-acp` client behavior.
- **HypothesisWorks/hypothesis** (~43 MB) — `subset:hypothesis-python/src/hypothesis/strategies/,hypothesis-python/src/hypothesis/core.py,hypothesis-python/src/hypothesis/extra/pytest_plugin.py`. Reference for `@given` style stage discovery, plugin registration, and how to make decorated functions pytest-discoverable — directly relevant to how `@lv.evaluator` should be discoverable by external tooling.
- **PrefectHQ/prefect** (~221 MB) — `subset:src/prefect/{tasks.py,flows.py,states.py,context.py,transactions.py},src/prefect/results.py`. Reference for `@task` / `@flow` decorators + state machine + transactional task semantics. Maps onto `@lv.runner` / `@lv.evaluator` + assessment-write transactions.
- **pydantic/pydantic-ai → also see `pydantic_evals/`** — already covered in Top 5, but flag the separate `pydantic_evals` subpackage explicitly; it's a clean small reference for "decorators + datasets + evaluators" composition.
- **dottxt-ai/outlines** (~91 MB) — `subset:outlines/{generate,models,types}`. Reference for structured-generation grammars and how the "typed output" promise is enforced at the LM boundary. Relevant for `cx.lm.complete(output_schema=...)` future work.
- **567-labs/instructor** (~77 MB) — `subset:instructor/{client.py,patch.py,function_calls.py,validators.py}`. The most popular typed-LM-output library; reference for the `@instructor.patch`-style adapter pattern that lets users layer typed outputs over a vanilla provider client — directly relevant to whether `cx.lm.complete` should accept a `response_model=...` argument.
- **langfuse/langfuse-python** (~8 MB) — `subset:langfuse/decorators/,langfuse/client.py`. Tiny SDK that shows the `@observe()` decorator pattern for ambient tracing without rewriting call sites — alternate reference to Weave's `@op` design.

---

## Considered + skipped

- **dagster-io/dagster** (~1.4 GB) — Too big; asset-materialization patterns are interesting but the gravitational pull of the whole orchestration framework (UI, run launcher, schedules, sensors, GraphQL) buries the small load-bearing kernel under 5x more material than we'd ever read. Web-only.
- **tiangolo/fastapi** (~48 MB) — Decorator + DI patterns are widely known and the Litestar peer we already considered earlier covers the same shape with a stronger msgspec story. Web-only; nothing new vs. what we already understand.
- **litestar-org/litestar** (~190 MB) — Similar reason. The Litestar msgspec story is interesting in isolation but Leaven is pydantic-v2-locked, so most of the codebase is not direct precedent. Skip unless we later pivot toward msgspec.
- **agno-agi/agno** (~286 MB) — Fast-moving multi-agent framework with high stars (40k) but spread across "build agents + manage agents + observe agents" tooling that's too broad for our scaffold's focused decorator surface. Worth a web read; not worth the disk.
- **huggingface/smolagents** (~7 MB) — Tiny and trendy, but the "code-as-agent-action" thesis is orthogonal to Leaven's typed-stage thesis. The vendoring overhead is small but the load-bearing patterns are few.
- **Mirascope/mirascope** (~132 MB) — `@llm.call` decorator and pydantic extraction are nice, but pydantic-ai covers the same ground with stricter typing and a better-matched architecture. Picking one is enough.
- **mickeyinfoshan/lsp-mcp** — Bridge project, not load-bearing reference. Skip.
- **python-lsp/python-lsp-server** (~1.5 MB) — The full LSP server is overkill; the JSON-RPC subset (`python-lsp-jsonrpc`) is the relevant 0.06 MB subset and is already in the vendor list above.
- **Various langfuse/langsmith/helicone/comet-ml/mlflow** — Tracing/observability SDKs are interesting in aggregate but Weave + langfuse-python already cover the decorator-instrumentation pattern. Diminishing returns.

---

## What we'd still be missing

After this pass, the scaffold still has under-covered design surfaces:

1. **Multi-tenant capability/token enforcement in a Python SDK.** No
   surveyed library does what `trust_profile="managed_sandbox"` +
   `input_classes=[...]` + capability-token-bearer-env-var implies. The
   nearest neighbor is sentry-python's DSN/transport split, but it doesn't
   model capability minting or data-class propagation. The closest
   conceptual peer is probably the Google Cloud Python client's IAM helpers
   (also too big and too cloud-specific to vendor). This is a Leaven-original
   surface; no upstream prior art to lean on.
2. **Per-assessment replay determinism in a high-level decorator surface.**
   Temporal Python has the strongest peer for replay determinism, but
   Temporal replay is workflow-event-replay; we want input+receipt+seam-validation
   replay. The two are related but not the same shape. Worth doing a custom
   design pass — no library will hand us the answer.
3. **Distributed evidence-envelope storage with public/private projection.**
   Weave's trace-server is the closest peer but treats traces as a flat
   stream, not as projection-class-aware envelopes. This is also Leaven-original.
4. **`cx.batch()` context-manager-as-transaction.** No surveyed library
   exposes "multiple typed effects, one wire round-trip, one receipt root"
   as a single Python context manager. Closest cousin is SQLAlchemy's
   session transaction, but that's row-level. We may have to invent this
   shape from first principles; the Python `contextlib`/async-context idioms
   are the only true precedent.
5. **External-language-worker process protocol that isn't MCP-style.**
   MCP SDK and jupyter_client are the two indirect references but neither
   does ACP capability-bearer-env-var auth, schema-locked payload kinds, or
   evaluator-task-shaped messages. The seam is ours to invent; vendored
   prior art only shows transport mechanics, not the semantic protocol.
6. **Pytest-plugin-style stage discovery.** Hypothesis covers some of this
   but a discovery story for "find all `@lv.evaluator` decorated functions
   in this repo and surface them to `leaven serve --stdio`" doesn't exist
   in any single library. We may end up cribbing pytest's own collection
   logic if entry-points + decorator-registration combined isn't enough.

Items 1, 3, 4, and 5 are Leaven-original surfaces by design (the entire
point of the locked seam is that nobody else has solved them). For items 2
and 6, the right play is probably a short focused design note rather than
a vendoring expedition.
