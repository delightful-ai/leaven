# Harbor-Leaven Adapter

Status: product spec.
Date: 2026-06-21.

Purpose: define how Harbor task packages, Harbor trials, and Harbor verifier
evidence become a normal Leaven workload without making users hand-write the
current Codex Terminal-Bench glue.

This spec is grounded in the live Python SDK shape:

```python
lv.optimize(
    seed=lv.AgentKitArtifact(...),
    environment=lv.Environment(
        task=lv.Task(name=..., cases=[...]),
        rollout=lv.Rollout.fn(run),
        rubric=lv.Rubric([verifier, ctrf]),
    ),
    optimizer=lv.optimizers.gepa(...),
    runtime=lv.runtime.local(...),
)
```

`Rubric` is not a Harbor-specific judge abstraction. It is Leaven's existing
weighted vector of `@lv.reward` scorer functions over rollout output. Harbor
must plug into that shape as:

```text
Harbor task package/ref      -> Leaven Task/Case source
candidate AgentKitArtifact   -> Harbor Codex workdir materialization
Leaven rollout               -> one Harbor Trial
Harbor TrialResult           -> structured rollout evidence
Harbor verifier evidence     -> normal Leaven reward helpers
Harbor ATIF trajectory       -> safe optimizer-visible feedback/evidence
```

## 1. Existing Proof

Leaven already has a Harbor-backed proof in:

- `sdk/python/examples/codex_terminal_bench/src/codex_terminal_bench/scenario.py`
- `sdk/python/examples/codex_terminal_bench/src/codex_terminal_bench/trial.py`
- `sdk/python/examples/codex_terminal_bench/src/codex_terminal_bench/agent.py`
- `sdk/python/examples/15_live_optimize_codex_terminal_bench.py`

That proof:

- optimizes an `AgentKitArtifact`;
- materializes the current kit into a temporary directory;
- runs one Harbor `Trial` per Leaven rollout;
- uses a Harbor `LeavenCodex(Codex)` subclass to upload `AGENTS.md` and skills
  into the task workdir before Codex runs;
- serializes verifier reward, CTRF partial credit, token/cost totals, and
  trajectory path as rollout output;
- scores the rollout output with `lv.Rubric([verifier, ctrf])`;
- projects ATIF trajectory excerpts into reward feedback for reflection;
- has a deterministic no-spend fake-trial seam.

The product gap is promotion/generalization. The adapter is not a first proof of
Harbor execution.

## 2. Product Goal

A user with a local Harbor task package should be able to start from ordinary
Leaven composition:

```python
import leaven as lv

task = lv.x.harbor.task("./harbor-task", split="train")

environment = lv.Environment(
    task=task,
    rollout=lv.x.harbor.rollout.codex_agent_kit(
        model="openai/gpt-5.4",
        trials_dir=".leaven/harbor-trials",
    ),
    rubric=lv.Rubric([
        lv.x.harbor.rewards.map_key("reward", weight=1.0),
        lv.x.harbor.rewards.ctrf_fraction(weight=0.25),
    ]),
)

result = await lv.optimize(
    seed=lv.AgentKitArtifact(system_prompt="...", skills=[]),
    environment=environment,
    optimizer=lv.optimizers.gepa(
        population_size=2,
        minibatch_size=1,
        reflection_agent=lv.agent.codex(model="<configured-codex-model>"),
    ),
    runtime=lv.runtime.local(
        lm=lv.lm.openai(model="gpt-4.1-mini"),
        budget=lv.budget(metric_calls=8),
    ),
).run()
```

The first public-ish surface should live under `leaven.x.harbor` to keep Harbor
optional and avoid coupling core Leaven imports to Harbor. Native sugar such as
`lv.Rollout.harbor(...)` may be added later if the adapter proves stable.

## 3. Adapter Surface

Initial module layout:

```text
leaven.x.harbor
  task(path, *, split="train", id_prefix="harbor") -> lv.Task
  case_from_task_dir(path, *, split="train", id_prefix="harbor") -> lv.Case
  import_trial_result(path) -> HarborTrialOutcome
  trajectory_excerpt(path, *, max_steps=4, strict=False) -> str

leaven.x.harbor.rollout
  codex_agent_kit(...) -> lv.Rollout

leaven.x.harbor.rewards
  map_key(key, *, weight=1.0, id=None) -> lv.RegisteredReward
  ctrf_fraction(*, weight=1.0, id=None) -> lv.RegisteredReward
  default_rewards() -> list[lv.RegisteredReward]
```

