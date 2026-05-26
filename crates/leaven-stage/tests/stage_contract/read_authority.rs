use futures::executor::block_on;
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, CacheIdentity,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem,
    ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    BudgetLedger, CachePolicy, CaseSet, EvaluationContext, EvaluationError, Evaluator, RunContext,
};
use leaven_kernel::{
    Budget, ContentId, Cost, EvaluatorId, Fingerprint, MetadataBag, Metered, StageId,
};
use leaven_stage::{
    AllowedQuerySet, QueryRecordEffect, QueryTiming, StageQuery, StageQueryKind, StageQueryPolicy,
    StageReadAuthority,
};
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn read_authority_writes_candidate_query_entry_and_receipt() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let candidate = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposal_ctx = ctx.proposal_context(StageId::custom("stage"));
        let stage_ctx = proposal_ctx.stage_engine_context();
        let policy = StageQueryPolicy::bounded(
            AllowedQuerySet::only([StageQueryKind::Candidate]),
            Vec::new(),
            Some(1),
            Some(4096),
        );
        let mut authority = StageReadAuthority::new(stage_ctx, policy);
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        {
            let mut slot = workspace
                .slot(WorkspacePath::new("stage").unwrap())
                .unwrap();

            let result = authority
                .query(
                    &mut slot,
                    QueryTiming::AgentRequested,
                    StageQuery::Candidate { id: candidate },
                )
                .unwrap();
            let record = result.into_record();

            assert!(matches!(record.effect, QueryRecordEffect::WroteEntries(_)));
            assert_eq!(record.entries.len(), 1);
            assert_eq!(record.entries[0].produced_by_query, Some(record.query_id));
            assert_eq!(
                record.entries[0].bytes,
                record.entries[0].file.as_ref().map(|file| file.bytes)
            );
            let bytes = slot.read_file(&record.entries[0].path).unwrap();
            let rendered = String::from_utf8(bytes).unwrap();
            assert!(rendered.contains(&candidate.to_string()));
        }

        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn read_authority_policy_denial_does_not_write_workspace_entry() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposal_ctx = ctx.proposal_context(StageId::custom("stage"));
        let stage_ctx = proposal_ctx.stage_engine_context();
        let policy = StageQueryPolicy::minimal();
        let mut authority = StageReadAuthority::new(stage_ctx, policy);
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        {
            let mut slot = workspace
                .slot(WorkspacePath::new("stage").unwrap())
                .unwrap();

            let result = authority
                .query(&mut slot, QueryTiming::AgentRequested, StageQuery::Help)
                .unwrap();
            let record = result.into_record();

            assert!(matches!(record.effect, QueryRecordEffect::PolicyDenied(_)));
            assert!(record.entries.is_empty());
        }

        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn read_authority_prewarm_uses_same_query_path_as_agent_requests() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposal_ctx = ctx.proposal_context(StageId::custom("stage"));
        let stage_ctx = proposal_ctx.stage_engine_context();
        let policy = StageQueryPolicy::bounded(
            AllowedQuerySet::only([StageQueryKind::Help]),
            vec![StageQuery::Help],
            Some(1),
            Some(4096),
        );
        let mut authority = StageReadAuthority::new(stage_ctx, policy);
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        {
            let mut slot = workspace
                .slot(WorkspacePath::new("stage").unwrap())
                .unwrap();

            let records: Vec<_> = authority
                .prewarm(&mut slot)
                .unwrap()
                .into_iter()
                .map(leaven_stage::QueryResult::into_record)
                .collect();

            assert_eq!(records.len(), 1);
            assert_eq!(records[0].timing, QueryTiming::Prewarm);
            assert!(matches!(
                records[0].effect,
                QueryRecordEffect::WroteEntries(_)
            ));
            assert_eq!(
                records[0].entries[0].path,
                WorkspacePath::new("queries/help.json").unwrap()
            );
        }

        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn read_authority_renders_all_visible_candidate_queries_and_records_limits() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let (left, right) = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            (
                ctx.insert_seed(TextArtifact("left".to_owned()), 0).unwrap(),
                ctx.insert_seed(TextArtifact("right".to_owned()), 0)
                    .unwrap(),
            )
        };
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposal_ctx = ctx.proposal_context(StageId::custom("stage"));
        let stage_ctx = proposal_ctx.stage_engine_context();
        let policy = StageQueryPolicy::bounded(
            AllowedQuerySet::only([
                StageQueryKind::Help,
                StageQueryKind::ListCandidates,
                StageQueryKind::Candidate,
                StageQueryKind::Evidence,
                StageQueryKind::Lineage,
                StageQueryKind::Diff,
            ]),
            Vec::new(),
            Some(8),
            Some(8192),
        );
        let mut authority = StageReadAuthority::new(stage_ctx, policy);
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        {
            let mut slot = workspace
                .slot(WorkspacePath::new("stage").unwrap())
                .unwrap();
            let queries = [
                StageQuery::Help,
                StageQuery::ListCandidates,
                StageQuery::Candidate { id: left },
                StageQuery::Evidence,
                StageQuery::Lineage {
                    candidate: left,
                    depth: 2,
                },
                StageQuery::Diff { left, right },
            ];

            for query in queries {
                let record = authority
                    .query(&mut slot, QueryTiming::AgentRequested, query)
                    .unwrap()
                    .into_record();
                assert!(matches!(record.effect, QueryRecordEffect::WroteEntries(_)));
                assert_eq!(record.entries.len(), 1);
                assert!(slot.read_file(&record.entries[0].path).is_ok());
            }

            let missing = authority
                .query(
                    &mut slot,
                    QueryTiming::AgentRequested,
                    StageQuery::Candidate {
                        id: leaven_kernel::CandidateId::new(),
                    },
                )
                .unwrap()
                .into_record();
            assert!(matches!(missing.effect, QueryRecordEffect::NotFound(_)));
        }
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn read_authority_enforces_query_and_byte_limits() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let candidate = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposal_ctx = ctx.proposal_context(StageId::custom("stage"));
        let stage_ctx = proposal_ctx.stage_engine_context();
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        {
            let mut slot = workspace
                .slot(WorkspacePath::new("stage").unwrap())
                .unwrap();
            let policy = StageQueryPolicy::bounded(
                AllowedQuerySet::only([StageQueryKind::Help]),
                Vec::new(),
                Some(1),
                Some(4096),
            );
            let mut authority = StageReadAuthority::new(stage_ctx.clone(), policy);
            assert!(matches!(
                authority
                    .query(&mut slot, QueryTiming::AgentRequested, StageQuery::Help)
                    .unwrap()
                    .effect,
                leaven_stage::QueryEffect::WroteEntries(_)
            ));
            assert!(matches!(
                authority
                    .query(&mut slot, QueryTiming::AgentRequested, StageQuery::Help)
                    .unwrap()
                    .effect,
                leaven_stage::QueryEffect::PolicyDenied(_)
            ));

            let tiny_policy = StageQueryPolicy::bounded(
                AllowedQuerySet::only([StageQueryKind::Candidate]),
                Vec::new(),
                Some(1),
                Some(1),
            );
            let mut tiny_authority = StageReadAuthority::new(stage_ctx, tiny_policy);
            assert!(matches!(
                tiny_authority
                    .query(
                        &mut slot,
                        QueryTiming::AgentRequested,
                        StageQuery::Candidate { id: candidate },
                    )
                    .unwrap()
                    .effect,
                leaven_stage::QueryEffect::PolicyDenied(_)
            ));
        }
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn read_authority_renders_visible_assessment_queries() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let case_set = CaseSet::new(vec![()]);
        let store = leaven_store_inline::InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate_id = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let assessment_id = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            ctx.evaluate_with(
                &StaticEvaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate_id],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Feedback,
                },
            )
            .await
            .unwrap()
            .assessment_ids[0]
        };
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let proposal_ctx = ctx.proposal_context(StageId::custom("stage"));
        let stage_ctx = proposal_ctx.stage_engine_context();
        let policy = StageQueryPolicy::bounded(
            AllowedQuerySet::only([StageQueryKind::Assessment]),
            Vec::new(),
            Some(1),
            Some(4096),
        );
        let mut authority = StageReadAuthority::new(stage_ctx, policy);
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        {
            let mut slot = workspace.slot(WorkspacePath::root()).unwrap();
            let record = authority
                .query(
                    &mut slot,
                    QueryTiming::AgentRequested,
                    StageQuery::Assessment { id: assessment_id },
                )
                .unwrap()
                .into_record();

            assert!(matches!(record.effect, QueryRecordEffect::WroteEntries(_)));
            let rendered =
                String::from_utf8(slot.read_file(&record.entries[0].path).unwrap()).unwrap();
            assert!(rendered.contains(&assessment_id.to_string()));
        }
        workspace.cleanup().await.unwrap();
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact(String);

impl Artifact for TextArtifact {
    type Change = String;
    type ApplyError = std::convert::Infallible;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(content_id(&self.0))
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(content_id(&self.0)))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(format!("{}{change}", self.0)))
    }
}

#[derive(Clone, Debug)]
struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TextArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct TestEvidence;

impl Evidence for TestEvidence {}

struct StaticEvaluator;

impl Evaluator<TestProblem> for StaticEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::from("static")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([5; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("expected independent".to_owned()));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Unscoped,
                    evidence: TestEvidence,
                    cost: Cost::zero(),
                    metadata: MetadataBag::new(),
                })
                .collect(),
            Cost::zero(),
        ))
    }
}

fn graph_and_budget() -> (
    leaven_engine::RunGraph<TestProblem>,
    leaven_engine::BudgetLedger,
) {
    (
        leaven_engine::RunGraph::new(leaven_kernel::RunId::new()),
        BudgetLedger::new(Budget::unlimited()),
    )
}

fn content_id(text: &str) -> ContentId {
    ContentId::hash_bytes(text)
}
