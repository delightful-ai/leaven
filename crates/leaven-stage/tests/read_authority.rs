use futures::executor::block_on;
use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity, Evidence, OptimizationProblem};
use leaven_engine::{BudgetLedger, RunContext};
use leaven_kernel::{Budget, ContentId, StageId};
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
    let mut bytes = [0; 32];
    let raw = text.as_bytes();
    bytes[..raw.len().min(32)].copy_from_slice(&raw[..raw.len().min(32)]);
    ContentId::from_bytes(bytes)
}
