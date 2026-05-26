use std::sync::Arc;

use futures::{FutureExt, executor::block_on};
use leaven_eval::Case;
use leaven_evidence::OutputRecord;
use leaven_kernel::{BudgetSnapshot, CandidateId, CaseId, Cost};

use super::{JudgeEvaluationJob, JudgeRequestKind, JudgeScoreContext, evaluate_judge_job};
use crate::test_support::IntArtifact;
use crate::{RunCase, RunOutput, Score};

#[test]
fn judge_job_preserves_group_scoped_reportable_output() {
    block_on(async {
        let left = CandidateId::new();
        let right = CandidateId::new();
        let runner: super::Runner<IntArtifact, i32, String> =
            Arc::new(|artifact: IntArtifact, case: RunCase<i32>| {
                async move {
                    Ok(RunOutput::new(format!("{}:{}", artifact.0, case.input()))
                        .with_cost(Cost::metric_calls(2))
                        .with_trace(format!("ran {}", artifact.0)))
                }
                .boxed()
            });
        let scorer: super::JudgeScorer<IntArtifact, i32, leaven_eval::NoTarget, String> = Arc::new(
            |ctx: JudgeScoreContext<IntArtifact, i32, leaven_eval::NoTarget, String>| {
                async move {
                    assert_eq!(ctx.outputs.len(), 2);
                    let report = ctx.report_text_output(format!(
                        "{}|{}",
                        ctx.outputs[0].output.output, ctx.outputs[1].output.output
                    ));
                    Ok(Score::new(1.0, "left wins")
                        .with_output(report)
                        .with_trace("judged pair"))
                }
                .boxed()
            },
        );
        let outcome = evaluate_judge_job(
            JudgeEvaluationJob {
                case_index: 0,
                request_kind: JudgeRequestKind::Pairwise { left, right },
                artifacts: vec![(left, IntArtifact(40)), (right, IntArtifact(41))],
                case: Case::input(CaseId::new(0), 2),
                budget: BudgetSnapshot::default(),
            },
            &runner,
            &scorer,
        )
        .await
        .unwrap();

        assert_eq!(outcome.case_id, CaseId::new(0));
        assert_eq!(outcome.cost.metric_calls, 5);
        assert_eq!(outcome.evidence.feedback(), "left wins");
        assert_eq!(
            outcome.evidence.trace(),
            &[
                "ran 40".to_owned(),
                "ran 41".to_owned(),
                "judged pair".to_owned()
            ]
        );
        assert_eq!(
            outcome.evidence.output(),
            &candidate_output_record("40:2|41:2")
        );
    });
}

fn candidate_output_record(output: impl Into<String>) -> OutputRecord {
    OutputRecord::candidate_inline(output)
}
