# AGENTS — docs/specs/leaven_py

## Boundary

This directory is a **scaffold instance** of the governing spec at
[`docs/specs/leaven_python.md`](../leaven_python.md) (revised 2026-05-29). It
is a real Python package — importable, typed, IDE-navigable — whose only
purpose is to make the spec's surface concrete enough to fire taste against
before implementation begins.

It is not implementation. Every function and method body is
`raise NotImplementedError(...)`, `...`, or a no-op pass-through for the
optional stage decorators. Do not add real behavior here.

It is not the published `leaven` package. The eventual published package will
live in its own repository, built against this spec.

If this scaffold and the spec disagree, **the spec wins**; update the scaffold
to match. If you change a shape here, update the corresponding section of the
spec in the same change.

## The core law (load-bearing)

```text
evolution = artifact × task × stages × optimizer × runtime
```

Entry point: `await lv.evolve(artifact=, task=, stages=, optimizer=,
runtime=).run()` returns `Evolved[Artifact]`.

## Stages: EXACTLY four slots

`Stages` has exactly `{rollout, score, reflect, propose}` — keyword-only.
There is **no** `evaluate`, `improve`, `judge`, `layout`, or `sampler` slot,
no fifth field, and no `Reflect.gepa()` preset. A test locks
`Stages.__init__` to those four params. `rollout` + `score` are required;
`reflect`/`propose` are optional (GEPA installs Codex-backed defaults).
`Stages.evaluator(...)` is the advanced alternate constructor that replaces
`rollout`+`score`.

The four transforms map to the optimizer-agnostic phase model:
`rollout → score → reflect → propose`.

## Stages are functions; built-ins are for the no-Python case

Each slot takes **either** a plain async function (custom logic; agentic work
via `cx.*` primitives inside) **or** a declarative engine-mediated built-in:

```text
lv.Rollout.agent(agent,...) / .command(argv,...) / .manifest(path,...)
lv.Reflect.agent(agent,...)
lv.Propose.agent_edit(agent,...)
```

- `cx` is passed **explicitly** to every stage fn — no ContextVar magic.
- `score` takes one `Scorer` or `Sequence[Scorer]`. `Scorer` is a **type alias
  only** (`Callable[[RolloutResult, Case, Context], Awaitable[Score]]`) exported
  for annotations — there is **no `Scorer` constructor/class**. A scorer is a
  plain async fn; agentic scoring is `cx.agent.run(...)` inside it.
- `Score` is `{value: float, feedback: str = ""}` — no `metrics`, no `output`.
  Multiple objectives are multiple named scorers.
- The optimizer references the primary score by the **scorer object**
  (`gepa(score=correctness)`); a name string is accepted as convenience.
- Decorators `@lv.runner @lv.scorer @lv.reflector @lv.proposer @lv.evaluator`
  are optional sugar (tag + name). They are load-bearing only when a stage is
  served out-of-process via `lv.serve(...)`. In the scaffold they are no-op
  pass-throughs so example modules import.

## Rings (the physical module split)

```text
lv.*            product nouns ONLY (the allow-list below)
lv.adapters.*   advanced authoring: Evaluator/@lv.evaluator support, RegisteredStage,
                RunContext/StageContext/EvalContext typed annotations,
                reflective types (ReflectiveBatch/ReflectiveCase/ReflectiveRun/
                Attachment/TraceRef)
lv.wire.*       generated public-seam schema records (OutputRecord, Visibility,
                EvaluationJob/Item, Granularity, Purpose, EvidenceEnvelope/Public/
                Private, AssessmentWrite, Replayability, ProposalBatch,
                ProposalEffect, Reflect/Propose/JudgeRequest, ReflectExample,
                ReflectionResult, StageSourceRef, StageRole,
                Query/Call/WriteReceipt)
lv._engine.*    private engine helpers; no user reach
```

The rule: **a user reading the top-level `lv.*` import surface sees product
nouns only.**

### Top-level `__all__` allow-list (exactly these — nothing more)

`__version__, Task, Case, Score, Scorer, RolloutResult, Critique, Proposal,
Stages, Rollout, Reflect, Propose, Runtime, runtime, budget, evolve, serve,
runner, scorer, reflector, proposer, evaluator, artifacts, optimizers, lm,
agent, sandbox, workspace, layouts, output, cases, setup, assets, runs, gepa,
trust, x`.

`Scorer` is the annotation-only alias. `runtime` is BOTH the callable and
carries `.local`/`.acp`. `Runtime` is the class.

### Forbidden from top-level `__all__` (live in adapters/wire/_engine)

A surface test asserts NONE of these appear in `lv.__all__`:

`OutputRecord, Visibility, EvaluationJob, EvaluationItem, Granularity, Purpose,
EvidenceEnvelope, EvidencePublic, EvidencePrivate, AssessmentWrite,
Replayability, ProposalBatch, ProposalEffect, ReflectRequest, ProposeRequest,
JudgeRequest, ReflectExample, ReflectionResult, ReflectiveCase, ReflectiveRun,
Attachment, TraceRef, StageSourceRef, StageRole, RegisteredStage, RunContext,
StageContext, EvalContext, RunCase, ScoreCase, CandidateHandle, WorkspaceHandle,
WorkspaceLifetime, WorkspaceSurface, QueryReceipt, CallReceipt, WriteReceipt`.

