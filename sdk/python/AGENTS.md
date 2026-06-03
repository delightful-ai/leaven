# AGENTS — sdk/python

## Boundary

This directory is the real in-repo Python SDK project for Leaven. It is
importable, typed, IDE-navigable, and owns Python dependency declarations,
examples, tests, codegen tooling, and private process clients for the public
seam.

It is mostly scaffold: nearly every function and method body is
`raise NotImplementedError(...)` or `...`. Add real behavior only for focused
foundation slices that are backed by the governing spec and a current proof.

### The wired paths

#### 1. Example 03, durable seam optimize mechanics

`examples/03_prompt_optimize.py` now runs `lv.optimize(...).run()` through the
durable `leaven seam serve --stdio --config` server route. The wired surface is
the current mechanics path: `optimize().run()`, `cases.from_jsonl`, registered
`@lv.runner` dispatch through the checked-in command worker, `cx.lm.complete`
and `cx.agent.run` callbacks while a worker stage is active, Python
`@lv.reward` vector execution, persisted in-process inspection via
`lv.runs.open(...)`, and the `Environment`/`Task`/`Rollout.fn`/`Rubric`/
`runtime.local` records those compose. The private owners are
`leaven._seam_optimize`, `leaven._seam_worker`, and `leaven._runs`, not legacy
`leaven._serve`.

Honest scope: the Python SDK now configures and calls the durable public seam
server for runner `leaven/stage.run` mechanics, executes the user's Python
`@lv.runner`, services configured `leaven/*` callbacks, runs Python reward
vectors, optionally dispatches a configured `Propose.fn(...)` proposer that can
run `cx.agent.run` against `cx.parent_workspace` and submit a proposal batch
over `leaven/proposal.submit_batch`, returns a typed
`Optimized[PromptArtifact]`, and writes an `optimized.json` projection under
`.leaven/runs/<run_id>/`. It still does not apply submitted proposals, run real
GEPA proposal search, persist Rust graph checkpoints, provide durable blob
readback, or close live LM-provider acceptance by itself.

#### 2. Example 10, live Codex agent.run over the public seam

`examples/10_live_codex_seam.py` is a live-gated substrate proof for the new
public seam service path. With `LEAVEN_LIVE_CODEX=1`, it binds
`AgentBuilder.run` to the private `leaven._seam` client, spawns
`leaven seam serve --stdio --config`, lowers one locked `leaven/agent.run` Plan
IR request, and checks that the Rust child materializes a workspace, runs the
configured Codex CLI adapter, and returns an `AgentSession` with workspace and
agent receipts plus a transcript blob ref.

Honest scope: this uses `AgentBuilder.run` through the private `leaven._seam`
process-client package, not an engine-supplied `cx.agent` inside
`lv.optimize(...).run()`. It proves that Python can drive the real Leaven-owned
stdio seam and Codex provider substrate; persisted blob inspection and typed
proposal submission remain later slices and stay scaffold.

#### 3. Private effect builders over the public seam

`AgentBuilder.run` and `LmBuilder.complete` can be privately bound to
`leaven._seam` request clients for focused substrate proofs. Their public
dependency is the locked `leaven/agent.run` and `leaven/lm.complete` Plan IR
wire. Their private dependency is the `_SeamRequester` protocol and
`leaven._seam` request/config helpers.

Honest scope: these builders are real process-seam clients when bound by tests
or examples, but they are not yet engine-supplied `cx.agent` / `cx.lm` values
inside `lv.optimize(...).run()`. Do not claim this as full Python stage-context
execution until the engine creates those bound contexts during stage dispatch.

The binary is resolved via `LEAVEN_BIN`, else `target/{debug,release}/leaven`
under the repo root (`LEAVEN_REPO_ROOT` override). Build it with
`cargo build -p leaven-cli`. The repo-root walk uses the topology-contract marker
so the example runs regardless of cwd.

It is not yet a published `leaven` package. It is the real source project that
future packaging/publishing work should harden instead of recreating under
`docs/specs`.

## Public API discipline (load-bearing)

