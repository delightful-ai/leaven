# Examples

Ten runnable example scripts that show the full Leaven Python surface.
Most examples are composable Python that imports cleanly, typechecks under
`ty`, and prints something illustrative. Bodies that would normally hit
the engine raise `NotImplementedError` (caught and printed as `(expected)`)
because the scaffold has no implementation behind the ergonomic SDK surface
yet. Example 03 is the no-spend wired prompt path; example 10 is live-gated
Codex seam evidence.

## Run

From `docs/specs/leaven_py/`:

```bash
uv sync                           # one-time
uv run python examples/01_runtime.py
just examples                     # run all ten in order; live-gated examples skip by default
just example 03                   # run just one (by number prefix)
LEAVEN_LIVE_CODEX=1 just example 10
```

## The tour

| # | File | Shows |
|---|------|-------|
| 01 | `01_runtime.py` | Every slot `lv.runtime(...)` accepts: workspace, multi-role LMs, multi-role agents, sandbox, trust profile, budget, cache; then a minimal `lv.optimize(...)` compose showing where the runtime lands. |
| 02 | `02_cases_and_artifacts.py` | `lv.PromptArtifact`, `lv.SkillBank` + skill files, JSONL case loader call shape, hand-built `lv.Case` with `split=` tags. |
| 03 | `03_prompt_optimize.py` | **The canonical minimal sketch** — `Rollout.fn(run)` + `Rubric([exact])` + `lv.optimize(environment=...).run()`, typed `InputCaseView`/`RolloutContext` (target-free) and `ScoringCaseView`/`RubricContext`. |
| 04 | `04_evoskill_skill_bank.py` | **The canonical big sketch** — `Rollout.agent()` + multi-reward `Rubric` + `gepa(propose=Propose.agent_edit(...))` over a SkillBank, EvoSkill-class shape with no runner body. |
| 05 | `05_evaluator_with_judge.py` | **Advanced seam** — rich `@lv.evaluator` body (`cx.case.load`, `cx.batch()` fan-out, `cx.agent.run` with `lv.output.json_schema(...)`, public/private evidence). Ordinary scoring is a `Rubric`; reach here only when it isn't enough. |
| 06 | `06_reflect_propose_custom.py` | Custom `@lv.reflector` + `@lv.proposer` attached via `gepa(reflect=Reflect.fn(...), propose=Propose.fn(...))` — the load-bearing reflection/proposal stage split, typed `ReflectContext`/`ProposeContext`. |
| 07 | `07_serve_stage_worker.py` | Standalone Python worker — `lv.serve_stage(my_judge)` script the engine spawns over ACP stdio. Same `@lv.judge` decorator shape as in-process, typed `JudgeContext`. |
| 08 | `08_dspy_dropin.py` | `dspy.configure(lm=lv.x.dspy.LeavenDSPyLM(...))` — existing DSPy modules unmodified through Leaven's LM seam. |
| 09 | `09_full_repro.py` | **The front-door showcase** — every product role on the new surface: `Rollout.agent()` + multi-reward `Rubric` in the `Environment`; `gepa(reflect=, propose=, judge=, objective=)` outer loop with a multi-LM runtime. |
| 10 | `10_live_codex_seam.py` | **Live-gated substrate proof** — Python spawns `leaven seam serve --stdio --config`, sends `leaven/agent.run`, and checks Codex CLI `agent_session` receipts/transcript refs. This is direct public-seam evidence, not the finished `cx.agent.run` SDK. |

## Fixtures

`fixtures/arithmetic.jsonl` — 8 trivial-to-medium arithmetic QA cases used by examples 03, 04, 06, 09. Each line is one `{id, input, target, metadata}` record matching the JSONL loader's default fields.

## What this is not

Most examples don't run a real optimization (the scaffold has
`NotImplementedError` at every effect boundary). They exist so:

- You can read the file and the SHAPE of user code fires your taste
- IDE autocomplete works on every decorator, builder, and context object
- `ty` proves the type signatures hold across the full surface
- When the engine wires up behind the seam, the same example files run
  end-to-end without source changes
- Live-gated substrate proofs can exercise the real public seam without
  pretending the high-level SDK path is finished

When something feels wrong in an example, the spec at
[`../../leaven_python.md`](../../leaven_python.md) is the governing truth;
update the spec and these examples in the same change.
