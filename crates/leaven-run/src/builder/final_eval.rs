use leaven_core::{Artifact, EvaluationPurpose, PartitionId};
use leaven_eval::Case;
use leaven_evidence::CaseAssessmentEvidence;
use leaven_kernel::Cost;
use leaven_store::EvidenceStore;

use crate::{
    builder::{OptimizeBuilder, RunProblem},
    run_report::{
        FinalEvaluationInputs, FinalEvaluations, FinalPartitionEvaluation, FinalPartitionResults,
        final_eval,
    },
};

pub(super) fn final_evaluation_inputs<A, I, T, O, Out>(
    seed: leaven_kernel::CandidateId,
    best: Option<leaven_kernel::CandidateId>,
    builder: &OptimizeBuilder<A, I, T, O, Out>,
) -> FinalEvaluationInputs
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    FinalEvaluationInputs {
        seed,
        best,
        has_train: !builder.train.is_empty(),
        has_validation: !builder.validation.is_empty(),
        has_test: !builder.test.is_empty(),
    }
}

pub(super) async fn run_final_evaluations<A, I, T>(
    engine: &mut leaven_engine::Engine<RunProblem<A, I, T>>,
    case_set: &leaven_engine::CaseSet<Case<I, T>>,
    store: &dyn EvidenceStore<CaseAssessmentEvidence>,
    inputs: FinalEvaluationInputs,
) -> Result<FinalEvaluations, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let mut cost = Cost::zero();
    let train = if inputs.has_train {
        let results = final_eval_partition(
            engine,
            case_set,
            store,
            &inputs,
            FinalPartitionEvaluation {
                partition: PartitionId::from("TRAIN"),
                purpose: EvaluationPurpose::Custom("final-train-report".into()),
            },
        )
        .await?;
        cost = cost.combine(&results.cost);
        Some((results.baseline, results.optimized))
    } else {
        None
    };
    let validation = if inputs.has_validation {
        let results = final_eval_partition(
            engine,
            case_set,
            store,
            &inputs,
            FinalPartitionEvaluation {
                partition: PartitionId::from("VALIDATION"),
                purpose: EvaluationPurpose::Validation,
            },
        )
        .await?;
        cost = cost.combine(&results.cost);
        Some((results.baseline, results.optimized))
    } else {
        None
    };
    let test = if inputs.has_test {
        let results = final_eval_partition(
            engine,
            case_set,
            store,
            &inputs,
            FinalPartitionEvaluation {
                partition: PartitionId::from("TEST"),
                purpose: EvaluationPurpose::FinalTest,
            },
        )
        .await?;
        cost = cost.combine(&results.cost);
        Some((results.baseline, results.optimized))
    } else {
        None
    };
    Ok(FinalEvaluations {
        baseline_train: train.as_ref().map(|(baseline, _)| baseline.clone()),
        train: train.and_then(|(_, optimized)| optimized),
        baseline_validation: validation.as_ref().map(|(baseline, _)| baseline.clone()),
        validation: validation.and_then(|(_, optimized)| optimized),
        baseline_test: test.as_ref().map(|(baseline, _)| baseline.clone()),
        test: test.and_then(|(_, optimized)| optimized),
        cost,
    })
}

async fn final_eval_partition<A, I, T>(
    engine: &mut leaven_engine::Engine<RunProblem<A, I, T>>,
    case_set: &leaven_engine::CaseSet<Case<I, T>>,
    store: &dyn EvidenceStore<CaseAssessmentEvidence>,
    inputs: &FinalEvaluationInputs,
    evaluation: FinalPartitionEvaluation,
) -> Result<FinalPartitionResults, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let (baseline, baseline_cost) = final_eval(
        engine,
        case_set,
        store,
        inputs.seed,
        evaluation.partition.clone(),
        evaluation.purpose.clone(),
    )
    .await?;
    let (optimized, optimized_cost) = if let Some(best) = inputs.best {
        let (optimized, optimized_cost) = final_eval(
            engine,
            case_set,
            store,
            best,
            evaluation.partition,
            evaluation.purpose,
        )
        .await?;
        (Some(optimized), optimized_cost)
    } else {
        (None, Cost::zero())
    };
    Ok(FinalPartitionResults {
        baseline,
        optimized,
        cost: baseline_cost.combine(&optimized_cost),
    })
}