Examples must use ONLY the allow-list nouns plus the documented namespaces
(`lv.artifacts.*`, `lv.optimizers.*`, `lv.gepa.*`, `lv.lm.*`, `lv.agent.*`,
`lv.sandbox.*`, `lv.workspace.*`, `lv.layouts.*`, `lv.output.*`, `lv.setup.*`,
`lv.assets.*`, `lv.cases.*`, `lv.runs.*`, `lv.x.*`, `lv.trust`). For advanced
annotations they may reach `lv.adapters.*` (e.g. `cx: lv.adapters.RunContext`)
and `lv.artifacts.PromptArtifact`. They must NOT name a `lv.wire.*` record.

## codex_kit mutable-surface rule

`lv.artifacts.codex_kit(root, mutable=[...])` REQUIRES `mutable=` and validates
it against the known surface:

```text
default mutable:  AGENTS.md, .agents/skills/**/SKILL.md, dev_instructions.md
opt-in  mutable:  task_message.md, hooks.toml, mcp.json, tool_policy.toml
not artifact:     codex_kit.toml, .codex/
```

Paths outside the known surface require `lv.artifacts.unsafe("path")` (warns at
construction). Edits outside the `mutable=` patterns are rejected on readback.
The Rust adapter `crates/leaven-artifact-codex-kit` is UPCOMING; the Python
adapter lowers to it via the locked ACP wire.

## Public API discipline (mechanics)

- **Public:** in module `__all__`, no leading underscore anywhere in the path,
  documented in the spec, frozen + `extra="forbid"` if a pydantic model.
- **Private:** leading underscore on file or symbol, not in any `__all__`.
- The rule: **if it isn't in `__all__` and the name is unprefixed in a public
  module, it does not exist.**
- Every public module ships a sorted `__all__` (ruff `RUF022` is on).
- Pydantic v2 frozen models (`ConfigDict(frozen=True, extra="forbid")`) for
  wire-shaped / value records; `@dataclass(frozen=True, slots=True)` for
  internal config (builder configs). `from __future__ import annotations` is
  the first line of every module.
- Python >=3.12 syntax only: `X | None`, builtin generics, `class Foo[T]`,
  `type Alias = ...`. NEVER `typing.List/Dict/Optional`.
- Reserved-but-not-real names raise `NotImplementedError`:
  `lv.optimizers.{mipro,textgrad,trace}`, `lv.agent.{claude_code,opencode}`.
  `lv.environment(...)` and `lv.optimize(...)` are deprecated aliases.

## Editing this scaffold

- Every module gets a top docstring pointing at the relevant spec section.
- Adding a top-level product noun? Add it to the owning module's `__all__`,
  surface it through `src/leaven/__init__.py`, and confirm it belongs on the
  allow-list. If it is engine/wire machinery, it goes in `lv.adapters`/`lv.wire`
  instead — never the top level.
- The scaffold should not grow beyond what the spec describes. If you want a
  module the spec didn't enumerate, surface the proposed shape and the spec
  section it extends as a question first.

## Vendored Repositories

This scaffold vendors external repositories under `repos/` for **read-only**
agent reference. They are not runtime dependencies of `leaven`.

- Use vendored repos as read-only reference when refining the scaffold's shape
  against real-world API ergonomics, idioms, tests, and known failure modes.
- Do not edit files under `repos/`; do not import from `repos/` in `leaven`
  source or `examples/`; do not include `repos/**` in formatter/linter/codegen
  passes (`pyproject.toml`'s ruff config exempts `repos/**`).
- If a vendored repo carries `AGENTS.md`/`CLAUDE.md`/`LLMS.md`, read it first.

Inventory + "what to read first" hints:
[`docs/agent-context/vendored-repositories.md`](docs/agent-context/vendored-repositories.md).
Per-dependency pattern notes:
[`docs/agent-context/patterns/`](docs/agent-context/patterns/).

Highest-signal references for the current surface:

- `repos/inspect_ai/` — `@scorer`/`@solver`/`@task` + context injection; the
  `scorer=[accuracy(), f1()]` self-named-list shape that `score=[...]` mirrors.
- `repos/dspy/` — `dspy.BaseLM` for `lv.x.dspy.LeavenDSPyLM`.
- `repos/pydantic-ai/` — literal `RunContext` name match, multi-provider lowering.
- `repos/mcp-python-sdk/`, `repos/python-lsp-jsonrpc/`, `repos/jupyter-client/`
  — stdio JSON-RPC idioms for the `lv.serve` worker shape and the `leaven-acp`
  Rust crate.
- `repos/verifiers/`, `repos/harbor/` — `lv.x.verifiers.*` / `lv.x.harbor.*`
  compatibility targets.

## Verification

Run from `docs/specs/leaven_py/`:

- `uv sync` succeeds.
- `uv run ruff check src/leaven examples` — passes (lint + sorted `__all__`).
- `uv run ty check src/leaven` — passes (public surface typechecks).
- `uv run ty check examples` — passes (examples typecheck against the surface).
- `uv run python examples/run_all.py` — all 8 examples complete
  (`NotImplementedError` at engine boundaries is expected and reported).
- `just check` runs ruff + ty; `just examples` runs the tour; `just all` does
  sync + check + compile-examples.

No runtime tests against the engine; examples compose typed configs and print
illustrative shapes only.
