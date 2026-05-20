use std::future::Future;

use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeError,
    AgentSession, OutputContract,
};
use leaven_core::InfoRef;
use leaven_evidence::{Attachment, AttachmentKind};
use leaven_kernel::{AgentSessionId, BudgetSnapshot, Cost};
use leaven_workspace::{
    WorkspaceConfig, WorkspaceError, WorkspaceFactory, WorkspacePath, WorkspaceView,
};
use serde::Serialize;
use thiserror::Error;

/// Artifact-specific seam for generic agentic reflection.
pub trait ArtifactReflector: Send + Sync {
    type Input: Send + Sync;
    type Change: Send + Sync;
    type Error: std::error::Error + Send + Sync + 'static;

    /// Stable reflection identity written into `MANIFEST.json`.
    fn reflection_id(&self) -> &'static str;

    /// Project the typed artifact input into `target/current`.
    fn project<'a>(
        &'a self,
        input: &'a Self::Input,
        view: &'a mut WorkspaceView<'_>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'a;

    /// Read agent edits in `target/current` back into a typed change.
    fn read_back<'a>(
        &'a self,
        input: &'a Self::Input,
        view: &'a WorkspaceView<'_>,
        session: &'a AgentSession,
    ) -> impl Future<Output = Result<ReadbackResult<Self::Change>, Self::Error>> + Send + 'a;
}

