# Examples

Eight example scripts that compose the full Leaven Python surface against the
type-stub scaffold. Every example imports cleanly, typechecks under `ty`, and
lints under `ruff`. Bodies that would normally hit the engine raise
`NotImplementedError` (caught and printed as `(expected)`) because the scaffold
has no implementation behind the wire — yet. The examples are illustrative:
they compose the API and prove the shape; they do not run a real optimization.

The surface is `lv.*` product nouns plus the documented namespaces
(`lv.artifacts.*`, `lv.optimizers.*`, `lv.gepa.*`, `lv.lm.*`, `lv.agent.*`,
`lv.sandbox.*`, `lv.workspace.*`, `lv.layouts.*`, `lv.output.*`, `lv.setup.*`,
`lv.assets.*`, `lv.cases.*`, `lv.runs.*`, `lv.x.*`, `lv.trust`). No
wire/adapter floorboard nouns leak into example code.

## Run

From `docs/specs/leaven_py/`:

```bash
uv sync                              # one-time
uv run python examples/03_prompt_evolve.py
just examples                        # run all eight in order
just example 04                      # run just one (by number prefix)
```

## The tour

| # | File | Shows |
|---|------|-------|
| 01 | `01_runtime.py` | Compose a `runtime`: LMs, the engine-mediated agent executor, sandbox, workspace, trust profile, budget; multi-LM / role-keyed agents; `runtime.local(...)` / `runtime.acp(...)` shortcuts. |
| 02 | `02_task_and_cases.py` | The task world — `Task`/`Case` with user-defined `split` labels, `files=` via `lv.assets.path`, `setup=` via `lv.setup.bash`, plus the `lv.cases.from_jsonl(splits=...)` loader. |
| 03 | `03_prompt_evolve.py` | **The minimal program** — `@lv.runner` + `@lv.scorer` functions, Codex-backed `Reflect.agent` / `Propose.agent_edit`, `lv.optimizers.gepa(score=correctness)`, `lv.evolve(...).run()`. |
| 04 | `04_codex_kit_mvp.py` | **The flagship** — `lv.artifacts.codex_kit` with a validated `mutable=` surface, `Rollout.agent(codex)`, a scorer reading `run.workspace`/`run.sessions`, `Reflect.agent` + `Propose.agent_edit`, GEPA, `runtime.local`. The sole custom Python is the scorer. |
| 05 | `05_multi_scorer.py` | `score=[a, b, judge]` self-named scorers (incl. an agentic LLM-judge scorer), with `gepa(score=lv.gepa.compare.weighted({...}))` keyed by scorer objects. |
| 06 | `06_custom_reflect_propose.py` | Custom `@lv.reflector` + `@lv.proposer` functions over the pre-built, target-safe batch — the load-bearing reflect/propose stage split (diagnosis vs. mutation intent). |
| 07 | `07_serve_worker.py` | `lv.serve(rollout=, score=)` — the out-of-process worker entry for the external-driver deployment mode; only Python-authored stages are served. |
| 08 | `08_dspy_dropin.py` | `lv.x.dspy.LeavenDSPyLM(...)` LM seam + `lv.x.dspy.artifact(program=...)` — DSPy through the `x` adapter namespace. |

## Fixtures

`fixtures/arithmetic.jsonl` — trivial-to-medium arithmetic QA cases. Each line
is one `{id, input, target, metadata}` record matching the JSONL loader's
default fields (used by example 02's loader shape).

## What this is not

These examples don't run a real optimization (the scaffold has
`NotImplementedError` at every effect boundary). They exist so:

- Reading the file fires your taste on the SHAPE of user code.
- IDE autocomplete works on every builder, decorator, and context object.
- `ty` proves the type signatures hold across the full surface.
- When the engine is wired behind the seam, the same example files run
  end-to-end without source changes.

When something feels wrong in an example, the spec at
[`../../leaven_python.md`](../../leaven_python.md) is the governing truth;
update the spec and these examples in the same change.
