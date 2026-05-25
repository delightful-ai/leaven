use std::collections::{BTreeMap, BTreeSet};

use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    CaseSetVersion, CausalInputs, EvaluationRequest, EvaluationSet, ProposalEffectKind,
    ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_eval::SplitPolicy;
use leaven_evidence::{CaseAssessmentEvidence, OutputRecord, ScalarEvidence};
use leaven_kernel::{
    AssessmentId, BudgetSnapshot, CandidateId, CaseId, ContentId, Cost, ErrorKind, ErrorRecord,
    EvaluationRequestId, EvaluationSetId, EvaluatorId, Fingerprint, IterationId, MetadataBag,
    Metered, PopulationId, ProposalBatchId, ProposalId, RunId, StageAttemptFailure,
    StageAttemptOutcome, StageAttemptReceiptId, StageAttemptReceiptRef, StageCallId, StageId,
    StageRole,
};
use leaven_store_inline::InlineEvidenceStore;

use super::assessment::assessment_summary;
use super::*;

mod assessment;
mod events;
mod splits;
mod summary;

struct ReportHarness {
    case_set: leaven_engine::CaseSet<leaven_eval::Case<&'static str>>,
    engine: leaven_engine::Engine<RunProblem<TestArtifact, &'static str>>,
    first: CandidateId,
    second: CandidateId,
    store: InlineEvidenceStore<CaseAssessmentEvidence>,
    splits: DatasetSplits,
}

fn report_harness() -> ReportHarness {
    let train = PartitionId::from("TRAIN");
    let audit = PartitionId::from("audit");
    let ignored = PartitionId::from("ignored");
    let train_case = CaseId::from_index(0);
    let audit_case = CaseId::from_index(1);
    let case_set = leaven_engine::CaseSet::from_entries([
        (train_case, leaven_eval::Case::input(train_case, "train")),
        (audit_case, leaven_eval::Case::input(audit_case, "audit")),
    ])
    .with_partition(train.clone(), vec![train_case])
    .with_partition(audit.clone(), vec![audit_case])
    .with_partition(ignored, vec![audit_case]);
    let mut engine = leaven_engine::Engine::<RunProblem<TestArtifact, &'static str>>::builder()
        .budget(leaven_kernel::Budget::unlimited())
        .evaluator(ReportEvaluator)
        .build();
    let first = engine.insert_seed(TestArtifact, 0).unwrap();
    let second = engine.insert_seed(TestArtifact, 1).unwrap();
    let splits = DatasetSplits::new(
        CaseSetVersion("report-v1".to_owned()),
        BTreeMap::from([
            (train.clone(), SplitRole::Train),
            (audit.clone(), SplitRole::Custom("audit".into())),
        ]),
        BTreeMap::from([(train, vec![train_case]), (audit, vec![audit_case])]),
        &BTreeSet::from([train_case, audit_case]),
        SplitPolicy::DisjointRequired,
    )
    .unwrap();
    ReportHarness {
        case_set,
        engine,
        first,
        second,
        store: InlineEvidenceStore::<CaseAssessmentEvidence>::new("report-groups"),
        splits,
    }
}

#[derive(Clone, Debug)]
struct TestArtifact;

#[derive(Debug)]
struct TestArtifactError;

impl std::fmt::Display for TestArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("test artifact error")
    }
}

impl std::error::Error for TestArtifactError {}

impl Artifact for TestArtifact {
    type Change = ();
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::from_bytes([7; ContentId::BYTES]))
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self)
    }
}

struct NoopPersistence;

impl leaven_engine::RunPersistence<RunProblem<TestArtifact, (), ()>> for NoopPersistence {
    fn checkpoint(
        &self,
        _request: leaven_engine::RunCheckpointRequest<'_, RunProblem<TestArtifact, (), ()>>,
    ) -> Result<(), leaven_engine::RunPersistenceError> {
        Ok(())
    }
}

struct ReportEvaluator;

impl leaven_engine::Evaluator<RunProblem<TestArtifact, &'static str>> for ReportEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([9; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: leaven_engine::EvaluationContext<'_, RunProblem<TestArtifact, &'static str>>,
    ) -> Result<
        Metered<Vec<Assessment<RunProblem<TestArtifact, &'static str>>>>,
        leaven_engine::EvaluationError,
    > {
        let mut assessments = Vec::new();
        match request.kind {
            ResolvedRequestKind::Independent { candidates } => {
                for candidate in candidates {
                    if matches!(request.granularity, AssessmentGranularity::Aggregate) {
                        assessments.push(Assessment::Independent {
                            candidate,
                            target: AssessmentTarget::EvaluationSet(EvaluationSetId::new()),
                            evidence: report_evidence("aggregate"),
                            cost: Cost::metric_calls(1),
                            metadata: MetadataBag::new(),
                        });
                        continue;
                    }
                    for case in &request.set.case_ids {
                        assessments.push(Assessment::Independent {
                            candidate,
                            target: AssessmentTarget::Case {
                                set: EvaluationSetId::new(),
                                case: *case,
                            },
                            evidence: report_evidence("case"),
                            cost: Cost::metric_calls(1),
                            metadata: MetadataBag::new(),
                        });
                    }
                }
            }
            ResolvedRequestKind::Pairwise { left, right, .. } => {
                for case in &request.set.case_ids {
                    assessments.push(Assessment::Pairwise {
                        left,
                        right,
                        target: AssessmentTarget::Case {
                            set: EvaluationSetId::new(),
                            case: *case,
                        },
                        evidence: report_evidence("pairwise"),
                        cost: Cost::metric_calls(1),
                        metadata: MetadataBag::new(),
                    });
                }
            }
            ResolvedRequestKind::Listwise { .. } => {}
        }
        Ok(Metered::new(
            assessments,
            Cost::metric_calls(request.set.case_ids.len() as u64),
        ))
    }
}

fn report_evidence(label: &'static str) -> CaseAssessmentEvidence {
    CaseAssessmentEvidence::new(
        ScalarEvidence::new(1.0).unwrap(),
        OutputRecord::inline(label),
        format!("{label} feedback"),
    )
}

struct BadPartitionEvaluator;

impl leaven_engine::Evaluator<RunProblem<TestArtifact, &'static str>> for BadPartitionEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([8; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: leaven_engine::EvaluationContext<'_, RunProblem<TestArtifact, &'static str>>,
    ) -> Result<
        Metered<Vec<Assessment<RunProblem<TestArtifact, &'static str>>>>,
        leaven_engine::EvaluationError,
    > {
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Ok(Metered::new(Vec::new(), Cost::zero()));
        };
        let [left, right, ..] = candidates.as_slice() else {
            return Ok(Metered::new(Vec::new(), Cost::zero()));
        };
        let case = request.set.case_ids[0];
        Ok(Metered::new(
            vec![Assessment::Pairwise {
                left: *left,
                right: *right,
                target: AssessmentTarget::Case {
                    set: EvaluationSetId::new(),
                    case,
                },
                evidence: report_evidence("bad pairwise"),
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            }],
            Cost::metric_calls(1),
        ))
    }
}