Two tiers, one rule. See `docs/specs/leaven_python.md` "Public API
discipline" for the governing spec section.

- **Public:** in module `__all__`, no leading underscore anywhere in the
  path, documented in the spec, frozen + `extra="forbid"` if a pydantic
  model.
- **Private:** leading underscore on file or symbol. Private packages may use
  `__all__` internally to declare their own map, but those names are not part
  of the public SDK surface and may break between versions.

The rule: **if it isn't in `__all__` and the name is unprefixed in a
public module, it does not exist.**

When editing the SDK:

- Every public module gets `__all__`. Look at the bottom of any module
  for the convention.
- Naming an internal helper or module? Underscore-prefix it.
- Adding a new public symbol? Add it to `__all__` AND surface it
  through `src/leaven/__init__.py` if part of the top-level `lv.*` API.
- Reaching into a private name from another module? Stop. Either it
  should be public (promote it) or use a public alternative.
- Submodules listed in `leaven/__init__.py`'s `__all__` (e.g.
  `optimizers`, `lm`, `agent`, `workspace`, `sandbox`, `cases`,
  `frontier`, `output`, `scoring`, `trust`, `runs`, `x`, `data_class`,
  `artifacts`, `layouts`, `setup`) are intentional namespaces. Submodules
  that leak into `dir(leaven)`
  only because their public types are imported from them (`leaven.case`,
  `leaven.assessment`, etc.) are NOT in `__all__`; users access the type
  as `lv.Case`, `lv.Assessment`.

ruff's `RUF022` (sorted `__all__`) is on. No current ruff rule for
"public module must declare `__all__`" — discipline is by review.

## Rules

- The governing spec is `docs/specs/leaven_python.md`. If this project and
  the spec disagree, the spec wins; this project is updated to match.
- Every module gets a docstring at the top explaining what it is and pointing
  to the relevant spec section.
- Public functions/classes get full type hints, signatures, and docstrings
  but `raise NotImplementedError(...)` bodies (or `...` for pure-type stubs).
- Pydantic v2 models for wire-shaped types. Dataclasses for internal config
  shapes. `from __future__ import annotations` at the top of every file.
- Top-level `lv.*` imports are the public surface; submodule paths
  (`lv.optimizers.gepa`, `lv.lm.anthropic`) are also part of the public
  surface where the spec names them.
- Internal modules (`leaven._serve`, `leaven._seam`, `leaven._types.*`) get
  leading underscore conventions or live under `_types/`.
- Do not introduce dependencies that aren't in `pyproject.toml`. Adding a
  dep is a taste call worth surfacing in a docstring.

## Editing this project

If you change a shape in this project, update the corresponding section of
`docs/specs/leaven_python.md` in the same change. The two stay in sync.

If you want to add a new public module to capture a surface the spec didn't
enumerate explicitly, surface the proposed shape and the spec section it
extends as a question before writing the module. The public SDK should not grow
beyond what the spec describes.

## Vendored Repositories

External reference repositories remain vendored under
`docs/specs/leaven_py/repos/` for agent reference only. They are read-only
inspiration; they are **not** runtime dependencies of `leaven`.

Discipline:

- Use vendored repos as read-only reference when refining the SDK's
  shape against real-world API ergonomics, idioms, tests, and known
  failure modes.
- Prefer examples and patterns from vendored source over guesses.
- Do not edit files under `docs/specs/leaven_py/repos/`.
- Do not import from those repos in `leaven` source or `examples/`.
- If a vendored repo carries `AGENTS.md`, `CLAUDE.md`, `LLMS.md`, or
  developer docs, read those first when writing code against that
  library's idioms.

The full vendored inventory plus add/update commands and "what to read
first" hints live at
[`docs/specs/leaven_py/docs/agent-context/vendored-repositories.md`](../../docs/specs/leaven_py/docs/agent-context/vendored-repositories.md).

Per-dependency pattern notes (what we'd steal, what we'd avoid, what's
surprising) live at
[`docs/specs/leaven_py/docs/agent-context/patterns/`](../../docs/specs/leaven_py/docs/agent-context/patterns/).

