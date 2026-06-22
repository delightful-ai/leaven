# Harbor-Leaven Adapter Closeout Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Finish the already-started `leaven.x.harbor` adapter slice so the deterministic proof denominator in `docs/working-memory/harbor-leaven-adapter-goal-handoff.yaml` is genuinely closed.

**Architecture:** Keep Harbor as an optional extension under `leaven.x.harbor`. Core `import leaven` must not import or require Harbor. Deterministic tests prove task mapping, structured outcomes, reward helpers, trajectory safety, fake/no-spend rollout mechanics, Terminal-Bench adapter reuse, and dependency/spend boundaries; live Harbor/Codex remains optional smoke only.

**Tech Stack:** Python 3.12, Leaven Python SDK, optional `harbor==0.13.1`, pytest/pytest-asyncio, uv, existing `sdk/python/examples/codex_terminal_bench` example package.

---

## Current State

This is a closeout plan, not a from-scratch implementation plan.

Already committed in `c35c196eb6 feat(python): add Harbor adapter surface`:

- `sdk/python/src/leaven/x/harbor/` exists.
- `lv.x.harbor.task` and `case_from_task_dir` map local task directories into target-free Leaven cases.
- `HarborTrialOutcome`, `CtrfEvidence`, and `TokenEvidence` encode/decode structured trial evidence.
- `lv.x.harbor.rewards.map_key`, `ctrf_fraction`, and `default_rewards` exist.
- `trajectory_excerpt` filters to agent-authored trajectory steps.
- `lv.x.harbor.rollout.codex_agent_kit` has a deterministic `trial_runner` seam.
- Terminal-Bench imports several generic Harbor helpers from `leaven.x.harbor`.
- `uv run --project sdk/python pytest sdk/python/tests/x/test_harbor.py -q` passed with `10 passed`.

Known gap from the commit:

- `sdk/python/examples/codex_terminal_bench/tests/test_adapter_reuse.py` fails in the root SDK test environment because `harbor` is not installed/importable.

Do not redo the whole adapter unless a test proves the current committed design is wrong.

## Completion Denominator

Closeout means these required handoff rows have fresh evidence:

- `local_harbor_task_to_leaven_task`
- `structured_trial_outcome`
- `helper_rewards_score_harbor_evidence`
- `codex_agent_kit_rollout_adapter`
- `trajectory_feedback_safe`
- `codex_terminal_bench_reuses_adapter`
- `optional_dependency_and_spend_boundary`

Live Harbor/Docker/Codex smoke is explicitly optional and must not be run unless Darin approves it in that turn.

## Task 1: Make Harbor Dependency Wiring Explicit

**Files:**
- Modify: `sdk/python/pyproject.toml`
- Test: `sdk/python/tests/x/test_harbor.py`
- Test: `sdk/python/examples/codex_terminal_bench/tests/test_adapter_reuse.py`

**Step 1: Write/extend the failing dependency-boundary test**

Add a test to `sdk/python/tests/x/test_harbor.py` that asserts:

```python
def test_live_rollout_missing_harbor_error_is_actionable(monkeypatch) -> None:
    monkeypatch.setitem(sys.modules, "harbor", None)
    # call the live-run boundary directly or through a rollout with no trial_runner
    # expected: HarborAdapterError mentions the optional Harbor extra/package
```

If direct monkeypatching of `sys.modules` is too brittle because importlib treats `None` specially, use pytest's `monkeypatch` to patch `builtins.__import__` only for names starting with `"harbor"`.

**Step 2: Run the new test and confirm it fails for the right reason**

Run:

```bash
uv run --project sdk/python pytest sdk/python/tests/x/test_harbor.py::test_live_rollout_missing_harbor_error_is_actionable -q
```

Expected before implementation: failure because the message does not name the optional extra clearly, or because the test hook needs the live-run helper to be reachable.

**Step 3: Add a real optional extra for Harbor**

In `sdk/python/pyproject.toml`, add:

```toml
[project.optional-dependencies]
harbor = [
    "harbor==0.13.1",
]
```

Update `[tool.leaven.dependency-boundaries].public_optional` from `[]` to:

```toml
public_optional = [
    "harbor",
]
```

Update the boundary notes so they say Harbor is optional and only required for live Harbor rollout execution, not for core import, task metadata helpers, outcome decoding, reward helpers, or deterministic tests.

**Step 4: Make the missing-Harbor error point to the extra**

In `sdk/python/src/leaven/x/harbor/rollout.py`, change the `HarborAdapterError` in `_run_live_harbor_trial` to include the operator command:

```python
"Harbor is required to execute live Harbor rollouts; install with "
"`pip install 'leaven[harbor]'` or pass `trial_runner=` for deterministic no-spend tests"
```

Keep the import lazy inside `_run_live_harbor_trial`.

**Step 5: Run the dependency-boundary tests**

Run:

```bash
uv run --project sdk/python pytest sdk/python/tests/x/test_harbor.py::test_import_leaven_does_not_import_harbor_dependency sdk/python/tests/x/test_harbor.py::test_live_rollout_missing_harbor_error_is_actionable -q
```

