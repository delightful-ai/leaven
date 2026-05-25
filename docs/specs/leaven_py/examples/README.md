# Examples

Ten runnable example scripts that show the full Leaven Python surface.
Every example is composable Python that imports cleanly, typechecks under
`ty`, and prints something illustrative. Bodies that would normally hit
the engine raise `NotImplementedError` (caught and printed as `(expected)`)
because the scaffold has no implementation behind the wire — yet.

## Run

From `docs/specs/leaven_py/`:

```bash
uv sync                           # one-time
uv run python examples/01_runtime.py
just examples                     # run all eight in order
just example 03                   # run just one (by number prefix)
```

## The tour

| # | File | Shows |
|---|------|-------|
| 01 | `01_runtime.py` | Every slot `lv.runtime(...)` accepts: workspace, multi-role LMs, multi-role agents, sandbox, trust profile, budget, cache. |
| 02 | `02_cases_and_artifacts.py` | `lv.PromptArtifact`, `lv.SkillBank` + skill files, JSONL case loader call shape, hand-built `lv.Case`. |
| 03 | `03_prompt_optimize.py` | **The canonical minimal sketch** — 25-line `@lv.runner` + `@lv.scorer` + `lv.optimize(...).run()` shape against arithmetic fixtures. |
| 04 | `04_evoskill_skill_bank.py` | **The canonical big sketch** — GEPA + workspace materialize + agent run + skill bank, EvoSkill-class shape in ~80 lines. |
| 05 | `05_evaluator_with_judge.py` | Rich `@lv.evaluator` body — `cx.case.load`, `cx.batch()` for fan-out, `cx.agent.run` with `lv.output.json_schema(...)`, assessment + public/private evidence. |
| 06 | `06_reflect_propose_custom.py` | Custom `@lv.reflector` + `@lv.proposer` overriding GEPA defaults — the load-bearing reflection/proposal stage split. |
| 07 | `07_serve_stage_worker.py` | Standalone Python worker — `lv.serve_stage(my_judge)` script the engine spawns over ACP stdio. Same decorator shape as in-process. |
| 08 | `08_dspy_dropin.py` | `dspy.configure(lm=lv.x.dspy.LeavenDSPyLM(...))` — existing DSPy modules unmodified through Leaven's LM seam. |
| 09 | `09_full_repro.py` | **The big repro sketch** — all 6 stage roles in one file (runner + scorer + reflector + proposer + judge + evaluator), multi-LM runtime, EvoSkill-shaped composition. Stress-tests the full surface. |
| 10 | `10_stage_composition.py` | **The new surface direction** — `Artifact x Task x Stages x Runtime` with explicit, swappable stage objects. |

## Fixtures

`fixtures/arithmetic.jsonl` — 8 trivial-to-medium arithmetic QA cases used by examples 03, 04, 06. Each line is one `{id, input, target, metadata}` record matching the JSONL loader's default fields.

## What this is not

These examples don't run a real optimization (the scaffold has
`NotImplementedError` at every effect boundary). They exist so:

- You can read the file and the SHAPE of user code fires your taste
- IDE autocomplete works on every decorator, builder, and context object
- `ty` proves the type signatures hold across the full surface
- When the engine wires up behind the seam, the same example files run
  end-to-end without source changes

When something feels wrong in an example, the spec at
[`../../leaven_python.md`](../../leaven_python.md) is the governing truth;
update the spec and these examples in the same change.