/// Result of lowering an edited reflection workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadbackResult<T> {
    Valid(T),
    Empty,
    Invalid {
        diagnostics: Vec<ReadbackDiagnostic>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReadbackDiagnostic {
    pub path: Option<WorkspacePath>,
    pub message: String,
}

/// Generic runner for one agentic reflection transaction.
#[derive(Clone, Debug)]
pub struct ReflectionWorkspace {
    layout: ReflectionLayoutConfig,
}

impl ReflectionWorkspace {
    #[must_use]
    pub const fn new(layout: ReflectionLayoutConfig) -> Self {
        Self { layout }
    }

    pub async fn run<R, Factory, Runtime>(
        &self,
        reflector: &R,
        input: &R::Input,
        cases: &[leaven_gepa::ReflectiveCase],
        source_refs: &[InfoRef],
        factory: &Factory,
        runtime: &Runtime,
        budget: &BudgetSnapshot,
    ) -> Result<ReflectionRunOutcome<R::Change>, ReflectionError<R::Error>>
    where
        R: ArtifactReflector,
        Factory: WorkspaceFactory,
        Runtime: AgentRuntime,
    {
        let mut workspace =
            factory
                .allocate(WorkspaceConfig::default())
                .await
                .map_err(|error| {
                    ReflectionError::Workspace(WorkspaceError::Cleanup(error.to_string()))
                })?;
        let stage_result = async {
            let mut view = workspace.view();
            write_workspace_contract::<R::Error>(
                &mut view,
                reflector.reflection_id(),
                &self.layout,
                cases,
                source_refs,
            )?;
            {
                let mut current = view.subdir(self.layout.mutable_root.clone())?;
                reflector
                    .project(input, &mut current)
                    .await
                    .map_err(ReflectionError::Project)?;
            }

            let session = runtime
                .run_session(
                    &mut view,
                    AgentRunRequest {
                        instructions: AgentInstructions::task(
                            "Read TASK.md and edit target/current in place.",
                        ),
                        cwd: WorkspacePath::root(),
                        output_contract: OutputContract::WorkspaceDiff {
                            roots: vec![self.layout.mutable_root.clone()],
                        },
                        env: Default::default(),
                        tool_policy: Default::default(),
                        limits: Default::default(),
                    },
                    AgentRunContext::new(AgentSessionId::new(), budget),
                )
                .await
                .map_err(ReflectionError::Runtime)?;
            let session_cost = session.cost;
            let session = session.value;

            let readback = {
                let current = view.subdir(self.layout.mutable_root.clone())?;
                reflector
                    .read_back(input, &current, &session)
                    .await
                    .map_err(ReflectionError::Readback)?
            };
            let attachments = session_attachments(&session)?;
            Ok(ReflectionRunOutcome {
                readback,
                session_attachments: attachments,
                cost: session_cost,
            })
        }
        .await;
        let cleanup_result = workspace.cleanup().await;

        match (stage_result, cleanup_result) {
            (Ok(outcome), Ok(())) => Ok(outcome),
            (Ok(_), Err(cleanup)) => Err(ReflectionError::Workspace(cleanup)),
            (Err(stage), Ok(())) => Err(stage),
            (Err(stage), Err(cleanup)) => Err(ReflectionError::StageAndCleanup {
                stage: Box::new(stage),
                cleanup,
            }),
        }
    }
}

impl Default for ReflectionWorkspace {
    fn default() -> Self {
        Self::new(ReflectionLayoutConfig::default())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectionLayoutConfig {
    pub mutable_root: WorkspacePath,
    pub readonly_roots: Vec<WorkspacePath>,
    pub inline_text_threshold: usize,
    pub full_trace_top_k: usize,
}

impl Default for ReflectionLayoutConfig {
    fn default() -> Self {
        Self {
            mutable_root: WorkspacePath::new("target/current").expect("constant path is valid"),
            readonly_roots: vec![
                WorkspacePath::new("cases").expect("constant path is valid"),
                WorkspacePath::new("cross_case").expect("constant path is valid"),
                WorkspacePath::new("MANIFEST.json").expect("constant path is valid"),
                WorkspacePath::new("TASK.md").expect("constant path is valid"),
                WorkspacePath::new("AGENTS.md").expect("constant path is valid"),
                WorkspacePath::new("CLAUDE.md").expect("constant path is valid"),
            ],
            inline_text_threshold: 512,
            full_trace_top_k: 5,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReflectionRunOutcome<C> {
    pub readback: ReadbackResult<C>,
    pub session_attachments: Vec<Attachment>,
    pub cost: Cost,
}

#[derive(Debug, Error)]
pub enum ReflectionError<E: std::error::Error + Send + Sync + 'static> {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Runtime(#[from] AgentRuntimeError),
    #[error("artifact projection failed")]
    Project(#[source] E),
    #[error("artifact readback failed")]
    Readback(#[source] E),
    #[error("manifest serialization failed")]
    Manifest(#[source] serde_json::Error),
    #[error("reflection stage failed and cleanup failed")]
    StageAndCleanup {
        #[source]
        stage: Box<Self>,
        cleanup: WorkspaceError,
    },
    #[error("cancelled")]
    Cancelled,
    #[error("budget exhausted")]
    BudgetExhausted,
}

impl<E> From<leaven_engine::ProposalError> for ReflectionError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(error: leaven_engine::ProposalError) -> Self {
        Self::Workspace(WorkspaceError::Cleanup(error.to_string()))
    }
}

#[derive(Serialize)]
struct ReflectionManifest<'a> {
    kind: &'static str,
    signature_id: &'a str,
    mutable_root: &'a str,
    readonly_roots: Vec<&'a str>,
    case_count: usize,
    source_ref_count: usize,
}

fn write_workspace_contract<E>(
    view: &mut WorkspaceView<'_>,
    signature_id: &str,
    layout: &ReflectionLayoutConfig,
    cases: &[leaven_gepa::ReflectiveCase],
    source_refs: &[InfoRef],
) -> Result<(), ReflectionError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let manifest = ReflectionManifest {
        kind: "leaven.agentic_reflection_workspace.v1",
        signature_id,
        mutable_root: layout.mutable_root.as_str(),
        readonly_roots: layout
            .readonly_roots
            .iter()
            .map(WorkspacePath::as_str)
            .collect(),
        case_count: cases.len(),
        source_ref_count: source_refs.len(),
    };
    let manifest = serde_json::to_vec_pretty(&manifest).map_err(ReflectionError::Manifest)?;
    view.write_file(
        &WorkspacePath::new("MANIFEST.json").expect("constant path is valid"),
        &manifest,
    )?;
    for (index, case) in cases.iter().enumerate() {
        let path = WorkspacePath::new(format!("cases/case-{index:03}.json"))
            .expect("constant path is valid");
        let bytes = serde_json::to_vec_pretty(case).map_err(ReflectionError::Manifest)?;
        view.write_file(&path, &bytes)?;
    }
    let source_refs = serde_json::to_vec_pretty(source_refs).map_err(ReflectionError::Manifest)?;
    view.write_file(
        &WorkspacePath::new("cross_case/source_refs.json").expect("constant path is valid"),
        &source_refs,
    )?;
    view.write_file(
        &WorkspacePath::new("TASK.md").expect("constant path is valid"),
        b"# Reflection task\n\nYou are improving the artifact in `target/current`.\n\nRead `MANIFEST.json`, inspect the evidence files when present, and edit only `target/current`.\n",
    )?;
    view.write_file(
        &WorkspacePath::new("AGENTS.md").expect("constant path is valid"),
        b"# Reflection workspace rules\n\nOnly `target/current` is mutable. Evidence under `cases` and `cross_case` is read-only.\n",
    )?;
    view.write_file(
        &WorkspacePath::new("CLAUDE.md").expect("constant path is valid"),
        b"# Reflection workspace rules\n\nOnly `target/current` is mutable. Evidence under `cases` and `cross_case` is read-only.\n",
    )?;
    Ok(())
}

fn session_attachments<E>(session: &AgentSession) -> Result<Vec<Attachment>, ReflectionError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let mut attachments = vec![Attachment {
        name: "session/main".to_owned(),
        kind: AttachmentKind::Json(
            serde_json::to_value(&session.transcript).map_err(ReflectionError::Manifest)?,
        ),
        media_type: Some("application/json".to_owned()),
    }];
    if !session.commands.is_empty() {
        attachments.push(Attachment {
            name: "session/commands".to_owned(),
            kind: AttachmentKind::Json(
                serde_json::to_value(&session.commands).map_err(ReflectionError::Manifest)?,
            ),
            media_type: Some("application/json".to_owned()),
        });
    }
    if !session.raw_provider_events.is_empty() {
        attachments.push(Attachment {
            name: "session/raw_provider_events".to_owned(),
            kind: AttachmentKind::Json(
                serde_json::to_value(&session.raw_provider_events)
                    .map_err(ReflectionError::Manifest)?,
            ),
            media_type: Some("application/json".to_owned()),
        });
    }
    Ok(attachments)
}
