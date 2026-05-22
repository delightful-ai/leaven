import dspy
import leaven as lv
from pydantic import BaseModel, Field

class JudgeResult(BaseModel):
    score: float = Field(ge=0.0, le=1.0)
    feedback: str
    verdict: str

class SkillJudge(dspy.Module):
    def __init__(self):
        super().__init__()
        self.grade = dspy.ChainOfThought(
            "task, target, pytest_report, workspace_diff, agent_report -> score: float, feedback: str, verdict: str"
        )

    def forward(self, *, task, target, pytest_report, workspace_diff, agent_report):
        return self.grade(task=task, target=target, pytest_report=pytest_report, workspace_diff=workspace_diff, agent_report=agent_report)

@lv.evaluator(id="skillbank/pytest-dspy-codex", trust_profile="managed_sandbox", granularity="per_case")
async def evaluate(job: lv.EvaluationJob, cx: lv.EvalContext):
    judge = SkillJudge()
    assessments = []
    with lv.dspy_context(cx, model_role="grader", strict=True):
        for item in job.independent_cases():
            case = await cx.case.load(item.case_id, include=["input", "target", "metadata"])
            ws = await cx.workspace.materialize_candidate(item.candidate_id, surface="full_repo", lifetime="stage_call")
            async with cx.batch() as b:
                diff = b.workspace.git_diff(ws, against="parent", expected_data_classes=["workspace.file"])
                status = b.workspace.git_status(ws)
                tests = b.sandbox.exec(
                    workspace=ws,
                    argv=["pytest", "-q", "tests/hidden", "--json-report"],
                    timeout_s=180,
                    output=lv.output.files(["report.json", "pytest.log"], max_bytes=256_000),
                    input_classes=["case.target", "workspace.file"],
                    forbidden_input_classes=["workspace.secret"],
                )
                agent = b.agent.run(
                    runtime="codex-app-server",
                    workspace=ws,
                    instructions=lv.AgentInstructions(task=f"Task: {case.input['task']}
Rubric: {case.target['rubric']}"),
                    output=lv.output.json_schema(JudgeResult),
                    input_classes=["case.input", "case.target", "workspace.file"],
                    forbidden_input_classes=["workspace.secret"],
                )
            diff, status, tests, agent = await b.run()
            report = lv.pytest.parse_json_report(tests.files["report.json"])
            with lv.dspy_call_context(input_classes=["case.input", "case.target", "candidate.output", "workspace.file"], forbidden_input_classes=["workspace.secret"]):
                pred = await lv.dspy_acall(judge, task=case.input["task"], target=case.target["rubric"], pytest_report=report.summary, workspace_diff=diff.text, agent_report=agent.parsed["feedback"])
            judged = JudgeResult.model_validate(pred.to_dict())
            output = lv.OutputRecord.text(summary=judged.feedback, visibility="optimizer_visible", data_classes=["optimizer.visible", "candidate.output"])
            assessments.append(lv.AssessmentWrite.independent_case(
                candidate=item.candidate_id,
                case=item.case_id,
                score=lv.Score(value=0.6 * report.pass_rate + 0.4 * judged.score, output=output, metrics={"pytest_pass_rate": report.pass_rate, "judge_score": judged.score}),
                evidence=lv.EvidenceEnvelope.public_private(
                    public={"feedback": judged.feedback, "metrics": {"pytest_pass_rate": report.pass_rate, "judge_score": judged.score}, "data_classes": ["optimizer.visible"]},
                    private={"visibility": "evaluator_only", "payload": {"target_ref": case.target_ref, "git_status": status.entries}, "data_classes": ["case.target", "evaluator.private"]},
                    target_derived=True,
                ),
                read_receipts=[case.receipt, diff.receipt, status.receipt],
                effect_receipts=[tests.receipt, agent.receipt, pred.leaven_lm_receipt],
                replayability="boundary_managed",
            ))
    return await cx.assessments.submit(job.evaluation_request_id, assessments)
