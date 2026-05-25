# Vendored Repositories

These repositories are vendored as read-only reference material for agents
working on the Leaven Python scaffold. They are **not** runtime dependencies
of the `leaven` package; the scaffold's `pyproject.toml` pins the
actually-used libs through normal `uv`/`pip`.

The vendoring discipline comes from
`/Users/darin/.claude/skills/vendor-key-dependency/SKILL.md`.

## Rules

- Treat each `repos/<slug>/` directory as read-only. Do not edit vendored
  source unless explicitly asked.
- Do not import from `repos/*` in `leaven` source or examples. The
  scaffold's runtime dependency on these libs (where any) is via `uv`.
- Do not include `repos/**` in formatters, linters, or codegen passes.
  `pyproject.toml` already exempts the directory in
  `[tool.ruff.lint.extend-per-file-ignores]`.
- Prefer examples, tests, and implementation patterns from vendored
  source over guesses when implementing or refining the scaffold's shape.
- Each vendored repo may carry its own `AGENTS.md`, `CLAUDE.md`, `LLMS.md`,
  or developer docs. Read those before writing code against that
  dependency's idioms.

## Inventory

12 subtree vendors + 1 informal copy. Total ~289 MB.

| repo | path | size | upstream | added |
|------|------|------|----------|-------|
| DSPy | `repos/dspy/` | 23 MB | stanfordnlp/dspy@main | 2026-05-24 |
| Inspect AI | `repos/inspect_ai/` | 38 MB | UKGovernmentBEIS/inspect_ai@main | 2026-05-24 |
| MCP Python SDK | `repos/mcp-python-sdk/` | 4 MB | modelcontextprotocol/python-sdk@main | 2026-05-24 |
| Verifiers | `repos/verifiers/` | 20 MB | PrimeIntellect-ai/verifiers@main | 2026-05-24 |
| Harbor | `repos/harbor/` | 43 MB | harbor-framework/harbor@main | 2026-05-24 |
| Temporal Python SDK | `repos/temporal-python-sdk/` | 12 MB | temporalio/sdk-python@main | 2026-05-24 |
| Anthropic SDK | `repos/anthropic-sdk-python/` | 8 MB | anthropics/anthropic-sdk-python@main | 2026-05-24 |
| Jupyter Client | `repos/jupyter-client/` | 1.4 MB | jupyter/jupyter_client@main | 2026-05-24 |
| python-lsp-jsonrpc | `repos/python-lsp-jsonrpc/` | 120 KB | python-lsp/python-lsp-jsonrpc@develop | 2026-05-24 |
| pydantic-ai | `repos/pydantic-ai/` | 13 MB (tests/ pruned) | pydantic/pydantic-ai@main | 2026-05-24 |
| Marvin | `repos/marvin/` | 5 MB | PrefectHQ/marvin@main | 2026-05-24 |
| BAML | `repos/baml/` | 117 MB | BoundaryML/baml@canary | 2026-05-24 |
| Weave (informal) | `repos/weave/weave/` | 6 MB | wandb/weave@main (sparse-clone copy) | 2026-05-24 |

---

## Phase 1 — Foundational (cited in specs)

### DSPy

- Local path: `docs/specs/leaven_py/repos/dspy/`
- Upstream: `https://github.com/stanfordnlp/dspy`
- Ref vendored: `main` (2026-05-24)
- Added for: `dspy.BaseLM` interface, `dspy.configure(lm=...)`, decorator
  + module composition. Direct ancestor of `leaven.lm.LmBuilder` and
  the `lv.x.dspy.LeavenDSPyLM` adapter. **Cited in specs:**
  `leaven_python.md:105-112`, `gepa_reference_behavior.md`,
  `evaluator_dspy_codex.v0.3.py`.
- Read first in: `dspy/clients/base_lm.py`, `dspy/clients/lm.py`,
  `dspy/programs/`, `dspy/examples/`.
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/dspy https://github.com/stanfordnlp/dspy.git main --squash`

