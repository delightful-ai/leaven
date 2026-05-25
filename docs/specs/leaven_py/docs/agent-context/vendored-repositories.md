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

## DSPy

- Local path: `docs/specs/leaven_py/repos/dspy/`
- Upstream: `https://github.com/stanfordnlp/dspy`
- Ref vendored: `main` (2026-05-24)
- Approximate size: ~23 MB
- Added for: `dspy.BaseLM` interface, `dspy.configure(lm=...)` pattern,
  decorator + module composition shapes. Direct ancestor of
  `leaven.lm.LmBuilder` and the `lv.x.dspy.LeavenDSPyLM` adapter.
- Runtime import/package name: `dspy` (optional dep; install with
  `uv add dspy-ai` or `pip install 'leaven[dspy]'`).
- Read first in: `dspy/clients/base_lm.py`, `dspy/clients/lm.py`,
  `dspy/programs/`, `dspy/examples/`.
- Add command:

  ```sh
  git subtree add --prefix=docs/specs/leaven_py/repos/dspy \
      https://github.com/stanfordnlp/dspy.git main --squash
  ```

- Update command:

  ```sh
  git subtree pull --prefix=docs/specs/leaven_py/repos/dspy \
      https://github.com/stanfordnlp/dspy.git main --squash
  ```

## Inspect AI

- Local path: `docs/specs/leaven_py/repos/inspect_ai/`
- Upstream: `https://github.com/UKGovernmentBEIS/inspect_ai`
- Ref vendored: `main` (2026-05-24)
- Approximate size: ~38 MB
- Added for: `@solver`, `@scorer`, `@task` decorator patterns and
  `TaskState` + context-injection model. Near 1:1 architectural ancestor
  of Leaven's stage decorators (`@lv.evaluator`, `@lv.scorer`,
  `@lv.reflector`) and the `RunContext`/`StageContext`/`EvalContext`
  hierarchy.
- Runtime import/package name: `inspect_ai` (not a runtime dep of
  leaven; reference only).
- Read first in: `src/inspect_ai/solver/`, `src/inspect_ai/scorer/`,
  `src/inspect_ai/_eval/` (async state threading),
  `src/inspect_ai/task/`.
- Add command:

  ```sh
  git subtree add --prefix=docs/specs/leaven_py/repos/inspect_ai \
      https://github.com/UKGovernmentBEIS/inspect_ai.git main --squash
  ```

- Update command:

  ```sh
  git subtree pull --prefix=docs/specs/leaven_py/repos/inspect_ai \
      https://github.com/UKGovernmentBEIS/inspect_ai.git main --squash
  ```

## MCP Python SDK

- Local path: `docs/specs/leaven_py/repos/mcp-python-sdk/`
- Upstream: `https://github.com/modelcontextprotocol/python-sdk`
- Ref vendored: `main` (2026-05-24)
- Approximate size: ~4 MB
- Added for: stdio JSON-RPC wire transport patterns, FastMCP decorator
  framework, session lifecycle. Reference for `leaven-acp` transport
  shape and for the `lv.serve_stage(...)` standalone-worker pattern.
  Note: Leaven owns its own ACP schema (not MCP-compatible) — vendor
  this for IDIOMS and KNOWN FAILURE MODES (especially issue
  [#2433](https://github.com/modelcontextprotocol/python-sdk/issues/2433)
  about Windows CRLF stdio corruption), not for protocol compatibility.
- Runtime import/package name: `mcp` (not a runtime dep of leaven;
  reference only).
- Read first in: `src/mcp/server/session.py`,
  `src/mcp/server/fastmcp.py`, `src/mcp/shared/json_rpc.py`,
  `src/mcp/client/`.
- Add command:

  ```sh
  git subtree add --prefix=docs/specs/leaven_py/repos/mcp-python-sdk \
      https://github.com/modelcontextprotocol/python-sdk.git main --squash
  ```

- Update command:

  ```sh
  git subtree pull --prefix=docs/specs/leaven_py/repos/mcp-python-sdk \
      https://github.com/modelcontextprotocol/python-sdk.git main --squash
  ```

## Skipped (deliberately not vendored)

- **Ray Tune** — 2+ GB monorepo; `Trainable` API is simpler than Leaven's
  stage model. Web-only reference suffices.
- **Pydantic** — already a runtime dependency (`pyproject.toml` pins it).
  Read the live installed source under `.venv/`, not a vendored copy.
- **Optuna** — ~200 MB; OSS Vizier covers similar distributed
  optimization patterns at smaller scale and is queued for Phase 2.

## Queued for Phase 2 (add when scaffold ergonomics stabilize)

- LangGraph — `StateGraph.add_node`, `RunnableConfig` injection
- OpenAI Evals — registry pattern + YAML/Python split
- OSS Vizier — gRPC client/server algorithm/host split

## Queued for Phase 3 (add when agentic reflection work begins)

- CrewAI — role-based agent composition + decorator stacking
- Modal — Python decorator deployment ergonomics

## Updating

After `git subtree pull`, update the "Ref vendored" date for the affected
entry in this file. If the upstream changed materially (e.g. major API
break), update the per-repo "Read first in" hints + the corresponding
`patterns/<slug>-patterns.md` file under this same directory.
