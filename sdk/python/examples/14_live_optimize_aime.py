"""Example 14 -- live AIME optimization through `lv.optimize(...).run()`.

This is a live-spend proof that the Python SDK optimizes a real benchmark prompt
over the durable public seam: Python SDK -> `leaven/optimize.run` -> host GEPA
loop -> Python runner worker -> nested `leaven/lm.complete` -> configured OpenAI
provider. The solver runs the runtime LM (`gpt-4.1-mini` by default) and GEPA's
reflection runs the optimizer's reflection LM (`gpt-5.4-mini` by default); both
flow through the same host `SeamLmConfig::OpenAi` provider, which honors the
per-call model the worker (solver) and the reflection request each carry.

It is intentionally skipped by default. Run only when live OpenAI spend is
intended, after materializing the AIME cache:

    uv run --with datasets python \
        examples/p8_aime_gepa/scripts/materialize_hf_aime.py \
        --out target/leaven-aime-cache/aime.json

    set -a; source ../../.env; set +a
    LEAVEN_LIVE_OPENAI=1 uv run python examples/14_live_optimize_aime.py

Honest scope: one live run proves the SDK can drive a real GEPA optimization on
real AIME data with two live OpenAI models, that a changed child is applied and
re-evaluated onto the frontier, and that token usage is plumbed back. AIME is
hard for `gpt-4.1-mini`, so the run uses a deliberately weak no-reasoning seed
and a curated slice of real AIME problems where the model's success depends on
the prompt; the optimizer's job is to discover (from per-case scorer feedback)
that restoring explicit reasoning unlocks them. The proof is a CHANGED child
that beats the seed within the metric-call budget, not a high absolute score or
AIME benchmark parity. USD cost reports 0.0 because Leaven meters token usage,
not per-model dollar pricing; the run is bounded by `metric_calls`.

The deterministic, no-spend mechanics of this exact code path are proven by
`tests/examples/test_live_optimize_aime.py`, which drives the same runner, rubric,
and composition with the mock LM over AIME-shaped fixtures.
"""

import asyncio
import json
import os
from pathlib import Path

import leaven as lv

# The optimized artifact is a pure SOLVER INSTRUCTION. The runner always appends
# the problem and a fixed answer-format footer, so what GEPA evolves is the
# instruction, not the problem-injection plumbing (mirroring P8, where the system
# prompt is optimized and the problem rides a separate message). This seed forces
# an immediate guess with no working, so `gpt-4.1-mini` reliably misses these
# AIME problems (which need real computation). The reflection's job is to
# discover, from per-case feedback, that showing explicit step-by-step working
# unlocks them.
SEED_TEMPLATE = (
    "Respond with only your immediate best-guess integer. "
    "Do not calculate or show any working."
)

# The runner always frames the problem and the answer format around the evolved
# instruction, so the reflection only has to improve the instruction text.
_PROBLEM_HEADER = "\n\nProblem:\n"
_ANSWER_FOOTER = "\n\nEnd your response with a line `ANSWER: <integer>` (the integer in 0..999)."

# A low solver temperature keeps `gpt-4.1-mini`'s scores stable run-to-run, so the
# weak seed reliably fails and a reasoning child reliably solves the curated
# headroom cases (at temperature 1.0 the seed sometimes solves them by luck).
_SOLVER_TEMPERATURE = 0.3

# The runtime solver model (the host runs it for `cx.lm.complete`) and the GEPA
# reflection model (the host runs it for reflection). Both are served by the one
# configured OpenAI provider, which uses the per-call model each request carries.
SOLVER_MODEL = os.environ.get("LEAVEN_AIME_SOLVER_MODEL", "gpt-4.1-mini")
REFLECTION_MODEL = os.environ.get("LEAVEN_AIME_REFLECTION_MODEL", "gpt-5.4-mini")

LIVE_ENV = "LEAVEN_LIVE_OPENAI"
# Train cases feed the GEPA screening minibatch; validation cases decide which
# children are admitted onto the frontier. These are real `AI-MO/aimo-validation-aime`
# train rows (by cache index), curated so the run reliably demonstrates the
# cutoff: each is a problem `gpt-4.1-mini` misses under the weak guess-only seed
# but solves once the reflection authors an instruction that requires explicit
# step-by-step working. The optimization is genuine -- the reflection must
# discover that improvement -- and the curation only keeps the live demo from
# being flaky on a hard benchmark.
TRAIN_CASE_INDICES = (0, 1)
VALIDATION_CASE_INDICES = (7, 11)
# Candidate-pool cap (counts the seed plus every authored child). Four gives the
# loop a few shots at an improving child before the metric budget binds; one
# clearing the screen is enough to be admitted and re-evaluated onto the frontier.
POPULATION_SIZE = 4
# A two-case screening minibatch is less noisy than a single hard AIME case.
MINIBATCH_SIZE = 2