The adapter may expose additional advanced helpers for building Harbor
`TrialConfig`, but ordinary users should not need to construct `TrialConfig`
directly.

## 4. Harbor Task Package Mapping

Input: local Harbor task directory or future Harbor task ref.

Output:

```python
lv.Task(
    name="...",
    cases=[case],
    metadata={
        "source": "harbor",
        "task_path": "...",
        "task_checksum": "...",
    },
)
```

Case shape:

```python
lv.Case(
    id="harbor_<stable_task_slug>_<split>",
    input={"harbor_task": {"path": "...", "kind": "local"}},
    target=None,
    metadata={
        "source": "harbor",
        "task_name": "...",
        "task_path": "...",
        "task_checksum": "...",
    },
    split="train",
)
```

Laws:

- `target` is `None`; Harbor verifier owns truth.
- Runner-visible case input may name the Harbor task path/ref.
- Runner-visible case input must not include `solution/`, hidden tests, or
  verifier-private content.
- Case id is stable for a stable task path/content/split.
- Temporary trial directory names must not affect case identity.
- Supplying the same task on train and validation splits is valid and matches
  the existing n=1 Terminal-Bench pattern.

## 5. AgentKit Materialization

For Codex-backed Harbor rollouts:

```text
AgentKitArtifact.system_prompt -> <workdir>/AGENTS.md
AgentKitArtifact.skills        -> <workdir>/.agents/skills/<path>
```

The current proof assumes `/app`; adapter workdir must be configurable because
not all Harbor tasks will use the same workdir.

This seam is deliberately Codex-kit-specific. A future Harbor agent adapter may
support other candidate artifacts, but the first supported artifact is
`AgentKitArtifact` because that is what the repo has proven.

## 6. Rollout Semantics

`lv.x.harbor.rollout.codex_agent_kit(...)` returns an `lv.Rollout` that is
function-backed in the current SDK:

```text
AgentKitArtifact + InputCaseView
  -> materialize kit
  -> build Harbor TrialConfig
  -> run one Harbor Trial
  -> return encoded HarborTrialOutcome string
```

The runner is target-free. It sees `InputCaseView`, not `ScoringCaseView`, and
therefore cannot read Leaven targets. Harbor's task package/verifier owns task
truth inside the trial.

Rollout configuration should include at least:

```python
codex_agent_kit(
    model="<configured-codex-model>",
    task_key="harbor_task",
    trials_dir=".leaven/harbor-trials",
    workdir="/app",
    timeout_multiplier=1.0,
)
```

Harbor/Docker/Codex spend is rollout spend. It is reported as evidence and cost
metadata, not hidden inside Leaven's LM channel. The adapter spec must not make
a full live Codex re-run on a particular model part of the default proof
denominator; live model runs are optional smoke tests behind explicit approval.

## 7. Trial Outcome Model

The rollout output is a stable, typed JSON record. It must not collapse Harbor
into an opaque pass/fail string.

Conceptual shape:

```json
{
  "trial_dir": "...",
  "rewards": {"reward": 0.0},
  "ctrf": {
    "passed": 7,
    "failed": 4,
    "total": 11,
    "failed_names": ["test_regex_matches_dates"]
  },
  "verifier_output": "verifier reward: 0\nCTRF 7/11 tests passed",
  "trajectory_path": ".../agent/trajectory.json",
  "tokens": {"input": 1234, "output": 567},
  "cost_usd": 0.0123,
  "exception": null
}
```

Decoder laws:

- Unknown fields are tolerated.
- Missing CTRF means no partial-credit evidence, not a crash.
- Missing trajectory means no trajectory excerpt, not a crash.
- Missing reward key scores as `0.0` only at reward-helper level, with feedback;
  the outcome decoder should preserve the actual map it found.
- Malformed JSON raises an actionable adapter error.
- Live rollout failures should preserve trial dir and exception detail when
  Harbor provides them.

## 8. Scoring Semantics

Leaven scores Harbor evidence through ordinary reward functions:

```python
rubric = lv.Rubric([
    lv.x.harbor.rewards.map_key("reward", weight=1.0),
    lv.x.harbor.rewards.ctrf_fraction(weight=0.25),
])
```

`map_key(key)`:

- decodes `HarborTrialOutcome`;
- reads `outcome.rewards[key]`;
- returns `lv.RewardValue(value=<float>, feedback=<summary>)`;
- returns `0.0` with explicit feedback when the key is missing.

