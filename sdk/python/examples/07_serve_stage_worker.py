"""Example 07 — standalone Python worker declaration shape.

A decorated stage (here a `@lv.judge`) can compose into an in-process
`lv.optimize(...)` call OR, in a later slice, run as a standalone Python script
the engine reaches over Leaven-owned stdio JSON-RPC. The decorator + function
shape is intended to stay identical in both cases.

Standalone usage looks like:

    if __name__ == "__main__":
        lv.serve_stage(my_judge)

The standalone loop is not implemented yet. `lv.serve_stage(...)` currently
raises an explicit scaffold error; when implemented, it must use the current
`_seam_worker` / `_stage_runtime` route, not the removed legacy `_serve`
bridge-demo module.

This pattern is how third-party judges / reflectors / proposers ship — one
script, one decorator, one `serve_stage` call.
"""

from pydantic import BaseModel, Field

import leaven as lv
from leaven.assessment import AssessmentWrite
from leaven.evidence import EvidenceEnvelope
from leaven.json_value import JsonValue
from leaven.stage_payloads import JudgeRequest


class JudgeOutcome(BaseModel):
    score: float = Field(ge=0.0, le=1.0)
    feedback: str


@lv.judge(
    stage_id="examples/llm-pairwise-judge",
    trust_profile=lv.TrustProfile.PACKAGE_SCORER,
)
async def judge(req: JudgeRequest, cx: lv.JudgeContext) -> AssessmentWrite:
    case = await cx.case.load(req.case_id, include=("input", "target"))

    response = await cx.lm.complete(
        messages=[
            {
                "role": "system",
                "content": (
                    "You are a pairwise judge. Prefer the candidate whose answer "
                    "is correct, or — if both are correct — clearer."
                ),
            },
            {
                "role": "user",
                "content": (
                    f"Question: {case.input['question']}\n"
                    f"Rubric  : {_target_rubric(case)}\n"
                    f"Candidates ({req.kind}): {', '.join(req.candidates)}"
                ),
            },
        ],
        response_format=lv.output.json_schema(JudgeOutcome),
        model_role="judge",
    )
    if not isinstance(response.parsed, JudgeOutcome):
        raise TypeError("judge response did not match JudgeOutcome")
    outcome = response.parsed

    return AssessmentWrite.pairwise(
        candidates=req.candidates,
        case=req.case_id,
        preference=req.candidates[0],  # judge picks; demo uses first
        evidence=EvidenceEnvelope.public_only(
            payload={"feedback": outcome.feedback, "judge_score": outcome.score},
            data_classes=[lv.data_class.OPTIMIZER_VISIBLE],
        ),
        effect_receipts=[response.receipt],
        replayability="boundary_managed",
    )


def _target_rubric(case: lv.Case) -> JsonValue:
    if case.target is None or "rubric" not in case.target:
        return "exact match"
    return case.target["rubric"]


if __name__ == "__main__":
    # Engine reaches this binary over ACP stdio; serve_stage handles the loop.
    try:
        lv.serve_stage(judge)
    except NotImplementedError as e:
        print(f"(expected) serve_stage scaffold: {e}")
