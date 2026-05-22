import asyncio
import dspy
import leaven as lv
from pydantic import BaseModel, Field

class JudgeResult(BaseModel):
    score: float = Field(ge=0.0, le=1.0)
    verdict: str
    feedback: str
    risk_notes: list[str] = []

class SkillJudge(dspy.Module):
    def __init__(self):
        super().__init__()
        self.grade = dspy.ChainOfThought(
            "task, target, pytest_report, workspace_diff, agent_report -> "
            "score: float, verdict: str, feedback: str, risk_notes: list[str]"
        )

    def forward(self, *, task, target, pytest_report, workspace_diff, agent_report):
        return self.grade(
            task=task,
            target=target,
            pytest_report=pytest_report,
            workspace_diff=workspace_diff,
            agent_report=agent_report,
        )

@lv.evaluator(id="skillbank/pytest-dspy-codex", trust_profile="managed_sandbox", granularity="per_case")
async def evaluate(job: lv.EvaluationJob, cx: lv.EvalContext) -> lv.AssessmentBatchReceipt:
    judge = SkillJudge()
    assessments = []

    with lv.dspy_context(
        cx,
        model_role="grader",
        allow_input_classes=["case.input", "case.target", "candidate.output", "workspace.file"],
        output_schema=JudgeResult,
    ):
        for item in job.independent_cases():
            case = await cx.case.load(item.case_id, include_input=True, include_target=True)
            ws = await cx.workspace.materialize_candidate(item.candidate_id, surface="full_repo")

            diff_task = cx.workspace.git_diff(ws, against="parent")
            log_task = cx.workspace.git_log(ws, max_entries=20)
            pytest_task = cx.sandbox.exec(
                workspace=ws,
                argv=["pytest", "-q", "tests/hidden", "--json-report"],
                timeout_s=180,
                output_contract=lv.output.files(["report.json", "pytest.log"], max_bytes=256_000),
                input_classes=["case.target", "workspace.file"],
            )
            agent_task = cx.agent.run(
                runtime="codex-app-server",
                workspace=ws,
                instructions=lv.AgentInstructions(
                    system="You are an evaluation judge. Inspect but do not modify source files.",
                    task=f"Task:\n{case.input['task']}\n\nHidden rubric:\n{case.target['rubric']}\n\nReturn JSON.",
                ),
                tool_policy=lv.AgentToolPolicy(allow_shell=True, allowed_tools=["read_file", "grep", "pytest", "git"]),
                output=lv.output.json_schema(JudgeResult),
                limits=lv.AgentLimits(timeout_s=300, max_turns=20),
                input_classes=["case.input", "case.target", "workspace.file"],
            )
            diff, log, pytest, agent = await asyncio.gather(diff_task, log_task, pytest_task, agent_task)

            pytest_report = lv.pytest.parse_json_report(pytest.files["report.json"])
            dspy_pred = await lv.dspy_acall(
                judge,
                task=case.input["task"],
                target=case.target["rubric"],
                pytest_report=pytest_report.summary,
                workspace_diff=diff.text,
                agent_report=agent.parsed.feedback,
            )
            judge_result = JudgeResult(**dspy_pred.to_dict())
            final_score = 0.6 * pytest_report.pass_rate + 0.4 * judge_result.score

            assessments.append(lv.AssessmentWrite.independent_case(
                candidate=item.candidate_id,
                resolved_set=job.resolved_set.id,
                case=item.case_id,
                score=lv.Score(value=final_score, metrics={
                    "pytest_pass_rate": pytest_report.pass_rate,
                    "dspy_judge_score": judge_result.score,
                    "agent_judge_score": agent.parsed.score,
                }),
                evidence=lv.EvidenceEnvelope(
                    target_derived=True,
                    public=lv.PublicEvidence(
                        feedback=judge_result.feedback,
                        metrics={"pytest_pass_rate": pytest_report.pass_rate, "judge_score": judge_result.score},
                        trace_refs=[pytest.trace_ref.summary(), agent.trace_ref.redacted_transcript(), dspy_pred.leaven_trace_ref.redacted_prompt()],
                    ),
                    private=lv.PrivateEvidence(
                        visibility="evaluator_only",
                        payload={"case_target_ref": case.target_ref, "git_log": log.entries, "agent_raw_session": agent.session_ref},
                    ),
                    redaction_policy={"optimizer": "public_only", "reflector": "score_and_feedback", "operator": "full"},
                ),
                read_receipts=[case.receipt, diff.receipt, log.receipt],
                effect_receipts=[pytest.receipt, agent.receipt, dspy_pred.leaven_lm_receipt],
                cost_attribution=lv.CostAttribution.sum_effect_receipts(),
            ))

    return await cx.assessments.submit(job.evaluation_request_id, assessments)