### Inspect AI

- Local path: `docs/specs/leaven_py/repos/inspect_ai/`
- Upstream: `https://github.com/UKGovernmentBEIS/inspect_ai`
- Ref vendored: `main` (2026-05-24)
- Added for: `@solver`, `@scorer`, `@task` + `TaskState` context
  injection. Near 1:1 ancestor of our stage decorators + context types.
- Read first in: `src/inspect_ai/solver/`, `src/inspect_ai/scorer/`,
  `src/inspect_ai/_eval/`, `src/inspect_ai/task/`.
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/inspect_ai https://github.com/UKGovernmentBEIS/inspect_ai.git main --squash`

### MCP Python SDK

- Local path: `docs/specs/leaven_py/repos/mcp-python-sdk/`
- Upstream: `https://github.com/modelcontextprotocol/python-sdk`
- Ref vendored: `main` (2026-05-24)
- Added for: stdio JSON-RPC + FastMCP idioms. **Cited in specs:**
  `COMPREHENSIVE_DESIGN_PASS_NOTES.md:41-42,123-131`. Read for IDIOMS
  and KNOWN FAILURE MODES (especially issue #2433 about Windows CRLF
  stdio corruption); Leaven owns its own ACP schema (not MCP-compatible).
- Read first in: `src/mcp/server/session.py`, `src/mcp/server/fastmcp.py`,
  `src/mcp/shared/json_rpc.py`, `src/mcp/client/`.
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/mcp-python-sdk https://github.com/modelcontextprotocol/python-sdk.git main --squash`

---

## Phase 2 — Eval frameworks (dataset + task semantics)

### Verifiers (Prime Intellect)

- Local path: `docs/specs/leaven_py/repos/verifiers/`
- Upstream: `https://github.com/PrimeIntellect-ai/verifiers`
- Ref vendored: `main` @ `4d7dbb893ce3adb5ad55abd65e7b102f4eab4cb7` (2026-05-24)
- Added for: environment modules with `load_environment()`, Taskset/Harness
  v1 API, `@vf.reward` rubrics, HuggingFace `Dataset` rows, and Prime CLI eval
  orchestration. Closest analog to Leaven's case + stage + scorer composition
  for RL/eval environments.
- Read first in: `docs/overview.md`, `verifiers/v1/taskset.py`,
  `verifiers/v1/env.py`, `verifiers/v1/harness.py`, `verifiers/rubrics/rubric.py`,
  `verifiers/decorators.py` (`@reward`, `@stop`, `@setup`), `verifiers/v1/packages/tasksets/harbor.py`
  (Harbor bridge).
- Pattern notes: `docs/agent-context/patterns/verifiers-patterns.md`
- Add: `git subtree add --prefix=docs/specs/leaven_py/repos/verifiers https://github.com/PrimeIntellect-ai/verifiers.git main --squash`
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/verifiers https://github.com/PrimeIntellect-ai/verifiers.git main --squash`

### Harbor (agent eval harness)

- Local path: `docs/specs/leaven_py/repos/harbor/`
- Upstream: `https://github.com/harbor-framework/harbor`
- Ref vendored: `main` @ `6a7b64fd82610e9e2cecaeea3212f14b5f5066d6` (2026-05-24)
- Added for: containerized agent benchmarks (Terminal-Bench official harness),
  task directory layout (`instruction.md`, `task.toml`, `tests/`, `environment/`),
  `BaseAgent`/`BaseVerifier` trial loop, dataset registry (`dataset.toml`),
  and `harbor run` job orchestration. **Not** the separate
  `meridianlabs-ai/inspect_harbor` Inspect AI adapter package.
- Read first in: `README.md`, `AGENTS.md`, `src/harbor/models/task/task.py`,
  `src/harbor/models/dataset/manifest.py`, `src/harbor/agents/base.py`,
  `src/harbor/verifier/verifier.py`, `src/harbor/trial/trial.py`,
  `examples/tasks/hello-world/`.
