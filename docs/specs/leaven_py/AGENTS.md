# AGENTS — docs/specs/leaven_py

## Boundary

This directory is a **scaffold instance** of the governing spec at
`docs/specs/leaven_python.md`. It is a real Python package — importable,
typed, IDE-navigable — whose only purpose is to make the spec's surface
concrete enough to fire taste against before implementation begins.

It is mostly scaffold: nearly every function and method body is
`raise NotImplementedError(...)` or `...`. Do not add real behavior here
beyond the one wired path described next.

### The one wired path (example 03, prompt/LM/exact-match)

`examples/03_prompt_optimize.py` runs FOR REAL over the locked Leaven worker
bidirectional seam. The wired surface is exactly the slice-3 path and no
more: `optimize().run()`, `cx.lm.complete(prompt=...)`, `cases.from_jsonl`,
and the `Environment`/`Task`/`Rollout.fn`/`Rubric`/`runtime.local` records
those compose. Everything else stays `NotImplementedError`.

Directionality (the crux, fixed by `docs/specs/leaven_python.md` "the wire"):
`optimize().run()` SPAWNS `leaven serve --stdio` as a child. The Rust child
owns the tiny real GEPA accept loop, the deterministic host mock LM, and
INITIATES `leaven/stage.run`. This Python package (`leaven/_serve.py`) serves
`leaven/stage.run` by running the user's `@lv.runner`, and INITIATES
`leaven/lm.complete` BACK to the child. That is a Leaven-owned JSON-RPC worker
seam, not upstream Agent Client Protocol. It is the Python generalization of
the proven Rust worker
`crates/leaven-acp-stage-bridge/worker/serve_stage_runner.py`.

Honest scope: the seam, stage dispatch, and GEPA-shaped accept are real; the LM
is a deterministic mock (no spend, no network). The exact-match reward and the
reflector run host-side in `leaven serve` and are named declaratively in the
plan (`_serve.run_optimization`), so the Python `@lv.reward`/reflect bodies are
not yet executed. The reward vector, agent, sandbox, message-list LM, and
Python-side reward/reflect are later slices and remain scaffold.

The binary is resolved via `LEAVEN_BIN`, else `target/{debug,release}/leaven`
under the repo root (`LEAVEN_REPO_ROOT` override). Build it with
`cargo build -p leaven-cli`. The repo-root walk uses the topology-contract marker
so the example runs regardless of cwd.

It is not the published `leaven` package. The eventual published package
will live in its own repository, built against this spec.

## Public API discipline (load-bearing)

Two tiers, one rule. See `docs/specs/leaven_python.md` "Public API
discipline" for the governing spec section.

- **Public:** in module `__all__`, no leading underscore anywhere in the
  path, documented in the spec, frozen + `extra="forbid"` if a pydantic
  model.
- **Private:** leading underscore on file or symbol, not in any
  `__all__`. May break between versions.

The rule: **if it isn't in `__all__` and the name is unprefixed in a
public module, it does not exist.**

When editing the scaffold:

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

- The governing spec is `docs/specs/leaven_python.md`. If this scaffold and
  the spec disagree, the spec wins; this scaffold is updated to match.
- Every module gets a docstring at the top explaining what it is and pointing
  to the relevant spec section.
- Public functions/classes get full type hints, signatures, and docstrings
  but `raise NotImplementedError(...)` bodies (or `...` for pure-type stubs).
- Pydantic v2 models for wire-shaped types. Dataclasses for internal config
  shapes. `from __future__ import annotations` at the top of every file.
- Top-level `lv.*` imports are the public surface; submodule paths
  (`lv.optimizers.gepa`, `lv.lm.anthropic`) are also part of the public
  surface where the spec names them.
- Internal modules (`leaven.transport.*`, `leaven._types.*`) get leading
  underscore conventions or live under `_types/`.
- Do not introduce dependencies that aren't in `pyproject.toml`. Adding a
  dep is a taste call worth surfacing in a docstring.

## Editing this scaffold

If you change a shape in this scaffold, update the corresponding section of
`docs/specs/leaven_python.md` in the same change. The two stay in sync.

If you want to add a new module to capture a surface the spec didn't
enumerate explicitly, surface the proposed shape and the spec section it
extends as a question before writing the module. The scaffold should not
grow beyond what the spec describes.

## Vendored Repositories

This scaffold vendors external repositories under `repos/` for agent
reference only. They are read-only inspiration; they are **not** runtime
dependencies of `leaven`.

Discipline:

- Use vendored repos as read-only reference when refining the scaffold's
  shape against real-world API ergonomics, idioms, tests, and known
  failure modes.
- Prefer examples and patterns from vendored source over guesses.
- Do not edit files under `repos/`.
- Do not import from `repos/` in `leaven` source or `examples/`.
- Do not include `repos/**` in broad formatter, linter, or codegen
  passes. `pyproject.toml`'s ruff `extend-per-file-ignores` exempts
  `repos/**` from linting.
- If a vendored repo carries `AGENTS.md`, `CLAUDE.md`, `LLMS.md`, or
  developer docs, read those first when writing code against that
  library's idioms.

The full vendored inventory plus add/update commands and "what to read
first" hints live at
[`docs/agent-context/vendored-repositories.md`](docs/agent-context/vendored-repositories.md).

Per-dependency pattern notes (what we'd steal, what we'd avoid, what's
surprising) live at
[`docs/agent-context/patterns/`](docs/agent-context/patterns/).

Phase 1 vendored (2026-05-24) — direct spec citations:

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
- `uv run ty check src/leaven --exclude 'src/leaven/_types/**' --force-exclude`
  passes type checking. `_types/**` is generated schema projection and remains
  excluded until codegen emits Ty-legal constrained aliases instead of pydantic
  `constr(...)`/`conint(...)` calls in type positions.
- `uv run ruff check src/leaven examples` passes linting.
- `cargo build -p leaven-cli` then `uv run python examples/03_prompt_optimize.py`
  runs the one wired path FOR REAL over the live Leaven worker seam: it spawns
  `leaven serve --stdio`, optimizes the seed prompt, and prints `seed score: 0.000`
  / `best score: 1.000` plus the optimized template. This is the slice-3 product
  proof. The remaining examples print composed types and canonical sketches only.

`tests/test_stage_surface.py` and `tests/test_product_surface_ring.py` reference
the pre-cutover `evolve`/`Stages`/`scorer` surface that the
`optimize(environment)` cutover removed; they are stale and are not part of the
named verification above. Bring them in line with the current surface in a
focused change before relying on `uv run pytest`.
