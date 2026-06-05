"""Example 05 — ADVANCED SEAM: rich evaluator body (judge call + batched ops + evidence).

ADVANCED / seam-only path. Ordinary scoring is a `Rubric` of `@lv.reward`
functions (see examples 03 and 04) — that is what you reach for first. Write an
`@lv.evaluator` ONLY when the rubric isn't enough: when you need to inspect
cases, materialize workspaces, fan out multiple typed effects in one
round-trip, and submit hand-authored assessments with public/private evidence.

This example shows the full inside of an evaluator body, mirroring the
locked-spec sketch at
`docs/specs/public-seam-v1/examples/evaluator_dspy_codex.v0.3.py` but
trimmed for clarity. It's the evaluator shape EvoSkill-class repros need when a
plain rubric can't express the scoring.
"""

from pydantic import BaseModel, Field

import leaven as lv
from leaven.assessment import AssessmentWrite
from leaven.builders.assessments import AssessmentSubmission
from leaven.evaluation_job import EvaluationJob
from leaven.evidence import EvidenceEnvelope
from leaven.json_value import JsonValue


class JudgeResult(BaseModel):
    score: float = Field(ge=0.0, le=1.0)
    feedback: str
    verdict: str


@lv.evaluator(
    id="examples/rich-evaluator",
    trust_profile=lv.TrustProfile.MANAGED_SANDBOX,
    granularity="per_case",
)
async def evaluate(job: EvaluationJob, cx: lv.EvaluatorContext) -> AssessmentSubmission:
    assessments: list[AssessmentWrite] = []

    for item in job.independent_cases():
        assert item.candidate_id is not None
        case = await cx.case.load(item.case_id, include=("input", "target", "metadata"))
        ws = await cx.workspace.materialize_candidate(
            item.candidate_id,
            surface="full_repo",
            lifetime="stage_call",
        )

        diff = await cx.workspace.git_diff(ws, against="parent")
        tests = await cx.sandbox.exec(
            workspace=ws,
            argv=["pytest", "-q", "tests/", "--json-report"],
            timeout_s=120,
            output=lv.output.files(["report.json"], max_bytes=128_000),
            input_classes=[lv.data_class.CASE_TARGET, lv.data_class.WORKSPACE_FILE],
            forbidden_input_classes=[lv.data_class.WORKSPACE_SECRET],
        )
        judgment = await cx.agent.run(
            workspace=ws,
            instructions=lv.AgentInstructions(
                task=f"Judge the candidate's answer against the rubric.\n"
                f"Question: {case.input['question']}\n"
                f"Rubric: {_target_rubric(case)}",
                system=lv.roles.JUDGE,
            ),
            output=lv.output.json_schema(JudgeResult),
            input_classes=[lv.data_class.CASE_TARGET, lv.data_class.CANDIDATE_OUTPUT],
            forbidden_input_classes=[lv.data_class.WORKSPACE_SECRET],
        )

        parsed = judgment.parsed
        composite = 0.7 * parsed.score + 0.3 * (1.0 if tests.exit_code == 0 else 0.0)

        assessments.append(
            AssessmentWrite.independent_case(
                candidate=item.candidate_id,
                case=item.case_id,
                score=lv.Score(
                    value=composite,
                    feedback=parsed.feedback,
                ),
                evidence=EvidenceEnvelope.public_private(
                    public={
                        "feedback": parsed.feedback,
                        "verdict": parsed.verdict,
                        "data_classes": [lv.data_class.OPTIMIZER_VISIBLE],
                    },
                    private={
                        "git_diff": diff.text,
                        "data_classes": [
                            lv.data_class.CASE_TARGET,
                            lv.data_class.EVALUATOR_PRIVATE,
                        ],
                    },
                    target_derived=True,
                ),
                read_receipts=[diff.receipt],
                effect_receipts=[tests.receipt, judgment.receipt],
                replayability="boundary_managed",
            ),
        )

    return await cx.assessments.submit(job.evaluation_request_id, assessments)


def _target_rubric(case: lv.Case) -> JsonValue:
    if case.target is None or "rubric" not in case.target:
        return "exact match"
    return case.target["rubric"]


def main() -> None:
    print("evaluator decorated:", evaluate)
    print("  role         :", evaluate.role)
    print("  trust_profile:", evaluate.trust_profile)


if __name__ == "__main__":
    main()