- Pattern notes: `docs/agent-context/patterns/harbor-patterns.md`
- Add: `git subtree add --prefix=docs/specs/leaven_py/repos/harbor https://github.com/harbor-framework/harbor.git main --squash`
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/harbor https://github.com/harbor-framework/harbor.git main --squash`

---

## Round 4 — High-taste references (discovery agent's recommendations)

### BAML

- Local path: `docs/specs/leaven_py/repos/baml/`
- Upstream: `https://github.com/BoundaryML/baml`
- Ref vendored: `canary` (2026-05-24)
- Added for: **the closest architectural peer** — Rust core + per-language
  typed SDKs + schema-codegen. Most directly relevant to how Leaven
  separates `leaven-engine` (Rust) from `leaven` (Python) from
  `leaven-types` (codegen'd from JSON Schema).
- Read first in: `engine/language_client_python/` (the Python SDK),
  `engine/baml-runtime/` (Rust runtime patterns),
  `baml_language/sdks/python/` (DSL-to-Python codegen),
  `integ-tests/python/` and `integ-tests/python-v1/` (multi-version test patterns).
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/baml https://github.com/BoundaryML/baml.git canary --squash`

### pydantic-ai

- Local path: `docs/specs/leaven_py/repos/pydantic-ai/`
- Upstream: `https://github.com/pydantic/pydantic-ai`
- Ref vendored: `main` (2026-05-24); `tests/` (~221 MB of VCR cassettes)
  pruned locally for disk hygiene.
- Added for: literal `RunContext` name match, multi-provider lowering,
  durable-execution hooks. The closest framework analog to our
  `RunContext`/`StageContext`/`EvalContext` hierarchy.
- Read first in: `pydantic_ai_slim/pydantic_ai/` (core agent + context),
  `pydantic_ai_slim/pydantic_ai/models/` (provider lowering),
  `pydantic_graph/` (durable graph execution),
  `pydantic_evals/` (eval surface),
  `examples/`.
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/pydantic-ai https://github.com/pydantic/pydantic-ai.git main --squash`
  (will re-add tests/ — re-prune after pull)

### Temporal Python SDK

- Local path: `docs/specs/leaven_py/repos/temporal-python-sdk/`
- Upstream: `https://github.com/temporalio/sdk-python`
- Ref vendored: `main` (2026-05-24)
- Added for: **the canonical "Python decorators backed by Rust core with
  replay determinism"** pattern. Temporal's worker/workflow/activity
  decorator model + replay/checkpoint semantics inform how Leaven's
  stage decorators + replayability + receipts compose.
- Read first in: `temporalio/worker/`, `temporalio/workflow.py`,
  `temporalio/activity.py`, `temporalio/client.py` (Rust core via PyO3),
  `tests/worker/`.
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/temporal-python-sdk https://github.com/temporalio/sdk-python.git main --squash`

### Marvin

- Local path: `docs/specs/leaven_py/repos/marvin/`
- Upstream: `https://github.com/PrefectHQ/marvin`
- Ref vendored: `main` (2026-05-24)
- Added for: high-ergonomic surface on top of pydantic-ai. Shows what
  "even higher-level than the typed core" looks like — the next layer
  up from a Leaven user's perspective.
- Read first in: `src/marvin/` (the surface), `src/marvin/tools/`,
  `src/marvin/agents/`, `examples/`.
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/marvin https://github.com/PrefectHQ/marvin.git main --squash`

### Weave (informal sparse-clone copy)

- Local path: `docs/specs/leaven_py/repos/weave/weave/`
- Upstream: `https://github.com/wandb/weave`
- Ref vendored: `main` (2026-05-24) — **NOT a git subtree.** Full upstream
  is ~1.6 GB (notebooks, docs site, JS UI dominate); only the Python
  source under `weave/` was copied via sparse-clone.
- Added for: `@weave.op()` decorator — the closest UX target for Leaven's
  `@lv.scorer` / `@lv.runner`.
- Read first in: `weave/trace/op.py` (the decorator),
  `weave/scorers/` (scorer composition).
- Update: see `repos/weave/README-leaven.md` for the sparse-clone +
  copy procedure (no `git subtree pull` works).

### Anthropic SDK Python

- Local path: `docs/specs/leaven_py/repos/anthropic-sdk-python/`
- Upstream: `https://github.com/anthropics/anthropic-sdk-python`
- Ref vendored: `main` (2026-05-24)
- Added for: how a major LLM provider models async/types/retries/errors
  in their Python SDK. Reference for `leaven.lm.anthropic` shape +
  `LeavenDSPyLM` request/response idioms.
- Read first in: `src/anthropic/` (top-level), `src/anthropic/_client.py`,
  `src/anthropic/types/`, `src/anthropic/lib/streaming/`.
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/anthropic-sdk-python https://github.com/anthropics/anthropic-sdk-python.git main --squash`

### Jupyter Client

- Local path: `docs/specs/leaven_py/repos/jupyter-client/`
- Upstream: `https://github.com/jupyter/jupyter_client`
- Ref vendored: `main` (2026-05-24)
- Added for: ZMQ/stdio-style RPC client patterns from a battle-tested
  Python ecosystem. Reference for `lv.serve_stage` and `leaven-acp`
  worker/client lifecycle.
- Read first in: `jupyter_client/client.py`, `jupyter_client/manager.py`,
  `jupyter_client/session.py`.
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/jupyter-client https://github.com/jupyter/jupyter_client.git main --squash`

### python-lsp-jsonrpc

- Local path: `docs/specs/leaven_py/repos/python-lsp-jsonrpc/`
- Upstream: `https://github.com/python-lsp/python-lsp-jsonrpc`
- Ref vendored: `develop` (2026-05-24)
- Added for: minimal Python JSON-RPC implementation. 120 KB of pure
  protocol code, perfect reference for the smallest possible
  stdio-JSON-RPC shape — useful when implementing the Python side of
  `lv.serve_stage` and the `leaven-acp` client.
- Read first in: `pylsp_jsonrpc/endpoint.py`,
  `pylsp_jsonrpc/streams.py`, `pylsp_jsonrpc/dispatchers.py`.
- Update: `git subtree pull --prefix=docs/specs/leaven_py/repos/python-lsp-jsonrpc https://github.com/python-lsp/python-lsp-jsonrpc.git develop --squash`

---

## Skipped (deliberately not vendored)

- **LangGraph / OpenAI Evals / OSS Vizier / CrewAI** — added in an
  earlier vendor round, then removed. The audit at
  `docs/working-memory/leaven-py-research/2026-05-24-python-libs-actually-referenced.md`
  confirmed zero spec citations for any of them — they were
  research-agent recommendations from external knowledge, not Leaven
  design references.
- **Ray Tune** — 2+ GB monorepo; `Trainable` API is simpler than ours.
- **Pydantic** — already a runtime dependency (`pyproject.toml` pins
  it). Read the live installed source under `.venv/`.
- **Optuna** — distributed optimization patterns less load-bearing for
  our scaffold than other options.
- **Weave full repo** — 1.6 GB unconscionable; informal Python-source
  copy used instead (see above).

## Updating

After any `git subtree pull`, update the "Ref vendored" date for the
affected entry. If pydantic-ai is updated, re-prune `tests/`. If weave
is updated, follow the procedure in `repos/weave/README-leaven.md`.

For the per-repo "what to steal / avoid / surprising" pattern notes,
see `docs/agent-context/patterns/<slug>-patterns.md`. Phase 1 and Phase 2
(eval frameworks) have pattern files; Round 4 pattern notes are still pending
for BAML, pydantic-ai, Temporal, Marvin, and the informal Weave copy.
