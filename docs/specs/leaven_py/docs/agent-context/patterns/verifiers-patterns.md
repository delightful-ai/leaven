# Verifiers Patterns — Leaven Python Scaffold Compatibility

**Date:** 2026-05-24  
**Vendored at:** `repos/verifiers/` (`PrimeIntellect-ai/verifiers@main`)  
**Scope:** What existing Verifiers environment code users bring, and what Leaven
must provide under `lv.x.verifiers.*` for low-friction adoption.

Verifiers is Prime Intellect's library for RL/eval **environments**: a dataset of
task rows, a model **harness** (tools, sandboxes, multi-turn control flow), and a
**rubric** (reward/score functions). It is tightly coupled to the Prime CLI
(`prime eval run`) and Environments Hub, but the Python module shape is the
compatibility target for Leaven.

**Related but distinct:** `meridianlabs-ai/inspect_harbor` is Inspect AI registry
glue, not this repo. Verifiers ships its own Harbor bridge at
`verifiers/v1/packages/tasksets/harbor.py`.

---

## 1. What to read first

| File | Why |
|------|-----|
| `repos/verifiers/docs/overview.md` | Canonical `load_environment()` examples (legacy + v1 Taskset/Harness). |
| `repos/verifiers/verifiers/v1/taskset.py` | Task row source, `rows()` / `eval_rows()`, HF `Dataset` materialization. |
| `repos/verifiers/verifiers/v1/env.py` | v1 `Env`: wires taskset + harness, `rollout()` / group rollouts. |
| `repos/verifiers/verifiers/v1/harness.py` | Multi-turn execution graph, tool/sandbox stages. |
| `repos/verifiers/verifiers/rubrics/rubric.py` | Reward function contract + weighted aggregation. |
| `repos/verifiers/verifiers/decorators.py` | `@vf.reward`, `@vf.stop`, `@vf.setup`, lifecycle discovery. |
| `repos/verifiers/verifiers/v1/ENVIRONMENT_BEST_PRACTICES.md` | Load-bearing module contract (`load_environment(config: vf.EnvConfig)`). |
| `repos/verifiers/verifiers/v1/packages/tasksets/harbor.py` | How Verifiers imports Harbor task layouts into Taskset rows. |

---

## 2. What existing API code users bring

### Environment module entrypoint

Users ship a Python package with:

```python
import verifiers as vf

def load_environment(config: vf.EnvConfig) -> vf.Env:
    ...
```

Legacy environments still use direct constructors (`vf.SingleTurnEnv`,
`vf.MultiTurnEnv`) inside `load_environment`. New code uses v1
`Taskset` + optional custom `Harness`.

**Leaven implication:** adapter namespace should accept a user's
`load_environment` callable (or import path) and treat the returned object as
an opaque environment handle — do not require rewriting into Leaven-native
modules upfront.

### Dataset / task rows (v1 Taskset)

Tasksets expose rows as dicts, often with fields like:

- `prompt` — chat messages or string input
- `answer` — ground truth for rubrics (metadata, not always shown to model)
- `task_id` / `id`, `split`, `max_turns`, custom bindings

Rows come from:

- inline `rows()` on a `Taskset` subclass
- HuggingFace datasets via `source` / `eval_source` config
- bundled `tasks/` directory next to the environment module
- Harbor datasets via `HarborTaskset` (`dataset="terminal-bench@2.0"`, etc.)

**Leaven mapping → `lv.cases.*`:**

| Verifiers row field | Leaven `Case` field | Notes |
|---------------------|---------------------|-------|
| `task_id` / `id` | `id` | Stable case identity for cache/resume. |
| `prompt` (str or messages) | `input` | Normalize chat → Leaven input projection. |
| `answer` and rubric metadata | `target` | Hidden from reflectors per Leaven seam law. |
| remaining row keys | `metadata` | `split`, `max_turns`, Harbor paths, tool bindings. |

Prefer `lv.cases.from_iterable(...)` for adapter materialization, or a dedicated
`lv.x.verifiers.load_taskset_rows(taskset)` that preserves Verifiers row keys in
metadata for rubric compatibility.

### Rubrics / reward functions

Two styles coexist:

**Legacy:** `vf.Rubric(funcs=[async_fn, ...], weights=[...])` where each fn
takes `(completion, answer, **kwargs)` and returns `float`.