`ctrf_fraction()`:

- decodes `HarborTrialOutcome`;
- returns `passed / total`;
- returns `0.0` when total is zero or missing;
- includes safe failed test names in feedback when available.

This is what "score it properly" means for Harbor:

- Harbor supplies structured reward/evidence dimensions.
- Leaven helper rewards choose which dimensions count for optimization.
- GEPA still selects on Leaven's normal summary score.
- The reward vector, per-dimension feedback, trial paths, logs, CTRF, trajectory,
  and cost/tokens remain available as evidence/readback material.
- Token/cost totals are not score unless the user explicitly adds a reward over
  them.

## 9. ATIF Trajectory Semantics

Harbor agents may write `agent/trajectory.json` in ATIF-ish form. Not all Harbor
agents are guaranteed to do so.

`trajectory_excerpt(path, max_steps=4)` returns a short optimizer-visible
summary of recent agent behavior.

Laws:

- Missing trajectory returns `""`.
- Malformed trajectory returns `""` unless `strict=True`.
- Only agent-authored steps and tool names are surfaced.
- Task instructions, solutions, hidden tests, and verifier internals are not
  surfaced through trajectory feedback.
- The raw trajectory path is preserved as evidence/artifact ref.

## 10. Trial Import

Running Harbor and importing Harbor results are separate capabilities.

Advanced users may already have Harbor jobs or trials. They should be able to
import a trial into the same outcome model:

```python
outcome = lv.x.harbor.import_trial_result("./jobs/.../trials/...")
```

Import laws:

- Imported outcomes use the same `HarborTrialOutcome` type as live rollouts.
- Missing optional files degrade cleanly.
- Malformed required files produce actionable adapter errors.
- Job-level aggregation is future work; trial-level import comes first.

## 11. Optional Dependency Boundary

`import leaven as lv` must work without Harbor installed.

Harbor imports must be lazy:

- task metadata helpers may work without importing Harbor if they only inspect
  local files;
- live rollout construction or execution may require Harbor;
- missing Harbor dependency errors must name the extra/package/action needed.

Core Leaven types stay generic:

- `Task`
- `Case`
- `Rollout`
- `Rubric`
- `RewardValue`
- `optimize`

The adapter lives under `leaven.x.harbor` until its shape is proven.

## 12. Runtime And Spend

Default tests must not start Docker, Codex, networked model calls, or live
Harbor trials.

The adapter must preserve a no-spend seam equivalent to the existing
`LEAVEN_CODEX_TB_FAKE_TRIAL` path.

Live runs require explicit opt-in env vars and clear operator commands. A live
smoke is optional confirmation, not required proof. In particular, this adapter
does not require fully re-running Codex tests with `gpt-5.4-mini` (or any other
specific live model) as part of default completion.

## 13. Non-Goals

- No Harbor Hub/upload integration.
- No Harbor registry publishing flow.
- No Rust core dependency on Harbor.
- No claim that all Harbor agents emit ATIF.
- No default live Docker/Codex/model spend.
- No required full live Codex re-test on `gpt-5.4-mini`.
- No primary `Rubric.harbor()` abstraction.
- No replacement of Leaven's existing `lv.optimize(... Environment(...))`
  composition model.

## 14. Acceptance Criteria

The adapter is acceptable when:

- A local Harbor task package can become an `lv.Task` without target leakage.
- A Leaven `AgentKitArtifact` can be evaluated by a Harbor Codex trial through a
  reusable adapter rollout.
- Harbor trial output is structured rollout evidence.
- Helper rewards can score Harbor reward maps and CTRF partial credit through
  normal `lv.Rubric([...])`.
- ATIF trajectory excerpts can feed reflection without leaking private task
  material.
- Existing `codex_terminal_bench` generic Harbor glue is replaced by adapter
  helpers while Terminal-Bench-specific constants remain in the example.
- All default tests are deterministic and no-spend.

## 15. Explicit Rejections

These do not count as completion:

- The old Terminal-Bench example still passes, but no reusable adapter exists.
- A `Rubric.harbor()` shortcut exists, but helper rewards and structured
  outcome evidence do not.
- `task.toml` is parsed, but `solution/` or verifier internals leak into runner
  input.
- The scorer collapses everything to one pass/fail scalar and drops reward
  vector feedback.
- A live Harbor run works, but deterministic no-spend tests do not prove the
  mechanics.
