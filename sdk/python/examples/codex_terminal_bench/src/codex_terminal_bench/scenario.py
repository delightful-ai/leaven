"""The optimize composition: rollout (one Harbor Trial) + rubric + seed kit.

The optimizer evolves a Codex agent kit on ONE pinned Terminal-Bench-2 task. The
rollout runs a single Harbor Trial of the current kit and serializes the rollout
evidence (verifier reward, CTRF fraction, token/cost totals, trajectory excerpt)
into its text output; the rubric scores that evidence and feeds informative
per-case feedback back to GEPA's agentic reflection so the next kit is better.

Rubric: the verifier reward passes through at weight 1 (the task's own
pass/fail), plus the CTRF passed/total fraction at weight 0.25 for partial-credit
gradient. Feedback carries the verifier output plus short excerpts of the
*agent's own behavior* from the trajectory (never the task's solution or test
internals — the TB2 canary requirement), which is what an agent kit can act on.
"""

import hashlib
import os
import tempfile
from pathlib import Path

import leaven as lv
from leaven.x.harbor import HarborTrialOutcome, trajectory_excerpt

# Absolute imports (not relative): the optimize worker loads this module's file
# standalone via `runpy.run_path`, where relative imports have no parent package.
# The package is editable-installed, so absolute imports resolve.
from codex_terminal_bench.kit import materialize_kit
from codex_terminal_bench.trial import TrialOutcome, TrialPlan, run_trial
from codex_terminal_bench.wire import RolloutOutcome

# The case-input key naming the pinned Terminal-Bench-2 task this rollout runs.
TASK_INPUT_KEY = "task"
# The runtime LM model. The optimize path requires a runtime LM for the worker's
# `cx.lm` channel even when the rollout does not call it; this rollout drives a
# Harbor Trial instead, so the runtime LM is a formal placeholder.
RUNTIME_LM_MODEL = "gpt-4.1-mini"
# Reflection model the host's agentic Codex reflector runs to author the next kit.
REFLECTION_MODEL = "gpt-5.4-mini"

# Rubric weights: the verifier reward is the task's own pass/fail; the CTRF
# fraction adds a partial-credit gradient so reflection sees progress before a
# full pass.
REWARD_WEIGHT = 1.0
CTRF_WEIGHT = 0.25

LIVE_ENV = "LEAVEN_CODEX_LIVE"
_RUNS_ROOT_ENV = "LEAVEN_CODEX_TB_TRIALS_DIR"
_TRIAL_NAME_MAX_LEN = 96
_TRIAL_NAME_HASH_LEN = 12


def _trials_root() -> Path:
    override = os.environ.get(_RUNS_ROOT_ENV)
    if override:
        return Path(override)
    return Path(".leaven") / "codex-tb-trials"


def _openai_api_key() -> str:
    key = os.environ.get("OPENAI_API_KEY", "")
    if not key:
        raise SystemExit(
            "OPENAI_API_KEY is not set; export it (e.g. `set -a; source ../../.env; set +a`) "
            "so the in-container Codex agent can authenticate."
        )
    return key


# ----- rollout: run ONE Harbor Trial of the current kit -----------------------
@lv.runner
async def run(kit: lv.AgentKitArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    """Run one Harbor Trial of the current kit and serialize the rollout evidence.

    The rollout is target-free: it sees only the case input naming the pinned
    task. It materializes the kit to a temp directory, runs one Harbor Trial of
    the pinned Terminal-Bench-2 task with the `LeavenCodex` agent (which uploads
    the kit into the task working directory), and returns the trial outcome as a
    JSON string the rubric parses.
    """
    _ = cx
    if TASK_INPUT_KEY not in case.input:
        raise TypeError(f"case input must carry a `{TASK_INPUT_KEY}` task path")
    task_path = case.input[TASK_INPUT_KEY]
    if not isinstance(task_path, str):
        raise TypeError(f"case input `{TASK_INPUT_KEY}` must be a string task path")

    trials_root = _trials_root()
    trials_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="leaven-codex-kit-", dir=trials_root) as kit_tmp:
        kit_dir = materialize_kit(kit, Path(kit_tmp) / "kit")
        trial_name = _trial_name(case.id, kit.candidate_id)
        outcome = await run_trial(
            TrialPlan(
                agent_kit_dir=kit_dir,
                trials_dir=trials_root,
                trial_name=trial_name,
                task_path=task_path,
                openai_api_key=_openai_api_key(),
            )
        )
    return _encode_outcome(outcome)


def _trial_name(case_id: str, candidate_id: str | None) -> str:
    """Build a unique trial dir name per (case, candidate) so trials never collide."""
    candidate = candidate_id or "seed"
    raw = f"{case_id}__{candidate}"
    safe = "".join(ch if ch.isalnum() or ch in "-_" else "_" for ch in raw)
    digest = hashlib.sha256(f"{case_id}\0{candidate}".encode()).hexdigest()
    suffix = f"__{digest[:_TRIAL_NAME_HASH_LEN]}"
    return f"{safe[: _TRIAL_NAME_MAX_LEN - len(suffix)]}{suffix}"


