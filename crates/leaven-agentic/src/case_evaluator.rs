//! Stock evaluator over agentic cases.

use std::future::Future;
use std::marker::PhantomData;

use leaven_agent::{AgentRunContext, AgentRunRequest, AgentRuntime, AgentSession};
use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, OptimizationProblem,
    ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{CachePolicy, EvaluationContext, EvaluationError, Evaluator};
use leaven_kernel::{
    AgentSessionId, CandidateId, Cost, EvaluationSetId, EvaluatorId, Fingerprint,
    FingerprintBuilder, MetadataBag, MetadataValue, Metered,
};
use leaven_workspace::{
    WithWorkspaceError, WorkspaceConfig, WorkspaceFactory, WorkspacePath, WorkspaceView,
};

use crate::AgenticAdapterError;
use crate::case::{AgentCase, CaseSuite};
use crate::case_record::{AgentCaseRunRecord, CASE_RUN_RECORD_METADATA_KEY};
use crate::error::{checked_add_cost, map_workspace_error};

/// Presenter input for one candidate/case execution.
pub struct AgentCasePresentationInput<'a, P>
where
    P: OptimizationProblem,
{
    pub candidate_id: CandidateId,
    pub candidate: &'a P::Artifact,
    pub case: &'a AgentCase,
    pub graph: leaven_engine::RunGraphView<'a, P>,
}

/// Agent-ready presentation for one candidate/case execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentCasePresentation {
    pub request: AgentRunRequest,
    pub materialized_refs: Vec<WorkspacePath>,
}

/// Capability that turns candidate + case into an agent-ready workspace.
pub trait AgentCasePresenter<P>: Send + Sync
where
    P: OptimizationProblem,
{
    fn fingerprint(&self) -> Fingerprint;

    fn present<'a>(
        &'a self,
        input: AgentCasePresentationInput<'a, P>,
        workspace: &'a mut WorkspaceView<'_>,
        ctx: leaven_engine::MaterializeContext<'a, P>,
    ) -> impl Future<Output = Result<Metered<AgentCasePresentation>, AgenticAdapterError>> + Send + 'a;
}

/// Scorer input for one completed agent case session.
pub struct AgentCaseScoreInput<'a, P>
where
    P: OptimizationProblem,
{
    pub candidate_id: CandidateId,
    pub case: &'a AgentCase,
    pub presentation: &'a AgentCasePresentation,
    pub session: &'a AgentSession,
    pub graph: leaven_engine::RunGraphView<'a, P>,
}

/// Capability that scores one agent session into problem evidence.
pub trait AgentCaseScorer<P>: Send + Sync
where
    P: OptimizationProblem,
{
    fn fingerprint(&self) -> Fingerprint;

    fn score<'a>(
        &'a self,
        input: AgentCaseScoreInput<'a, P>,
        workspace: &'a WorkspaceView<'_>,
    ) -> impl Future<Output = Result<Metered<P::Evidence>, AgenticAdapterError>> + Send + 'a;
}

/// Stock evaluator over agentic cases.
pub struct AgentCaseEvaluator<P, Factory, Runtime, Presenter, Scorer>
where
    P: OptimizationProblem,
{
    config: AgentCaseEvaluatorConfig,
    cases: CaseSuite,
    workspace_factory: Factory,
    runtime: Runtime,
    presenter: Presenter,
    scorer: Scorer,
    marker: PhantomData<P>,
}

impl<P, Factory, Runtime, Presenter, Scorer>
    AgentCaseEvaluator<P, Factory, Runtime, Presenter, Scorer>