Expected: both pass.

**Step 6: Commit**

```bash
git add sdk/python/pyproject.toml sdk/python/src/leaven/x/harbor/rollout.py sdk/python/tests/x/test_harbor.py
git commit -m "fix(python): declare Harbor optional adapter extra"
```

## Task 2: Make Terminal-Bench Adapter Reuse Test Run In The Right Environment

**Files:**
- Modify: `sdk/python/examples/codex_terminal_bench/pyproject.toml`
- Modify: `sdk/python/examples/codex_terminal_bench/tests/test_adapter_reuse.py`
- Modify if needed: `sdk/python/examples/codex_terminal_bench/src/codex_terminal_bench/agent.py`

**Step 1: Reproduce the current failure**

Run from repo root:

```bash
PYTHONPATH=sdk/python/examples/codex_terminal_bench/src uv run --project sdk/python pytest sdk/python/examples/codex_terminal_bench/tests/test_adapter_reuse.py -q
```

Expected current failure: `ModuleNotFoundError: No module named 'harbor'`.

**Step 2: Decide the intended verification command**

Prefer running the example test under the example project, because `sdk/python/examples/codex_terminal_bench/pyproject.toml` already declares `harbor==0.13.1`.

Try:

```bash
uv run --project sdk/python/examples/codex_terminal_bench pytest tests/test_adapter_reuse.py -q
```

Expected after environment resolution: the test imports `harbor` and either passes or fails on a real assertion.

If uv cannot resolve the local editable `leaven` source from the example project, update `[tool.uv.sources]` or the command path rather than adding Harbor to the root dev dependency group.

**Step 3: Keep Harbor out of root default tests**

Do not make `sdk/python/tests` require Harbor by default. The root SDK deterministic tests should continue to pass without Harbor installed.

If a root-level collection path accidentally reaches the example test, configure the command/documentation so example tests are run from the example package. Do not hide the example test behind a broad skip unless Harbor truly cannot be installed in local dev.

**Step 4: Strengthen the adapter-reuse assertions**

In `sdk/python/examples/codex_terminal_bench/tests/test_adapter_reuse.py`, keep assertions that prove:

```python
assert trial.HarborTrialOutcome is lv.x.harbor.HarborTrialOutcome
assert scenario.decode_outcome.__func__ is lv.x.harbor.HarborTrialOutcome.decode.__func__
assert scenario.trajectory_excerpt is lv.x.harbor.trajectory_excerpt
assert scenario.verifier.id == "leaven.x.harbor.rewards.reward"
assert scenario.ctrf.id == "leaven.x.harbor.rewards.ctrf_fraction"
assert isinstance(agent.LeavenCodex(...), lv.x.harbor.LeavenCodex)
```

If `agent.LeavenCodex` import still forces Harbor at root import time, leave that as example-local behavior only; the root package boundary is `import leaven`, not importing the live-spend example package.

**Step 5: Run the example test**

Run:

```bash
uv run --project sdk/python/examples/codex_terminal_bench pytest tests/test_adapter_reuse.py -q
```

Expected: pass.

**Step 6: Commit**

```bash
git add sdk/python/examples/codex_terminal_bench/pyproject.toml sdk/python/examples/codex_terminal_bench/tests/test_adapter_reuse.py sdk/python/examples/codex_terminal_bench/src/codex_terminal_bench/agent.py
git commit -m "test(python): verify Terminal-Bench reuses Harbor adapter"
```

Only include files that actually changed.

## Task 3: Close `import_trial_result` Beyond The Placeholder Path

**Files:**
- Modify: `sdk/python/src/leaven/x/harbor/__init__.py`
- Modify: `sdk/python/src/leaven/x/harbor/rollout.py`
- Test: `sdk/python/tests/x/test_harbor.py`

**Step 1: Write failing import tests**

Add deterministic tests covering:

```python
def test_import_trial_result_reads_adapter_outcome_file(tmp_path: Path) -> None:
    trial_dir = tmp_path / "trial"
    trial_dir.mkdir()
    (trial_dir / "leaven_outcome.json").write_text(
        lv.x.harbor.HarborTrialOutcome(rewards={"reward": 1.0}).encode(),
        encoding="utf-8",
    )
    assert lv.x.harbor.import_trial_result(trial_dir).rewards["reward"] == 1.0


def test_import_trial_result_can_reconstruct_from_harbor_artifacts(tmp_path: Path) -> None:
    # create verifier/ctrf.json and a minimal fake result/evidence file shape if the adapter supports it
    # expected: rewards, CTRF, trajectory ref, and exception/cost fields degrade cleanly
```

Before writing the second test, inspect real Harbor trial output paths from the existing Terminal-Bench code and only target files the repo already knows how to read, such as `verifier/ctrf.json` and `agent/trajectory.json`.

**Step 2: Run the tests and confirm the gap**

Run:

```bash
uv run --project sdk/python pytest sdk/python/tests/x/test_harbor.py::test_import_trial_result_reads_adapter_outcome_file sdk/python/tests/x/test_harbor.py::test_import_trial_result_can_reconstruct_from_harbor_artifacts -q
```

Expected before implementation: first may pass, second fails because `import_trial_result` only accepts `leaven_outcome.json`.

**Step 3: Implement the minimal importer**

Keep `leaven_outcome.json` as the preferred fast path.

For raw Harbor trial directories, implement only the evidence shapes the spec requires and this repo can prove without live Harbor:

- `verifier/ctrf.json` -> `CtrfEvidence`
- `agent/trajectory.json` -> `trajectory_path` when present
- optional verifier/reward JSON only if there is a stable Harbor file in the existing fixture or docs
- missing optional files degrade to `None`/empty evidence, not crash

If there is no stable reward file path available, record that limitation in the error message and do not fabricate rewards.

**Step 4: Run the importer tests**

Run:

```bash
uv run --project sdk/python pytest sdk/python/tests/x/test_harbor.py -q
```

Expected: all Harbor adapter tests pass.

**Step 5: Commit**

```bash
git add sdk/python/src/leaven/x/harbor/__init__.py sdk/python/src/leaven/x/harbor/rollout.py sdk/python/tests/x/test_harbor.py
git commit -m "feat(python): import deterministic Harbor trial evidence"
```

## Task 4: Run The Deterministic Closeout Gates

**Files:**
- No source edits expected unless a gate fails for a real bug.

**Step 1: Run root Harbor adapter tests**

Run:

```bash
uv run --project sdk/python pytest sdk/python/tests/x/test_harbor.py -q
```

Expected: pass.

**Step 2: Run root SDK tests**

Run:

```bash
uv run --project sdk/python pytest sdk/python/tests -q
```

Expected: pass. If unrelated existing failures appear, capture exact failures and classify them before changing adapter code.

**Step 3: Run Terminal-Bench deterministic tests from the example project**

Run:

```bash
uv run --project sdk/python/examples/codex_terminal_bench pytest tests -q
```

Expected: pass without Docker/Codex/live model calls.

If this command attempts live Docker/Codex, stop and narrow to deterministic tests:

```bash
uv run --project sdk/python/examples/codex_terminal_bench pytest \
  tests/test_adapter_reuse.py \
  tests/test_kit_optimization_mechanics.py \
  tests/test_scenario.py \
  tests/test_trial.py \
  -q
```

Expected: pass.

**Step 4: Confirm no live spend knobs were required**

Check command environment and test output. No successful closeout command should require:

- Docker
- `OPENAI_API_KEY`
- live Codex
- networked model calls
- `gpt-5.4-mini`

**Step 5: Commit any fixes from failed deterministic gates**

Use focused commits. Example:

```bash
git add <files>
git commit -m "fix(python): keep Harbor adapter tests deterministic"
```

Skip this step if no files changed.

## Task 5: Update The Goal Handoff With Evidence

**Files:**
- Modify: `docs/working-memory/harbor-leaven-adapter-goal-handoff.yaml`

**Step 1: Record evidence per acceptance row**

For each required acceptance row, update:

```yaml
status: proven
evidence:
  - command: ...
    result: ...
    commit: ...
```

Use the exact commit hashes and exact commands from Task 4. Do not mark a row proven from vibes.

**Step 2: Keep optional live smoke optional**

Under `proof_denominator.optional`, leave live Harbor/Codex smoke as optional unless Darin explicitly approves running it.

**Step 3: Parse the YAML**

Run:

```bash
ruby -e 'require "yaml"; YAML.load_file("docs/working-memory/harbor-leaven-adapter-goal-handoff.yaml"); puts "yaml ok"'
```

Expected: `yaml ok`.

**Step 4: Commit the evidence update**

```bash
git add docs/working-memory/harbor-leaven-adapter-goal-handoff.yaml
git commit -m "docs: record Harbor adapter proof evidence"
```

## Final Verification

Run these before claiming the goal is complete:

```bash
uv run --project sdk/python pytest sdk/python/tests/x/test_harbor.py -q
uv run --project sdk/python pytest sdk/python/tests -q
uv run --project sdk/python/examples/codex_terminal_bench pytest tests -q
ruby -e 'require "yaml"; YAML.load_file("docs/working-memory/harbor-leaven-adapter-goal-handoff.yaml"); puts "yaml ok"'
git status --short --branch
```

Expected:

- Harbor adapter tests pass.
- Root SDK tests pass, or any unrelated failures are explicitly classified and not used as Harbor proof.
- Terminal-Bench deterministic example tests pass without live Docker/Codex/model spend.
- YAML parses.
- Worktree is clean.

## Execution Choice

Plan complete and saved to `docs/plans/2026-06-22-harbor-leaven-adapter-closeout.md`.

Two execution options:

1. **Subagent-Driven (this session)** - dispatch fresh subagent per task, review between tasks, fast iteration.
2. **Parallel Session (separate)** - open a new session with `superpowers:executing-plans`, batch execution with checkpoints.