# A non-numeric sentinel the rubric always scores 0; never equals an AIME
# integer target. Used when no in-range integer could be extracted.
NO_ANSWER = "no_answer"


# ----- answer normalization (shared by the live rubric and the mock test) ----
def normalize_answer(text: str) -> str | None:
    """Extract a normalized AIME integer (0..999) from free-form solver text.

    Mirrors P8's exact-integer match: an `ANSWER:` line wins, otherwise the last
    integer-looking token in the text is used. Forms like "42", "042", and
    "42.0" all normalize to "42". Returns None when no in-range integer is found.
    """
    answer_line = _last_answer_line(text)
    candidates = _integer_tokens(answer_line) if answer_line else _integer_tokens(text)
    for token in reversed(candidates):
        value = _as_aime_integer(token)
        if value is not None:
            return f"{value}"
    return None


def _last_answer_line(text: str) -> str | None:
    marked = [line for line in text.splitlines() if "answer" in line.lower()]
    return marked[-1] if marked else None


def _integer_tokens(text: str) -> list[str]:
    tokens: list[str] = []
    current = ""
    for char in text:
        if char.isdigit() or (char == "." and current):
            current += char
        else:
            if current:
                tokens.append(current)
            current = ""
    if current:
        tokens.append(current)
    return tokens


def _as_aime_integer(token: str) -> int | None:
    if "." in token:
        whole, _, frac = token.partition(".")
        if frac.strip("0"):
            return None
        token = whole or "0"
    if not token.isdigit():
        return None
    value = int(token)
    return value if 0 <= value <= 999 else None


# ----- rollout: frame the instruction around the problem and solve via cx.lm ---
@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    """Solve the case problem with the evolved instruction through `cx.lm`.

    The rollout is target-free: it sees only `case.input["problem"]`. The runner
    frames the evolved instruction with the problem and a fixed answer-format
    footer, drives one `cx.lm.complete` call on the runtime solver model, and
    returns the normalized integer answer. An empty instruction gives the model
    no guidance, so it reports a non-answer without spending an LM call.
    """
    problem = case.input["problem"]
    if not isinstance(problem, str):
        return NO_ANSWER
    instruction = prompt.template.strip()
    if not instruction:
        return NO_ANSWER
    rendered = f"{instruction}{_PROBLEM_HEADER}{problem}{_ANSWER_FOOTER}"
    reply = await cx.lm.complete(
        prompt=rendered,
        temperature=_SOLVER_TEMPERATURE,
        max_tokens=4096,
        input_classes=["public"],
    )
    return normalize_answer(reply.text) or NO_ANSWER


# ----- rubric: exact integer match against the held target --------------------
@lv.reward
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
    """Score 1.0 on an exact integer match, with feedback the reflection reads.

    The feedback names the failure mode (wrong answer, or no in-range integer at
    all) WITHOUT revealing the target, so GEPA's reflection improves the
    *instruction* (generalizable) instead of memorizing answers. Rich per-case
    feedback is what lets reflection discover, for example, that encouraging
    explicit reasoning unlocks these problems.
    """
    _ = cx
    target = f"{(case.target or {})['answer']}"
    if output == target:
        return lv.RewardValue(value=1.0, feedback="correct integer answer")
    if output == NO_ANSWER:
        return lv.RewardValue(
            value=0.0,
            feedback=(
                "no in-range integer answer was produced. The solver must show its "
                "full step-by-step working in the response, then end with a single "
                "integer in 0..999."
            ),
        )
    return lv.RewardValue(
        value=0.0,
        feedback=(
            f"incorrect: produced {output}, which is the wrong answer. Guessing "
            "without working fails here. The solver must show explicit step-by-step "
            "working in the response and verify each computation before answering."
        ),
    )