where
    P: OptimizationProblem,
{
    #[must_use]
    pub fn new(
        config: AgentCaseEvaluatorConfig,
        cases: CaseSuite,
        workspace_factory: Factory,
        runtime: Runtime,
        presenter: Presenter,
        scorer: Scorer,
    ) -> Self {
        Self {
            config,
            cases,
            workspace_factory,
            runtime,
            presenter,
            scorer,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn cases(&self) -> &CaseSuite {
        &self.cases
    }
}

/// Configuration for [`AgentCaseEvaluator`].
#[derive(Clone, Debug)]
pub struct AgentCaseEvaluatorConfig {
    pub id: EvaluatorId,
    pub fingerprint: Fingerprint,
    pub workspace: WorkspaceConfig,
    pub cache_policy: CachePolicy,
}

impl AgentCaseEvaluatorConfig {
    #[must_use]
    pub fn new(id: EvaluatorId, fingerprint: Fingerprint) -> Self {
        Self {
            id,
            fingerprint,
            workspace: WorkspaceConfig::default(),
            cache_policy: CachePolicy::Never,
        }
    }
}

impl<P, Factory, Runtime, Presenter, Scorer> Evaluator<P>
    for AgentCaseEvaluator<P, Factory, Runtime, Presenter, Scorer>
where
    P: OptimizationProblem,
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
    Presenter: AgentCasePresenter<P>,
    Scorer: AgentCaseScorer<P>,
{
    fn id(&self) -> EvaluatorId {
        self.config.id.clone()
    }

    fn fingerprint(&self) -> Fingerprint {
        let mut builder = FingerprintBuilder::new();
        builder
            .update(b"leaven.agentic.agent-case-evaluator.v1")
            .update(self.config.fingerprint.0)
            .update(self.cases.fingerprint().0)
            .update(self.runtime.fingerprint().0)
            .update(self.presenter.fingerprint().0)
            .update(self.scorer.fingerprint().0);
        builder.finish()
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        self.config.cache_policy.clone()
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        mut ctx: EvaluationContext<'_, P>,
    ) -> Result<Metered<Vec<Assessment<P>>>, EvaluationError> {
        if request.granularity != AssessmentGranularity::PerCase {
            return Err(EvaluationError::with_source(
                "agent case evaluator failed",
                AgenticAdapterError::Input(
                    "AgentCaseEvaluator currently requires per-case granularity".to_owned(),
                ),
            ));
        }

        let candidates = match &request.kind {
            ResolvedRequestKind::Independent { candidates } => candidates.clone(),
            ResolvedRequestKind::Pairwise { .. } | ResolvedRequestKind::Listwise { .. } => {
                return Err(EvaluationError::with_source(
                    "agent case evaluator failed",
                    AgenticAdapterError::Input(
                        "AgentCaseEvaluator currently supports independent requests".to_owned(),
                    ),
                ));
            }
        };

        let mut total = Cost::zero();
        let mut assessments = Vec::new();
        for candidate_id in candidates {
            let candidate = ctx.graph().artifact(candidate_id).cloned().ok_or_else(|| {
                EvaluationError::with_source(
                    "agent case evaluator failed",
                    AgenticAdapterError::Input(format!("unknown candidate `{candidate_id}`")),
                )
            })?;
            for case_id in &request.set.case_ids {
                let case = self.cases.cases().get(case_id).ok_or_else(|| {
                    EvaluationError::with_source(
                        "agent case evaluator failed",
                        AgenticAdapterError::Input(format!(
                            "case suite does not contain resolved case `{case_id}`"
                        )),
                    )
                })?;
                let metered = self
                    .evaluate_one(candidate_id, &candidate, case, &request, &mut ctx)
                    .await?;
                total = checked_add_cost(total, &metered.cost).map_err(|error| {
                    EvaluationError::with_source("agent case evaluator failed", error)
                })?;
                assessments.push(metered.value);
            }
        }
        Ok(Metered::new(assessments, total))
    }
}

impl<P, Factory, Runtime, Presenter, Scorer>
    AgentCaseEvaluator<P, Factory, Runtime, Presenter, Scorer>
where
    P: OptimizationProblem,
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
    Presenter: AgentCasePresenter<P>,
    Scorer: AgentCaseScorer<P>,
{
    async fn evaluate_one(
        &self,
        candidate_id: CandidateId,
        candidate: &P::Artifact,
        case: &AgentCase,
        request: &ResolvedEvaluationRequest,
        ctx: &mut EvaluationContext<'_, P>,
    ) -> Result<Metered<Assessment<P>>, EvaluationError> {
        let mut workspace = self
            .workspace_factory
            .allocate(self.config.workspace.clone())
            .await
            .map_err(|error| {
                EvaluationError::with_source(
                    "agent case evaluator failed",
                    AgenticAdapterError::WorkspaceAllocate(error),
                )
            })?;
        let stage_result = async {
            let mut view = workspace.view();
            let presented = self
                .presenter
                .present(
                    AgentCasePresentationInput {
                        candidate_id,
                        candidate,
                        case,
                        graph: ctx.graph().clone(),
                    },
                    &mut view,
                    ctx.materialize_context(),
                )
                .await?;
            let budget = ctx.budget();
            let session = self
                .runtime
                .run_session(
                    &mut view,
                    presented.value.request.clone(),
                    AgentRunContext::new(AgentSessionId::new(), &budget),
                )
                .await
                .map_err(AgenticAdapterError::Runtime)?;
            let scored = self
                .scorer
                .score(
                    AgentCaseScoreInput {
                        candidate_id,
                        case,
                        presentation: &presented.value,
                        session: &session.value,
                        graph: ctx.graph().clone(),
                    },
                    &view,
                )
                .await?;
            let run_total = checked_add_cost(Cost::zero(), &presented.cost)?;
            let run_total = checked_add_cost(run_total, &session.cost)?;
            let run_total = checked_add_cost(run_total, &scored.cost)?;
            let partition = EvaluationSetId::from_uuid(request.set.id.as_uuid());
            let run_record = AgentCaseRunRecord::scored(
                ctx.graph().run_id(),
                candidate_id,
                case.id,
                partition,
                session.value.session_id,
                session.value.output_files.clone(),
                run_total.clone(),
            );
            let mut metadata = MetadataBag::new();
            metadata.insert(
                CASE_RUN_RECORD_METADATA_KEY,
                MetadataValue::Json(serde_json::to_value(&run_record).map_err(|error| {
                    AgenticAdapterError::Input(format!(
                        "case run record serialization failed: {error}"
                    ))
                })?),
            );
            let assessment = Assessment::Independent {
                candidate: candidate_id,
                target: AssessmentTarget::Case {
                    set: partition,
                    case: case.id,
                },
                evidence: scored.value,
                cost: run_total.clone(),
                metadata,
            };
            drop(view);
            Ok(Metered::new(assessment, run_total))
        }
        .await;
        let cleanup_result = workspace.cleanup().await;
        match (stage_result, cleanup_result) {
            (Ok(metered), Ok(())) => Ok(metered),
            (Ok(_), Err(cleanup)) => Err(map_workspace_error(WithWorkspaceError::Cleanup(cleanup))),
            (Err(stage), Ok(())) => Err(stage),
            (Err(stage), Err(cleanup)) => {
                Err(map_workspace_error(WithWorkspaceError::StageAndCleanup {
                    stage,
                    cleanup,
                }))
            }
        }
        .map_err(|error| EvaluationError::with_source("agent case evaluator failed", error))
    }
}