**v1:** `@vf.reward(weight=1.0)` on methods or module functions; discovered and
bound to tasksets / harness runtime. Signatures accept `(task, state)` or
completion/answer forms depending on registration site.

**Leaven mapping → `@lv.scorer` / assessments:**

| Verifiers | Leaven |
|-----------|--------|
| `Rubric.score_rollout(s)` | `@lv.scorer` producing `lv.Assessment` rows |
| weighted reward funcs | multi-metric assessments or composite score |
| `answer` in rubric kwargs | `case.target` via scorer-only visibility |
| `State` completion transcript | runner output + stage payload materialization |

Minimal glue: wrap each `@vf.reward` function as a Leaven scorer that adapts
`State` → completion string and `Task` → `lv.Case`, then forwards to the user's
reward logic unchanged.

### Harness / rollout semantics

`Env.rollout(input, client, model, sampling_args)` runs the harness against one
task row. Multi-turn envs loop until `@vf.stop` conditions or turn limits.
Tool/sandbox envs delegate to harness stages (`verifiers/v1/harness.py`).

**Leaven mapping → `@lv.runner`:**

| Verifiers concept | Leaven stage |
|-------------------|--------------|
| harness model calls | `@lv.runner` + `cx.lm` / agent builder |
| tool execution | `@lv.runner` + `cx.sandbox` / workspace |
| `State` mutation | runner output record + explicit payload updates |
| group rollouts | evaluator batching / population stages (not 1:1) |

Do **not** try to reimplement Verifiers harness graphs inside Leaven Python.
Expose an adapter that runs the user's existing `Env.rollout` inside a Leaven
worker process when they opt into `lv.x.verifiers.use_existing_env()`.

---

## 3. What Leaven must provide (`lv.x.verifiers.*`)

Recommended adapter surface (names illustrative; spec must ratify):

```python
import leaven as lv

# Register user's environment module
env = lv.x.verifiers.load_environment("my_env", config={...})

# Materialize cases from taskset rows
cases = lv.x.verifiers.cases_from_env(env, split="eval")

# Wrap legacy rubric rewards as Leaven scorers
@lv.scorer
async def vf_score(output, case, cx):
    return lv.x.verifiers.score_with_rubric(env.rubric, output, case)

lv.optimize(seed=...)
    .train(cases.train)
    .val(cases.val)
    .runner(lv.x.verifiers.runner_for_env(env))
    .scorer(vf_score)
    .run()
```

**Must preserve without rewrite:**

1. User's `load_environment` module and config objects (`vf.EnvConfig`).
2. Reward function bodies (including `@vf.reward` decorators).
3. Task row keys needed by rubrics (`answer`, custom bindings).
4. Optional: pass-through to `prime eval` for users who want Prime CLI parity.

**Must translate:**

1. Row dicts → `lv.Case` with target isolation rules.
2. `State` / completion transcripts → runner output shape.
3. HF dataset splits → `lv.cases.splits(train=..., val=...)`.
4. OpenAI-compatible client config → `lv.lm` builder (similar to `lv.x.dspy`).

---

## 4. Dataset loading semantics (detailed)

### HuggingFace / example datasets

`verifiers.utils.data_utils.load_example_dataset(name)` returns preprocessed HF
rows (`question`/`answer`, etc.). These are **reference helpers**, not core API.

Leaven should **not** vendor-copy HF dataset catalogs. Instead:

- map normalized rows through `lv.cases.from_iterable`
- store provenance in case metadata (`metadata["verifiers_dataset"] = name`)

### Taskset `source` / `eval_source`

Tasksets lazy-load rows via `rows_from_source()`; eval splits can differ from
train (`eval_rows()`). This mirrors Leaven's train/val/test split bundles in
`lv.cases.splits(...)`.

Adapter rule: call `taskset.eval_rows()` for validation/eval CaseSets and
`taskset.rows()` for training when the user distinguishes them.

### Harbor-backed tasksets

`HarborTaskset` downloads Harbor registry datasets and expands task directories
into Verifiers rows (instruction, verifier scripts, sandbox config in metadata).

For users with **Harbor tasks only**, prefer `lv.x.harbor.*` first; use
`lv.x.verifiers` when they already wrapped Harbor inside a Verifiers environment
module.

---

## 5. Task semantics vs Leaven stages