Phase 1 vendored (2026-05-24) — direct spec citations:

Paths in the list below are relative to `docs/specs/leaven_py/`.

- `repos/dspy/` — `dspy.BaseLM` + decorator patterns. Read when working
  on `leaven.lm` or `lv.x.dspy.LeavenDSPyLM`.
- `repos/inspect_ai/` — `@solver`/`@scorer`/`@task` decorators + context
  injection. Read when working on stage decorators or context objects.
- `repos/mcp-python-sdk/` — stdio JSON-RPC + FastMCP idioms + known
  failure modes. Read when working on `lv.serve_stage` shape or
  considering wire-level decisions in the `leaven-acp` Rust crate.

Phase 2 vendored (2026-05-24) — eval framework compatibility targets:

- `repos/verifiers/` — Prime Intellect environments: `load_environment()`,
  Taskset/Harness v1, `@vf.reward` rubrics, HF dataset rows. Read when
  designing `lv.x.verifiers.*` adapters or mapping case/scorer semantics
  for RL-style eval environments.
- `repos/harbor/` — container agent eval harness: task directories,
  `BaseAgent`/`BaseVerifier`, dataset registry, `harbor run` jobs. Read
  when designing `lv.x.harbor.*` adapters or importing Harbor task layouts
  into `lv.cases.*`. **Not** `inspect_harbor` (Inspect AI registry glue).

Round 4 vendored (2026-05-24) — high-taste references:

- `repos/baml/` — closest architectural peer (Rust core + per-language
  typed SDKs + schema-codegen). Read `engine/language_client_python/`,
  `baml_language/sdks/python/`.
- `repos/pydantic-ai/` — literal `RunContext` name match, multi-provider
  lowering. Read `pydantic_ai_slim/`, `pydantic_graph/`. `tests/`
  locally pruned (~221 MB VCR cassettes).
- `repos/temporal-python-sdk/` — Python decorators backed by Rust core
  with replay determinism. Read `temporalio/worker/`, `workflow.py`,
  `activity.py`.
- `repos/marvin/` — high-ergonomic surface on top of pydantic-ai.
- `repos/weave/weave/` — `@weave.op()` decorator (UX target). **Informal
  sparse-clone copy**, not subtree-tracked; see
  `repos/weave/README-leaven.md`.
- `repos/anthropic-sdk-python/` — major LLM provider Python SDK shape.
- `repos/braintrust-sdk-python/` — Python tracing/evals SDK, `Eval(...)`,
  span/logging APIs, OpenTelemetry bridge, pytest plugin, and agent/model
  auto-instrumentation patterns.
- `repos/jupyter-client/` — battle-tested stdio/ZMQ RPC patterns.
- `repos/python-lsp-jsonrpc/` — 120 KB minimal Python JSON-RPC reference.

## Verification

- `uv sync` succeeds.
- `uv run python -c "import leaven; print(dir(leaven))"` lists the top-level
  surface without import errors.
- `just check` runs lint, types, and compiles `examples/*.py`.
- `uv run ruff check src/leaven examples tests --exclude src/leaven/_types`
  passes linting.
- `uv run ty check src/leaven tests --exclude src/leaven/_types` passes type
  checking.
- `uv run pytest` passes the current Python package tests.
- No checked-in Python file exceeds 650 lines. Generated schema modules are not
  checked in until the codegen output has a deliberate split policy.
- `cargo build -p leaven-cli` then `uv run python examples/03_prompt_optimize.py`
  runs the current wired mechanics path over the durable public seam server:
  it spawns `leaven seam serve --stdio --config`, sends runner
  `leaven/stage.run` requests, and returns a typed `Optimized[PromptArtifact]`.
  Expected current output is `seed score: 0.000` / `best score: 0.000` plus the
  seed prompt. This is deterministic mechanics evidence, not optimizer-search
  product proof. The remaining examples print composed types and canonical
  sketches only unless their own comments name a live-gated proof.