decode_outcome = HarborTrialOutcome.decode


def _encode_outcome(outcome: TrialOutcome) -> str:
    return outcome.encode()


# ----- rubric: verifier reward (w=1) + CTRF fraction (w=0.25) ------------------
verifier = lv.x.harbor.rewards.map_key("reward", weight=REWARD_WEIGHT)
ctrf = lv.x.harbor.rewards.ctrf_fraction(weight=CTRF_WEIGHT)


def _verifier_feedback(parsed: HarborTrialOutcome | RolloutOutcome) -> str:
    lines = [parsed.verifier_output.strip()]
    excerpt = trajectory_excerpt(parsed.trajectory_path)
    if excerpt:
        lines.append("Recent agent actions on this task:")
        lines.append(excerpt)
    lines.append(
        "Improve the agent kit (AGENTS.md + skills) so the agent works this task "
        "more effectively. Do not encode the task's specific answer; teach a "
        "general working method."
    )
    return "\n".join(line for line in lines if line)


_trajectory_excerpt = trajectory_excerpt


# ----- composition (reused by the live run and the mock test) -----------------
def build_optimization(
    *,
    cases: list[lv.Case],
    codex_bin_env: str = "LEAVEN_CODEX_BIN",
    metric_calls: int = 8,
    minibatch_size: int = 1,
    population_size: int = 2,
) -> lv.OptimizeBuilder[lv.AgentKitArtifact]:
    """Compose the Codex agent-kit optimization on the pinned task.

    The seed kit is deliberately weak-but-honest (a terse generic AGENTS.md with
    no skills) so the optimizer has real headroom; the host's agentic Codex
    reflector evolves it from the rollout feedback.
    """
    return lv.optimize(
        seed=_seed_kit(),
        environment=lv.Environment(
            task=lv.Task(name="codex-terminal-bench", cases=cases),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([verifier, ctrf]),
        ),
        optimizer=lv.optimizers.gepa(
            population_size=population_size,
            minibatch_size=minibatch_size,
            reflection_agent=lv.agent.codex(
                model=REFLECTION_MODEL,
                transport="cli",
                bin_path_env=codex_bin_env,
            ),
        ),
        runtime=lv.runtime.local(
            lm=lv.lm.openai(model=RUNTIME_LM_MODEL),
            budget=lv.budget(metric_calls=metric_calls),
        ),
    )


def _seed_kit() -> lv.AgentKitArtifact:
    """A deliberately weak-but-honest seed kit: rush-the-first-idea guidance, no skills.

    The instruction is honest (an agent really would follow it) but unhelpful for
    a hard, edge-case-heavy task: it pushes the agent to commit the first
    plausible answer fast and skip checking the cases the task warns about. A
    strong model still flounders on Terminal-Bench-2 `regex-log` under it (the
    task explicitly calls out lookalike dates/IPs, no leading zeros, and matching
    only the last date per line), leaving real headroom for the host's agentic
    reflection to author a kit that works the task carefully.
    """
    return lv.AgentKitArtifact(
        system_prompt=(
            "You are a command-line agent. Move fast: write the first solution that "
            "looks plausible and finish immediately. Do not spend time exploring the "
            "task's inputs, enumerating edge cases, or testing your answer before you "
            "submit it."
        ),
        skills=[],
    )


def pinned_task_case(*, task_path: str = "regex-log", split: str = "train") -> lv.Case:
    """One optimize case naming the pinned task on a given split.

    The case carries no target: the Terminal-Bench-2 verifier is the judge, so
    there is no held answer to score against. The rubric scores the rollout
    output (the trial's verifier reward and CTRF fraction) instead.
    """
    suffix = "" if split == "train" else f"_{split}"
    return lv.Case(
        id=f"tb_{task_path.replace('-', '_')}{suffix}",
        input={TASK_INPUT_KEY: task_path},
        target=None,
        metadata={"benchmark": "terminal-bench-2"},
        split=split,
    )


def pinned_task_cases(*, task_path: str = "regex-log") -> list[lv.Case]:
    """The n=1 task as both a train-screening and a validation-admission case.

    GEPA screens a candidate on the train minibatch and admits it on the
    validation set, so the single pinned task is supplied on both splits. This is
    still one task (n=1); it is the screening case and the admission case so a
    candidate that improves the task is both screened and admitted on it.
    """
    return [
        pinned_task_case(task_path=task_path, split="train"),
        pinned_task_case(task_path=task_path, split="validation"),
    ]


__all__ = [
    "CTRF_WEIGHT",
    "LIVE_ENV",
    "REFLECTION_MODEL",
    "REWARD_WEIGHT",
    "RUNTIME_LM_MODEL",
    "TASK_INPUT_KEY",
    "build_optimization",
    "ctrf",
    "pinned_task_case",
    "pinned_task_cases",
    "run",
    "verifier",
]