| User mental model (Verifiers) | Leaven stage | Friction |
|------------------------------|--------------|----------|
| Environment = dataset + harness + rubric | `optimize(...).train/val + runner + scorer` | Split env into case load + stages |
| `load_environment()` factory | adapter import hook | Low if we preserve signature |
| `@vf.reward` | `@lv.scorer` | Medium — signature adapter |
| Multi-turn harness | `@lv.runner` loop or passthrough rollout | High if reimplemented; low if delegated |
| `prime eval run` CLI | `lv.optimize(...).run()` | Optional parallel path |
| RL trainer (`verifiers/rl/`) | Rust optimizer engine | **Out of scope** for v1 adapter |

**Inspect AI overlap:** Verifiers v1 harness ≈ `@solver` chains; rubrics ≈
`@scorer`. Read `inspect-patterns.md` for decorator/registry comparisons.

---

## 6. Minimal glue beyond existing code

1. **`lv.x.verifiers.load_environment(import_path, **config)`** — import user module, call their `load_environment`, validate return type.
2. **`lv.x.verifiers.cases_from_taskset(taskset, *, split=None)`** — row dicts → `CaseSet`.
3. **`lv.x.verifiers.rubric_as_scorer(rubric)`** — wrap `Rubric.score_rollout` as `@lv.scorer`.
4. **`lv.x.verifiers.runner_for_env(env)`** — delegate rollout to `env.rollout` using `cx.lm` as OpenAI-compatible client.
5. **`lv.x.verifiers.configure_endpoints(...)`** — optional mapping from Prime `endpoints.toml` to `lv.lm`.

Users keep their environment package; Leaven supplies thin shims only.

---

## 7. Anti-patterns / what NOT to steal

| Do not import into Leaven | Why |
|---------------------------|-----|
| `verifiers/rl/trainer/*` | RL training loop belongs in Rust optimizer crates, not Python adapter. |
| `verifiers/serve/*` ZMQ env server | Leaven owns ACP/public seam transport, not Prime's env server protocol. |
| `prime eval` TUI / hosted eval assumptions | Optional CLI parity only; not core product path. |
| Global HF dataset helpers as benchmark catalog | Spec forbids bundling benchmark catalogs in `lv.cases.*`. |
| Verifiers `State` as public Leaven type | Keep Verifiers state inside adapter; expose Leaven payloads at boundary. |
| Environments Hub publish/upload flows | Product-specific; document as external workflow. |

**Do not confuse:** Harbor framework (`harbor-framework/harbor`) vs
`HarborTaskset` bridge vs `inspect_harbor` — three different integration layers.

---

## 8. Surprises

1. **Dual API (legacy env classes + v1 Taskset/Harness)** — adapters must detect
   which path `load_environment` returns (`vf.Environment` vs `vf.Env`).
2. **`@vf.reward` discovery is introspection-based** — similar to Inspect registry
   tagging; Leaven scorers should preserve function identity for debugging.
3. **Group rollouts** — Verifiers has first-class group advantage/reward stages;
   Leaven maps these to evaluator batch APIs, not single-case runner loops.
4. **Weave integration** — vendored Weave patches Verifiers rollouts for tracing;
   useful reference for `@lv.scorer` telemetry, not a hard dependency.
5. **GEPA module** — Verifiers ships `verifiers/gepa/` adapter code; Leaven GEPA
   lives in Rust (`leaven-gepa`); do not merge the two optimizers.

---

## 9. Recommended next adapter targets

| Priority | Surface | Purpose |
|----------|---------|---------|
| P0 | `lv.x.verifiers.load_environment` | Import user env modules unchanged |
| P0 | `lv.x.verifiers.cases_from_taskset` | Row → `lv.Case` projection |
| P0 | `lv.x.verifiers.rubric_as_scorer` | Reward funcs → `@lv.scorer` |
| P1 | `lv.x.verifiers.runner_for_env` | Delegate rollouts to existing harness |
| P1 | `lv.x.verifiers.LmClient` | OpenAI-compatible client shim over `cx.lm` |
| P2 | `lv.x.verifiers.harbor_taskset` | Re-export Harbor bridge helpers when user already uses `HarborTaskset` |

---

**Last updated:** 2026-05-24  
**Confidence:** High on module contracts; medium on RL/group-rollout mapping until spec ratifies adapter scope.
