use leaven_core::OptimizationProblem;
use leaven_engine::StageEngineContext;
use leaven_kernel::{Cost, FingerprintBuilder, MetadataBag, StageQueryId, WorkspaceEntryId};
use leaven_workspace::{WorkspaceError, WorkspacePath, WorkspaceSlot, fingerprint_file};

use crate::receipt::QueryTiming;
use crate::{
    EntryAccess, EntryProjection, EntrySourceRef, QueryRecord, QueryRecordEffect, StageQuery,
    StageQueryError, StageQueryKind, StageQueryPolicy, WorkspaceEntryReceipt, WorkspaceEntryRole,
};

pub struct StageReadAuthority<'a, P: OptimizationProblem> {
    ctx: StageEngineContext<'a, P>,
    policy: StageQueryPolicy,
    issued_queries: usize,
    materialized_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct QueryResult {
    pub query_id: StageQueryId,
    pub timing: QueryTiming,
    pub query: StageQuery,
    pub effect: QueryEffect,
    pub entries: Vec<WorkspaceEntryReceipt>,
    pub cost: Cost,
}

#[derive(Clone, Debug)]
pub enum QueryEffect {
    WroteEntries(Vec<WorkspaceEntryId>),
    NotVisible(String),
    NotFound(String),
    PolicyDenied(String),
}

impl<'a, P: OptimizationProblem> StageReadAuthority<'a, P> {
    #[must_use]
    pub fn new(ctx: StageEngineContext<'a, P>, policy: StageQueryPolicy) -> Self {
        Self {
            ctx,
            policy,
            issued_queries: 0,
            materialized_bytes: 0,
        }
    }

    pub fn prewarm(
        &mut self,
        workspace: &mut WorkspaceSlot<'_>,
    ) -> Result<Vec<QueryResult>, StageQueryError> {
        let queries = self.policy.prewarm.clone();
        queries
            .into_iter()
            .map(|query| self.query(workspace, QueryTiming::Prewarm, query))
            .collect()
    }

    pub fn query(
        &mut self,
        workspace: &mut WorkspaceSlot<'_>,
        timing: QueryTiming,
        query: StageQuery,
    ) -> Result<QueryResult, StageQueryError> {
        let query_id = StageQueryId::new();
        if !self.policy.allowed.contains(query.kind()) {
            return Ok(QueryResult {
                query_id,
                timing,
                query,
                effect: QueryEffect::PolicyDenied("query kind is not allowed".to_owned()),
                entries: Vec::new(),
                cost: Cost::zero(),
            });
        }
        if self
            .policy
            .max_queries
            .is_some_and(|max| self.issued_queries >= max)
        {
            return Ok(QueryResult {
                query_id,
                timing,
                query,
                effect: QueryEffect::PolicyDenied("query limit exhausted".to_owned()),
                entries: Vec::new(),
                cost: Cost::zero(),
            });
        }

        self.issued_queries += 1;
        let rendered = self.render_query(&query);
        let Some(rendered) = rendered else {
            return Ok(QueryResult {
                query_id,
                timing,
                query,
                effect: QueryEffect::NotFound(
                    "query target is not visible or does not exist".to_owned(),
                ),
                entries: Vec::new(),
                cost: Cost::zero(),
            });
        };
        let bytes = rendered.bytes.as_bytes();
        if self
            .policy
            .max_materialized_bytes
            .is_some_and(|max| self.materialized_bytes + bytes.len() as u64 > max)
        {
            return Ok(QueryResult {
                query_id,
                timing,
                query,
                effect: QueryEffect::PolicyDenied("materialized byte limit exhausted".to_owned()),
                entries: Vec::new(),
                cost: Cost::zero(),
            });
        }

        workspace.write_file(&rendered.path, bytes)?;
        self.materialized_bytes += bytes.len() as u64;
        let file = fingerprint_file(workspace.view(), &rendered.path)?;
        let mut fingerprint = FingerprintBuilder::new();
        fingerprint
            .update(b"leaven.stage.workspace-entry.v1")
            .update(rendered.path.as_str().as_bytes())
            .update(rendered.source.fingerprint_bytes())
            .update(rendered.projection.fingerprint_bytes())
            .update(file.fingerprint.0);
        let entry = WorkspaceEntryReceipt {
            id: WorkspaceEntryId::new(),
            path: rendered.path,
            role: WorkspaceEntryRole::query_summary(),
            source: rendered.source,
            projection: rendered.projection,
            access: EntryAccess::InputReadOnly,
            fingerprint: fingerprint.finish(),
            file: Some(file.clone()),
            bytes: Some(file.bytes),
            produced_by_query: Some(query_id),
            metadata: MetadataBag::new(),
        };
        Ok(QueryResult {
            query_id,
            timing,
            query,
            effect: QueryEffect::WroteEntries(vec![entry.id]),
            entries: vec![entry],
            cost: Cost::zero(),
        })
    }

