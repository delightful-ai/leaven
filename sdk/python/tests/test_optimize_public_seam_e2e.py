from __future__ import annotations

import shlex
from pathlib import Path

import leaven as lv
from leaven._seam.resolve import resolve_leaven_binary


@lv.runner
async def run_with_lm(
    prompt: lv.PromptArtifact,
    case: lv.Case,
    cx: lv.RolloutContext,
) -> str:
    reply = await cx.lm.complete(prompt=prompt.template.format(**case.input), max_tokens=12)
    return reply.text


@lv.reward(id="exact")
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    return 1.0 if output == (case.target or {})["answer"] else 0.0


async def test_optimize_run_spawns_public_seam_and_returns_inspectable_receipts(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Scenario: SDK optimize drives the real public seam and returns audit facts."""

    log_path = tmp_path / "leaven-bin-argv.log"
    wrapper = _leaven_bin_wrapper(tmp_path, resolve_leaven_binary(), log_path)
    monkeypatch.setenv("LEAVEN_BIN", str(wrapper))
    monkeypatch.chdir(tmp_path)

    result = await lv.optimize(
        seed=lv.PromptArtifact(template="{question}"),
        environment=lv.Environment(
            task=lv.Task(
                name="public-seam-e2e",
                cases=[
                    lv.Case(
                        id="public-seam-001",
                        input={"question": "say seam-ok"},
                        target={"answer": "seam-ok"},
                        split="test",
                    )
                ],
            ),
            rollout=lv.Rollout.fn(run_with_lm),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(population_size=1),
        runtime=lv.runtime(
            workspace=lv.workspace.local(),
            lm=lv.lm.mock(responses=["seam-ok"]),
            trust_profile=lv.TrustProfile.TRUSTED_LOCAL_OPERATOR,
            budget=lv.budget(usd=1),
        ),
    ).run()

    argv_lines = log_path.read_text(encoding="utf-8").splitlines()
    assert len(argv_lines) == 1
    assert argv_lines[0].startswith(
        f"seam serve --stdio --root {Path(__file__).resolve().parents[3]} --config "
    )

    assessment = result.assessment("case_public_seam_001")
    assert result.best.summary_score == 1.0
    assert assessment.score.value == 1.0
    assert assessment.evidence.public is not None
    assert assessment.evidence.public.payload == {"output": "seam-ok", "reward_count": 1}
    assert [receipt.receipt_id for receipt in assessment.effect_receipts] == ["lmrec_completion"]
    assert result.summary.total_lm_tokens == 2

    inspection = lv.runs.inspect(result.summary.run_dir or "")
    assert inspection.best_lineage == ["cand_seed"]
    assert inspection.receipt_ids(kind="call") == ["lmrec_completion"]
    assert inspection.evidence[0].payload == {"output": "seam-ok", "reward_count": 1}
    assert inspection.total_lm_tokens == 2
    assert [fact.surface for fact in inspection.unsupported] == ["run.inspection"]


def _leaven_bin_wrapper(tmp_path: Path, real_bin: Path, log_path: Path) -> Path:
    wrapper = tmp_path / "leaven-wrapper"
    wrapper.write_text(
        "\n".join(
            [
                "#!/bin/sh",
                f"printf '%s\\n' \"$*\" >> {shlex.quote(str(log_path))}",
                f"exec {shlex.quote(str(real_bin))} \"$@\"",
                "",
            ]
        ),
        encoding="utf-8",
    )
    wrapper.chmod(0o755)
    return wrapper