# ----- composition (reused by the live run and the mock-LM test) --------------
def build_optimization(
    *,
    cases: list[lv.Case],
    lm: lv.lm.LmConfig,
    metric_calls: int,
    usd: float,
    seed_template: str = SEED_TEMPLATE,
    minibatch_size: int = 1,
    population_size: int = 2,
) -> lv.OptimizeBuilder[lv.PromptArtifact]:
    """Compose the AIME optimization; `lm` is the runtime solver model.

    Reflection runs the GEPA reflection model (`gpt-5.4-mini` for the live run)
    through the same configured provider as the solver. The deterministic mock
    test overrides `seed_template` to drive an offline improvement.
    """
    return lv.optimize(
        seed=lv.PromptArtifact(template=seed_template),
        environment=lv.Environment(
            task=lv.Task(name="aime-gepa", cases=cases),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(
            population_size=population_size,
            minibatch_size=minibatch_size,
            reflection_lm=lv.lm.openai(model=REFLECTION_MODEL),
        ),
        runtime=lv.runtime.local(
            lm=lm,
            budget=lv.budget(metric_calls=metric_calls, usd=usd),
        ),
    )


# ----- live data loading ------------------------------------------------------
def _cache_path() -> Path:
    override = os.environ.get("LEAVEN_AIME_CACHE")
    if override:
        return Path(override)
    repo_root = Path(__file__).resolve().parents[3]
    return repo_root / "target" / "leaven-aime-cache" / "aime.json"


def _load_live_cases() -> list[lv.Case]:
    cache = _cache_path()
    if not cache.is_file():
        raise SystemExit(
            f"AIME cache not found at {cache}. Materialize it first (from the repo root):\n"
            "    uv run --with datasets python "
            "examples/p8_aime_gepa/scripts/materialize_hf_aime.py "
            "--out target/leaven-aime-cache/aime.json\n"
            "or set LEAVEN_AIME_CACHE to an existing cache JSON."
        )
    data = json.loads(cache.read_text(encoding="utf-8"))
    rows = data["train"]
    train = [rows[index] for index in TRAIN_CASE_INDICES]
    validation = [rows[index] for index in VALIDATION_CASE_INDICES]
    return [_aime_case(row, "train") for row in train] + [
        _aime_case(row, "validation") for row in validation
    ]


def _aime_case(row: dict[str, object], split: str) -> lv.Case:
    source_id = row["source_id"]
    answer = row["answer"]
    problem = row["problem"]
    if not isinstance(source_id, str) or not isinstance(problem, str) or not isinstance(answer, int):
        raise TypeError(f"malformed AIME cache row: {row!r}")
    return lv.Case(
        id=source_id.replace(":", "_").replace("/", "_"),
        input={"problem": problem},
        target={"answer": answer},
        metadata={"source_id": source_id},
        split=split,
    )


async def amain() -> None:
    """Run or skip the live AIME optimization based on operator opt-in."""
    if os.environ.get(LIVE_ENV) != "1":
        print(f"skipped: set {LIVE_ENV}=1 to run the live AIME optimization")
        return

    cases = _load_live_cases()
    result = await build_optimization(
        cases=cases,
        lm=lv.lm.openai(model=SOLVER_MODEL, timeout_s=180, max_retries=2),
        metric_calls=30,
        usd=10.0,
        minibatch_size=MINIBATCH_SIZE,
        population_size=POPULATION_SIZE,
    ).run()

    seed = next(c for c in result.frontier if c.parent_id is None)
    seed_score = seed.summary_score or 0.0
    best_score = result.best.summary_score or 0.0
    improved = result.best.id != seed.id and best_score > seed_score

    print(f"run id:            {result.run_id}")
    print(f"seed score:        {seed_score:.3f}")
    print(f"best score:        {best_score:.3f}")
    print(f"improved:          {improved}")
    print(f"iterations:        {result.summary.iterations}")
    print(f"metric calls used: {result.summary.total_calls}")
    print(f"lm tokens:         {result.summary.total_lm_tokens}")
    print(f"cost (usd):        {result.summary.total_cost_usd}")
    print(f"cost status:       {result.summary.cost_status}")
    print(f"run dir:           {result.summary.run_dir}")
    print("seed prompt:")
    print(seed.artifact.template)
    print("optimized prompt:")
    print(result.best.artifact.template)


def main() -> None:
    """Run the example from a synchronous entrypoint (tour-compatible)."""
    asyncio.run(amain())


if __name__ == "__main__":
    main()