    fn render_query(&self, query: &StageQuery) -> Option<RenderedQuery> {
        let json = match query {
            StageQuery::Help => serde_json::json!({
                "kind": "help",
                "queries": StageQueryKind::all_v0_4()
                    .iter()
                    .map(|kind| kind.label())
                    .collect::<Vec<_>>()
            }),
            StageQuery::ListCandidates => serde_json::json!({
                "kind": "list_candidates",
                "candidate_count": self.ctx.graph().candidate_count()
            }),
            StageQuery::Candidate { id } => {
                let candidate = self.ctx.graph().candidate(*id)?;
                serde_json::json!({
                    "kind": "candidate",
                    "id": candidate.id().to_string(),
                    "origin": format!("{:?}", candidate.origin()),
                    "identity": format!("{:?}", candidate.identity()),
                    "created_at": candidate.created_at()
                })
            }
            StageQuery::Assessment { id } => {
                let assessment = self.ctx.graph().assessment(*id)?;
                serde_json::json!({
                    "kind": "assessment",
                    "id": assessment.id().to_string(),
                    "request_id": assessment.request_id().to_string(),
                    "evidence_ref": format!("{:?}", assessment.evidence_ref()),
                    "target": format!("{:?}", assessment.target()),
                    "created_at": assessment.created_at()
                })
            }
            StageQuery::Evidence => serde_json::json!({
                "kind": "evidence",
                "visibility": format!("{:?}", self.ctx.graph().visible_evidence())
            }),
            StageQuery::Lineage { candidate, depth } => {
                let candidate_view = self.ctx.graph().candidate(*candidate)?;
                serde_json::json!({
                    "kind": "lineage",
                    "candidate": candidate_view.id().to_string(),
                    "depth": depth
                })
            }
            StageQuery::Diff { left, right } => {
                let left = self.ctx.graph().candidate(*left)?;
                let right = self.ctx.graph().candidate(*right)?;
                serde_json::json!({
                    "kind": "diff",
                    "left": left.id().to_string(),
                    "right": right.id().to_string()
                })
            }
        };
        let source = match query {
            StageQuery::Candidate { id } => EntrySourceRef::Candidate(*id),
            StageQuery::Assessment { id } => EntrySourceRef::Assessment(*id),
            StageQuery::Help
            | StageQuery::ListCandidates
            | StageQuery::Evidence
            | StageQuery::Lineage { .. }
            | StageQuery::Diff { .. } => EntrySourceRef::Generated,
        };
        Some(RenderedQuery {
            path: query_path(query).ok()?,
            bytes: serde_json::to_string_pretty(&json).ok()?,
            source,
            projection: EntryProjection::Summary,
        })
    }
}

impl QueryResult {
    #[must_use]
    pub fn into_record(self) -> QueryRecord {
        QueryRecord {
            query_id: self.query_id,
            timing: self.timing,
            query: self.query,
            effect: match self.effect {
                QueryEffect::WroteEntries(entries) => QueryRecordEffect::WroteEntries(entries),
                QueryEffect::NotVisible(message) => QueryRecordEffect::NotVisible(message),
                QueryEffect::NotFound(message) => QueryRecordEffect::NotFound(message),
                QueryEffect::PolicyDenied(message) => QueryRecordEffect::PolicyDenied(message),
            },
            entries: self.entries,
            cost: self.cost,
        }
    }
}

struct RenderedQuery {
    path: WorkspacePath,
    bytes: String,
    source: EntrySourceRef,
    projection: EntryProjection,
}

fn query_path(query: &StageQuery) -> Result<WorkspacePath, WorkspaceError> {
    let file = match query {
        StageQuery::Help => "help.json".to_owned(),
        StageQuery::ListCandidates => "candidates.json".to_owned(),
        StageQuery::Candidate { id } => format!("candidate-{id}.json"),
        StageQuery::Assessment { id } => format!("assessment-{id}.json"),
        StageQuery::Evidence => "evidence.json".to_owned(),
        StageQuery::Lineage { candidate, .. } => format!("lineage-{candidate}.json"),
        StageQuery::Diff { left, right } => format!("diff-{left}-{right}.json"),
    };
    WorkspacePath::new(format!("queries/{file}")).map_err(WorkspaceError::from)
}

trait EntrySourceRefFingerprint {
    fn fingerprint_bytes(&self) -> Vec<u8>;
}

impl EntrySourceRefFingerprint for EntrySourceRef {
    fn fingerprint_bytes(&self) -> Vec<u8> {
        format!("{self:?}").into_bytes()
    }
}

trait EntryProjectionFingerprint {
    fn fingerprint_bytes(&self) -> Vec<u8>;
}

impl EntryProjectionFingerprint for EntryProjection {
    fn fingerprint_bytes(&self) -> Vec<u8> {
        format!("{self:?}").into_bytes()
    }
}
