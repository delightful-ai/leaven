use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::configured_extension::{
    authorize_evaluation_request_write, evaluation_job_value_from_write,
    evaluation_request_plan_result, extension_result_for_plan_report,
    single_request_evaluation_write,
};
use crate::git_workspace::{
    SeamWorkspaceGitConfig, execute_git_workspace_query, initialize_workspace_git,
};
use crate::graph_state::SeamGraphState;
use crate::lm::{ConfiguredLmError, ConfiguredLmRuntime, SeamLmConfig};
use crate::run_context_service::{
    RunContextProposalApplyState, SeamRunContextConfig, requested_proposal_batch,
};
use crate::stage::SeamStageConfig;
use leaven_agent::{AgentRunContext, AgentRuntime};
use leaven_agent_codex_cli::{CodexCliApproval, CodexCliConfig, CodexCliRuntime, CodexCliSandbox};
use leaven_kernel::{AgentSessionId, BudgetSnapshot, Cost, FingerprintBuilder, Metered};
use leaven_public_seam::{
    AgentCommandOutputRefs, CapabilityDocument, LockedMethod, PlanAgentRunOutcome,
    PlanAgentRunRequest, PlanApplyProposalBatchOutcome, PlanApplyProposalBatchRequest,
    PlanCaseQueryOutcome, PlanCaseQueryRequest, PlanEmitRunEventOutcome, PlanEmitRunEventRequest,
    PlanExecutionContext, PlanExecutionHost, PlanGraphQueryOutcome, PlanGraphQueryRequest,
    PlanLmCompleteOutcome, PlanLmCompleteRequest, PlanSandboxExecOutcome, PlanSandboxExecRequest,
    PlanSubmitAssessmentsOutcome, PlanSubmitAssessmentsRequest, PlanSubmitProposalBatchOutcome,
    PlanSubmitProposalBatchRequest, PlanWorkspaceMaterializeOutcome,
    PlanWorkspaceMaterializeRequest, PlanWorkspaceQueryOutcome, PlanWorkspaceQueryRequest,
    PlanWorkspaceReleaseOutcome, PlanWorkspaceReleaseRequest, PublicSeamError, PublicSeamPackage,
};
use leaven_seam_runtime::{SeamPlanRequest, SeamService, SeamServiceError, SeamStageRunRequest};
use leaven_workspace::{Workspace, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Configured public-seam service that executes supported Plan IR effects.
#[derive(Clone, Debug)]
pub struct ConfiguredSeamService {
    package: PublicSeamPackage,
    config: SeamServiceConfig,
    capability: Option<CapabilityDocument>,
    graph_state: Arc<Mutex<SeamGraphState>>,
    run_context_state: Option<Arc<Mutex<RunContextProposalApplyState>>>,
}

impl ConfiguredSeamService {
    /// Loads the active public-seam package from a repository root.
    pub fn from_repo(
        root: impl AsRef<Path>,
        config: SeamServiceConfig,
    ) -> Result<Self, ConfiguredSeamServiceError> {
        let package = PublicSeamPackage::active_from_repo(root)?;
        Self::from_package(package, config)
    }

    /// Builds a service from an already loaded public-seam package.
    pub fn from_package(
        package: PublicSeamPackage,
        config: SeamServiceConfig,
    ) -> Result<Self, ConfiguredSeamServiceError> {
        config.lm.validate()?;
        let capability = config
            .capability
            .clone()
            .map(CapabilityDocument::from_value)
            .transpose()?;
        let run_context_state = if config.run_context.enabled {
            Some(Arc::new(Mutex::new(RunContextProposalApplyState::new(
                config.run_context.clone(),
            )?)))
        } else {
            None
        };
        Ok(Self {
            package,
            graph_state: Arc::new(Mutex::new(SeamGraphState::new(&config.graph))),
            run_context_state,
            config,
            capability,
        })
    }

    /// Service configuration.
    pub const fn config(&self) -> &SeamServiceConfig {
        &self.config
    }
}

impl SeamService for ConfiguredSeamService {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        self.execute_plan_method(request.method(), request.params())
            .map_err(|error| SeamServiceError::execution(error.to_string()))
    }

    fn handle_stage_run(
        &self,
        request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        let mut effects =
            |method: LockedMethod, params: &Value| self.execute_plan_method(method, params);
        self.config
            .stage
            .runner_result(request.params(), &mut effects)
            .map_err(|error| SeamServiceError::execution(error.to_string()))
    }
}

impl ConfiguredSeamService {
    fn execute_plan_method(
        &self,
        method: LockedMethod,
        params: &Value,
    ) -> Result<Value, PublicSeamError> {
        if method == LockedMethod::ProposalApply
            && let Some(state) = &self.run_context_state
        {
            self.package.validate_plan_document(params)?;
            let mut state = state.lock().map_err(|_| PublicSeamError::InvalidPlan {
                message: "RunContext seam service state lock poisoned".to_owned(),
            })?;
            if state.accepts_proposal_apply(params) {
                return state
                    .apply_proposal_batch(method, params, &self.config.context)
                    .and_then(|result| {
                        self.package
                            .validate_acp_extension_result_document(&result)?;
                        Ok(result)
                    });
            }
        }
        if method == LockedMethod::EventEmit
            && let Some(state) = &self.run_context_state
        {
            self.package.validate_plan_document(params)?;
            let mut state = state.lock().map_err(|_| PublicSeamError::InvalidPlan {
                message: "RunContext seam service state lock poisoned".to_owned(),
            })?;
            if state.accepts_event_emit(params) {
                return state
                    .emit_run_event(method, params, &self.config.context)
                    .and_then(|result| {
                        self.package
                            .validate_acp_extension_result_document(&result)?;
                        Ok(result)
                    });
            }
        }
        if method == LockedMethod::EvaluationRequest
            && let Some(state) = &self.run_context_state
        {
            self.package.validate_plan_document(params)?;
            let mut state = state.lock().map_err(|_| PublicSeamError::InvalidPlan {
                message: "RunContext seam service state lock poisoned".to_owned(),
            })?;
            if state.accepts_evaluation_request(params) {
                return state
                    .request_evaluation(method, params, &self.config.context)
                    .and_then(|result| {
                        self.package
                            .validate_acp_extension_result_document(&result)?;
                        Ok(result)
                    });
            }
        }
        if method == LockedMethod::AssessmentSubmit
            && let Some(state) = &self.run_context_state
        {
            self.package.validate_plan_document(params)?;
            let mut state = state.lock().map_err(|_| PublicSeamError::InvalidPlan {
                message: "RunContext seam service state lock poisoned".to_owned(),
            })?;
            if state.accepts_assessment_submit(params) {
                return state
                    .submit_assessments(method, params, &self.config.context)
                    .and_then(|result| {
                        self.package
                            .validate_acp_extension_result_document(&result)?;
                        Ok(result)
                    });
            }
        }
        if method == LockedMethod::EvaluationRequest {
            return self.execute_evaluation_request_method(method, params);
        }
        let context = self.config.context.to_execution_context();
        let mut host =
            ConfiguredPlanHost {
                lm: self.config.lm.to_lm_runtime().map_err(|error| {
                    PublicSeamError::InvalidPlan {
                        message: format!("configured LM provider failed: {error}"),
                    }
                })?,
                workspace_config: self.config.workspace.clone(),
                workspace_factory: self.config.workspace.to_factory(),
                agent: self.config.agent.to_codex_runtime(),
                graph_state: Arc::clone(&self.graph_state),
                run_context_state: self.run_context_state.clone(),
                cases: self.config.cases.clone(),
                graph_revision: self.config.context.base_revision.clone(),
                workspaces: BTreeMap::new(),
            };
        let report = match &self.capability {
            Some(capability) => self
                .package
                .execute_plan_document_with_capability(params, &context, capability, &mut host),
            None => self
                .package
                .execute_plan_document(params, &context, &mut host),
        }?;
        extension_result_for_plan_report(method, params, report.value())
    }

    fn execute_evaluation_request_method(
        &self,
        method: LockedMethod,
        params: &Value,
    ) -> Result<Value, PublicSeamError> {
        let context = self.config.context.to_execution_context();
        self.package.validate_plan_document(params)?;
        let capability = self
            .capability
            .as_ref()
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "request_evaluation execution requires capability".to_owned(),
            })?;
        let (name, write) = single_request_evaluation_write(params)?;
        authorize_evaluation_request_write(write, capability)?;
        let job_value = evaluation_job_value_from_write(write, &context)?;
        let job = self.package.validate_evaluation_job_document(&job_value)?;
        let result = evaluation_request_plan_result(params, name, &context, &job)?;
        self.package
            .validate_evaluation_request_receipt_document(&job, &result)?;
        self.graph_state
            .lock()
            .map_err(|_| PublicSeamError::InvalidPlan {
                message: "configured seam graph state lock poisoned".to_owned(),
            })?
            .record_evaluation_request(name, &job, context.base_revision());
        extension_result_for_plan_report(method, params, &result)
    }
}

/// Serve-process configuration for executable public-seam methods.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeamServiceConfig {
    /// Execution context projected into Plan Result receipts.
    pub context: SeamExecutionContextConfig,
    /// Optional capability document used for capability-bound Plan execution.
    pub capability: Option<Value>,
    /// Workspace provider configuration.
    pub workspace: SeamWorkspaceConfig,
    /// Agent provider configuration.
    pub agent: SeamAgentConfig,
    /// LM provider configuration.
    pub lm: SeamLmConfig,
    /// Stage runner configuration.
    pub stage: SeamStageConfig,
    /// Configured graph read state.
    pub graph: SeamGraphConfig,
    /// Optional RunContext-backed graph-write proof path.
    pub run_context: SeamRunContextConfig,
    /// Configured case records by case id.
    pub cases: BTreeMap<String, SeamCaseRecordConfig>,
}

impl Default for SeamServiceConfig {
    fn default() -> Self {
        Self {
            context: SeamExecutionContextConfig::default(),
            capability: None,
            workspace: SeamWorkspaceConfig::default(),
            agent: SeamAgentConfig::default(),
            lm: SeamLmConfig::default(),
            stage: SeamStageConfig::default(),
            graph: SeamGraphConfig::default(),
            run_context: SeamRunContextConfig::default(),
            cases: BTreeMap::new(),
        }
    }
}

/// Stable execution metadata for one local seam service.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeamExecutionContextConfig {
    /// Capability fingerprint used for receipts.
    pub capability_fingerprint: String,
    /// Policy fingerprint used for receipts.
    pub policy_fingerprint: String,
    /// Base graph revision used for no-write plans.
    pub base_revision: String,
    /// Execution start timestamp.
    pub started_at: String,
    /// Execution completion timestamp.
    pub completed_at: String,
    /// Optional evaluation run bound to capability-authorized case reads.
    pub evaluation_run: Option<String>,
    /// Optional evaluation request id bound to capability-authorized case reads.
    pub evaluation_request_id: Option<String>,
    /// Optional case partition bound to capability-authorized case reads.
    pub case_partition: Option<String>,
}

impl SeamExecutionContextConfig {
    fn to_execution_context(&self) -> PlanExecutionContext {
        let mut context = PlanExecutionContext::new(
            &self.capability_fingerprint,
            &self.policy_fingerprint,
            &self.base_revision,
            &self.started_at,
            &self.completed_at,
        );
        if let (Some(run), Some(request_id)) = (
            self.evaluation_run.as_deref(),
            self.evaluation_request_id.as_deref(),
        ) {
            context = context.with_evaluation_request(run, request_id);
        }
        if let Some(partition) = self.case_partition.as_deref() {
            context = context.with_case_partition(partition);
        }
        context
    }
}

impl Default for SeamExecutionContextConfig {
    fn default() -> Self {
        Self {
            capability_fingerprint: "fp_cap_sha256_leaven_seam_local".to_owned(),
            policy_fingerprint: "fp_policy_sha256_leaven_seam_local".to_owned(),
            base_revision: "rev_leaven_seam_local_base".to_owned(),
            started_at: "2026-01-01T00:00:00Z".to_owned(),
            completed_at: "2026-01-01T00:00:01Z".to_owned(),
            evaluation_run: None,
            evaluation_request_id: None,
            case_partition: None,
        }
    }
}

/// Configured graph read state for public-seam execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeamGraphConfig {
    /// Items returned by graph queries.
    pub items: Vec<Value>,
    /// Data classes attached to the graph-set value.
    pub data_classes: Vec<String>,
    /// Optional pagination cursor returned by graph queries.
    pub next_cursor: Option<String>,
}

impl Default for SeamGraphConfig {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            data_classes: vec!["public".to_owned()],
            next_cursor: None,
        }
    }
}

/// Configured case read state for public-seam execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeamCaseRecordConfig {
    /// Case identifier returned in the public `case_record`.
    pub case: String,
    /// Optional case input projection.
    pub input: Option<Value>,
    /// Optional case target projection.
    pub target: Option<Value>,
    /// Optional case metadata projection.
    pub metadata: Option<Value>,
    /// Data classes attached to the case record.
    pub data_classes: Vec<String>,
}

impl Default for SeamCaseRecordConfig {
    fn default() -> Self {
        Self {
            case: String::new(),
            input: None,
            target: None,
            metadata: None,
            data_classes: vec!["public".to_owned()],
        }
    }
}

/// Workspace provider configuration for public-seam execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeamWorkspaceConfig {
    /// Parent directory for local temp workspaces. Uses the OS temp directory when omitted.
    pub parent: Option<PathBuf>,
    /// UTF-8 seed files written into every materialized workspace.
    pub seed_files: BTreeMap<String, String>,
    /// Optional Git initialization for workspaces that must answer Git-backed queries.
    pub git: SeamWorkspaceGitConfig,
}

impl SeamWorkspaceConfig {
    fn to_factory(&self) -> LocalWorkspaceFactory {
        self.parent
            .clone()
            .map_or_else(LocalWorkspaceFactory::temp, LocalWorkspaceFactory::new)
    }
}

impl Default for SeamWorkspaceConfig {
    fn default() -> Self {
        Self {
            parent: None,
            seed_files: BTreeMap::new(),
            git: SeamWorkspaceGitConfig::default(),
        }
    }
}

/// Agent provider configuration for public-seam execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SeamAgentConfig {
    /// No agent provider is wired.
    None,
    /// Run agent sessions through the Codex CLI.
    CodexCli {
        /// Codex executable path.
        codex_bin: String,
        /// Codex model.
        model: String,
        /// Optional process timeout in seconds.
        timeout_s: Option<u64>,
        /// Optional CODEX_HOME override.
        codex_home: Option<String>,
        /// Run Codex with full bypass flags. Intended for explicit live proof only.
        bypass_approvals_and_sandbox: bool,
    },
}

impl SeamAgentConfig {
    fn to_codex_runtime(&self) -> Option<CodexCliRuntime> {
        match self {
            Self::None => None,
            Self::CodexCli {
                codex_bin,
                model,
                timeout_s,
                codex_home,
                bypass_approvals_and_sandbox,
            } => {
                let mut config = CodexCliConfig::new(codex_bin.clone());
                config.model.clone_from(model);
                config.timeout = timeout_s.map(std::time::Duration::from_secs);
                config.codex_home.clone_from(codex_home);
                if *bypass_approvals_and_sandbox {
                    config.approval = CodexCliApproval::BypassSandboxAndApprovals;
                } else {
                    config.approval = CodexCliApproval::Sandbox(CodexCliSandbox::WorkspaceWrite);
                }
                Some(CodexCliRuntime::new(config))
            }
        }
    }
}

impl Default for SeamAgentConfig {
    fn default() -> Self {
        Self::None
    }
}

struct ConfiguredPlanHost {
    lm: ConfiguredLmRuntime,
    workspace_config: SeamWorkspaceConfig,
    workspace_factory: LocalWorkspaceFactory,
    agent: Option<CodexCliRuntime>,
    graph_state: Arc<Mutex<SeamGraphState>>,
    run_context_state: Option<Arc<Mutex<RunContextProposalApplyState>>>,
    cases: BTreeMap<String, SeamCaseRecordConfig>,
    graph_revision: String,
    workspaces: BTreeMap<String, Workspace>,
}

impl PlanExecutionHost for ConfiguredPlanHost {
    fn graph_query(
        &mut self,
        request: PlanGraphQueryRequest<'_>,
    ) -> Result<PlanGraphQueryOutcome, PublicSeamError> {
        if let Some(state) = &self.run_context_state {
            let state = state.lock().map_err(|_| PublicSeamError::InvalidPlan {
                message: "RunContext seam service state lock poisoned".to_owned(),
            })?;
            if state.accepts_graph_query_plan_id(request.expr()) {
                return Ok(state.graph_query(request));
            }
        }
        Ok(self
            .graph_state
            .lock()
            .map_err(|_| PublicSeamError::InvalidPlan {
                message: "configured seam graph state lock poisoned".to_owned(),
            })?
            .query(request.scope(), &self.graph_revision))
    }

    fn case_query_load(
        &mut self,
        request: PlanCaseQueryRequest<'_>,
    ) -> Result<PlanCaseQueryOutcome, PublicSeamError> {
        let include = request
            .query()
            .get("include")
            .and_then(Value::as_array)
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "case_query.load must carry include fields".to_owned(),
            })?;
        let includes = |field: &str| include.iter().any(|value| value.as_str() == Some(field));
        let case_id = request
            .query()
            .get("case")
            .and_then(|case| case.get("id"))
            .and_then(Value::as_str)
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "case_query.load must carry case.id".to_owned(),
            })?;
        let record = self
            .cases
            .get(case_id)
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: format!("case `{case_id}` is not configured"),
            })?;
        let data_classes = record
            .data_classes
            .iter()
            .filter(|data_class| match data_class.as_str() {
                "case.input" => includes("input"),
                "case.target" => includes("target"),
                "case.metadata" => includes("metadata"),
                _ => true,
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut outcome = PlanCaseQueryOutcome::new(&record.case, self.graph_revision.clone())
            .with_data_classes(data_classes);
        if includes("input") {
            let input = record
                .input
                .as_ref()
                .ok_or_else(|| PublicSeamError::InvalidPlan {
                    message: format!("case `{case_id}` has no configured input"),
                })?;
            outcome = outcome.with_input(input.clone());
        }
        if includes("target") {
            let target = record
                .target
                .as_ref()
                .ok_or_else(|| PublicSeamError::InvalidPlan {
                    message: format!("case `{case_id}` has no configured target"),
                })?;
            outcome = outcome.with_target(target.clone());
        }
        if includes("metadata") {
            let metadata =
                record
                    .metadata
                    .as_ref()
                    .ok_or_else(|| PublicSeamError::InvalidPlan {
                        message: format!("case `{case_id}` has no configured metadata"),
                    })?;
            outcome = outcome.with_metadata(metadata.clone());
        }
        Ok(outcome)
    }

    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        block_on_configured_provider(request.execute_with_lm(&self.lm))
    }

    fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> Result<PlanEmitRunEventOutcome, PublicSeamError> {
        Ok(self
            .graph_state
            .lock()
            .map_err(|_| PublicSeamError::InvalidPlan {
                message: "configured seam graph state lock poisoned".to_owned(),
            })?
            .emit_run_event(request))
    }

    fn submit_proposal_batch(
        &mut self,
        request: PlanSubmitProposalBatchRequest<'_>,
    ) -> Result<PlanSubmitProposalBatchOutcome, PublicSeamError> {
        self.graph_state
            .lock()
            .map_err(|_| PublicSeamError::InvalidPlan {
                message: "configured seam graph state lock poisoned".to_owned(),
            })?
            .submit_proposal_batch(request)
    }

    fn apply_proposal_batch(
        &mut self,
        request: PlanApplyProposalBatchRequest<'_>,
    ) -> Result<PlanApplyProposalBatchOutcome, PublicSeamError> {
        if let Some(state) = &self.run_context_state {
            let state = state.lock().map_err(|_| PublicSeamError::InvalidPlan {
                message: "RunContext seam service state lock poisoned".to_owned(),
            })?;
            let batch_ref = requested_proposal_batch(&request)?;
            if state.accepts_proposal_batch_ref(batch_ref) {
                return Err(PublicSeamError::InvalidPlan {
                    message: "RunContext proposal.apply must be executed by the service dispatcher"
                        .to_owned(),
                });
            }
        }
        self.graph_state
            .lock()
            .map_err(|_| PublicSeamError::InvalidPlan {
                message: "configured seam graph state lock poisoned".to_owned(),
            })?
            .apply_proposal_batch(request)
    }

    fn submit_assessments(
        &mut self,
        request: PlanSubmitAssessmentsRequest<'_>,
    ) -> Result<PlanSubmitAssessmentsOutcome, PublicSeamError> {
        self.graph_state
            .lock()
            .map_err(|_| PublicSeamError::InvalidPlan {
                message: "configured seam graph state lock poisoned".to_owned(),
            })?
            .submit_assessments(request)
    }

    fn workspace_materialize(
        &mut self,
        request: PlanWorkspaceMaterializeRequest<'_>,
    ) -> Result<PlanWorkspaceMaterializeOutcome, PublicSeamError> {
        let workspace_id = materialized_workspace_id(request.candidate()?);
        let mut workspace = block_on_configured_provider(
            self.workspace_factory.allocate(WorkspaceConfig::default()),
        )
        .map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("local workspace allocation failed: {error}"),
        })?;
        {
            let mut view = workspace.view();
            for (path, contents) in &self.workspace_config.seed_files {
                let path =
                    WorkspacePath::new(path).map_err(|error| PublicSeamError::InvalidPlan {
                        message: format!("invalid configured seed file path `{path}`: {error}"),
                    })?;
                view.write_file(&path, contents.as_bytes())
                    .map_err(|error| PublicSeamError::InvalidPlan {
                        message: format!("failed to write seed file `{}`: {error}", path.as_str()),
                    })?;
            }
            if self.workspace_config.git.initialize {
                initialize_workspace_git(&mut view, &self.workspace_config.git)?;
            }
        }
        self.workspaces.insert(workspace_id.clone(), workspace);
        Ok(PlanWorkspaceMaterializeOutcome::new(
            workspace_id,
            request.lifetime()?,
            "fp_runtime_sha256_leaven_local_workspace",
        ))
    }

    fn workspace_release(
        &mut self,
        request: PlanWorkspaceReleaseRequest<'_>,
    ) -> Result<PlanWorkspaceReleaseOutcome, PublicSeamError> {
        let workspace_id = request.live_workspace()?.to_owned();
        let lifetime = request
            .deps()
            .values()
            .find(|value| {
                value.get("workspace").and_then(Value::as_str) == Some(workspace_id.as_str())
            })
            .and_then(|value| value.get("lifetime"))
            .and_then(Value::as_str)
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: format!("workspace `{workspace_id}` missing live lifetime"),
            })?
            .to_owned();
        self.workspaces
            .remove(&workspace_id)
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: format!("workspace `{workspace_id}` is not materialized"),
            })?;
        Ok(PlanWorkspaceReleaseOutcome::new(
            workspace_id,
            lifetime,
            "fp_runtime_sha256_leaven_local_workspace",
        ))
    }

    fn workspace_query(
        &mut self,
        request: PlanWorkspaceQueryRequest<'_>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        let workspace_id = request.workspace().to_owned();
        let workspace =
            self.workspaces
                .get_mut(&workspace_id)
                .ok_or_else(|| PublicSeamError::InvalidPlan {
                    message: format!("workspace `{workspace_id}` is not materialized"),
                })?;
        let expected = request.expected_data_classes();
        let data_classes = if expected.is_empty() {
            vec!["workspace.file".to_owned()]
        } else {
            expected.into_iter().map(str::to_owned).collect()
        };
        let mut view = workspace.view();
        if matches!(request.op_kind(), "git_log" | "git_diff" | "git_status") {
            execute_git_workspace_query(
                &request,
                &mut view,
                self.graph_revision.clone(),
                data_classes,
            )
        } else {
            request.execute_on_workspace_view(&view, self.graph_revision.clone(), data_classes)
        }
    }

    fn sandbox_exec(
        &mut self,
        request: PlanSandboxExecRequest<'_>,
    ) -> Result<PlanSandboxExecOutcome, PublicSeamError> {
        let name = request.name().to_owned();
        let workspace_id = request.live_workspace()?.to_owned();
        let workspace =
            self.workspaces
                .get_mut(&workspace_id)
                .ok_or_else(|| PublicSeamError::InvalidPlan {
                    message: format!("workspace `{workspace_id}` is not materialized"),
                })?;
        let mut view = workspace.view();
        let output = view
            .run_command(request.into_workspace_command())
            .map_err(|error| PublicSeamError::InvalidPlan {
                message: format!(
                    "sandbox_exec command failed in workspace `{workspace_id}`: {error}"
                ),
            })?;
        let stdout_ref = blob_ref_for_bytes(
            &format!("blob_{name}_sandbox_stdout"),
            &output.stdout.bytes,
            &["transcript.raw"],
        );
        let stderr_ref = blob_ref_for_bytes(
            &format!("blob_{name}_sandbox_stderr"),
            &output.stderr.bytes,
            &["transcript.raw"],
        );
        let file_refs = output
            .output_files
            .iter()
            .map(|(path, captured)| {
                (
                    path.clone(),
                    blob_ref_for_bytes(
                        &format!(
                            "blob_{name}_sandbox_file_{}",
                            sanitize_blob_id(path.as_str())
                        ),
                        &captured.bytes,
                        &["workspace.file"],
                    ),
                )
            })
            .collect::<Vec<_>>();
        PlanSandboxExecOutcome::from_command_output_with_file_refs(
            Metered::new(
                output,
                Cost::custom("sandbox_calls", 1.0).map_err(|error| {
                    PublicSeamError::InvalidPlan {
                        message: format!("failed to record sandbox cost: {error}"),
                    }
                })?,
            ),
            configured_sandbox_fingerprint(),
            stdout_ref,
            stderr_ref,
            file_refs,
        )
    }

    fn agent_run(
        &mut self,
        request: PlanAgentRunRequest<'_>,
    ) -> Result<PlanAgentRunOutcome, PublicSeamError> {
        let name = request.name().to_owned();
        let workspace_id = request.live_workspace()?.to_owned();
        let agent = self
            .agent
            .as_ref()
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "configured seam service does not provide an agent runtime".to_owned(),
            })?;
        if let Some(runtime) = request.runtime()
            && runtime.as_str() != agent.id().as_str()
        {
            return Err(PublicSeamError::InvalidPlan {
                message: format!(
                    "configured agent runtime `{}` cannot satisfy requested runtime `{}`",
                    agent.id().as_str(),
                    runtime.as_str()
                ),
            });
        }
        let workspace =
            self.workspaces
                .get_mut(&workspace_id)
                .ok_or_else(|| PublicSeamError::InvalidPlan {
                    message: format!("workspace `{workspace_id}` is not materialized"),
                })?;
        let agent_request = request.into_agent_run_request();
        let budget = BudgetSnapshot::default();
        let mut view = workspace.view();
        let session = block_on_configured_provider(agent.run_session(
            &mut view,
            agent_request,
            AgentRunContext::new(AgentSessionId::new(), &budget),
        ))
        .map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("agent runtime failed: {error}"),
        })?;
        let transcript_bytes = serde_json::to_vec(&session.value.transcript).map_err(|error| {
            PublicSeamError::InvalidPlan {
                message: format!("agent transcript serialization failed: {error}"),
            }
        })?;
        let command_refs = session
            .value
            .commands
            .iter()
            .enumerate()
            .map(|(index, command)| {
                AgentCommandOutputRefs::new(
                    blob_ref_for_bytes(
                        &format!("blob_{name}_command_{index}_stdout"),
                        &command.output.stdout.bytes,
                        &["transcript.raw"],
                    ),
                    blob_ref_for_bytes(
                        &format!("blob_{name}_command_{index}_stderr"),
                        &command.output.stderr.bytes,
                        &["transcript.raw"],
                    ),
                )
            })
            .collect::<Vec<_>>();
        PlanAgentRunOutcome::from_agent_session_with_command_output_refs(
            session,
            agent.fingerprint(),
            blob_ref_for_bytes(
                &format!("blob_{name}_transcript"),
                &transcript_bytes,
                &["transcript.raw"],
            ),
            format!("agentrec_{name}"),
            command_refs,
        )
    }
}

fn materialized_workspace_id(candidate: &str) -> String {
    let stem = candidate.strip_prefix("cand_").unwrap_or(candidate);
    let sanitized = sanitize_id_fragment(stem);
    format!("ws_{sanitized}_materialized")
}

fn sanitize_id_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn sanitize_blob_id(value: &str) -> String {
    sanitize_id_fragment(value)
}

fn configured_sandbox_fingerprint() -> leaven_kernel::Fingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"leaven-seam-service.local-sandbox.v1");
    builder.finish()
}

fn blob_ref_for_bytes(id: &str, bytes: &[u8], data_classes: &[&str]) -> Value {
    serde_json::json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": format!("{:x}", Sha256::digest(bytes)),
        "bytes": bytes.len(),
        "data_classes": data_classes
    })
}

fn block_on_configured_provider<F>(future: F) -> F::Output
where
    F: Future,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("configured seam provider runtime builds")
        .block_on(future)
}

/// Error while constructing a configured public-seam service.
#[derive(Debug, thiserror::Error)]
pub enum ConfiguredSeamServiceError {
    /// The public-seam package could not be loaded.
    #[error(transparent)]
    PublicSeam(#[from] PublicSeamError),
    /// Capability document parsing failed.
    #[error(transparent)]
    Capability(#[from] leaven_public_seam::CapabilityError),
    /// LM provider configuration failed validation.
    #[error(transparent)]
    LmConfig(#[from] ConfiguredLmError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    use leaven_seam_runtime::{JsonRpcErrorCode, SeamRuntime};
    use serde_json::{Value, json};

    use super::{
        ConfiguredSeamService, SeamAgentConfig, SeamCaseRecordConfig, SeamExecutionContextConfig,
        SeamGraphConfig, SeamServiceConfig, SeamStageConfig, SeamWorkspaceConfig,
    };
    use crate::SeamWorkspaceGitConfig;
    use crate::lm::{MockLmResponseConfig, SeamLmConfig};

    #[test]
    fn seam_runtime_executes_lm_complete_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                lm: SeamLmConfig::Mock {
                    responses: vec![MockLmResponseConfig {
                        text: "configured service ok".to_owned(),
                        input_tokens: 7,
                        output_tokens: 3,
                    }],
                },
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&lm_complete_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["message"]["content"][0]["text"],
            "configured service ok"
        );
        assert_eq!(
            response.value()["result"]["primary"]["cost"],
            json!({
                "input_tokens": 7,
                "output_tokens": 3,
                "lm_calls": 1
            })
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["call_kind"],
            "lm_complete"
        );
    }

    #[test]
    fn seam_runtime_executes_lm_complete_through_openai_provider_config() {
        let server = FakeOpenAiServer::start(
            json!({
                "id": "resp_seam_service",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "live provider seam ok"}]
                }],
                "usage": {
                    "input_tokens": 11,
                    "output_tokens": 4
                }
            })
            .to_string(),
        );
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                lm: SeamLmConfig::OpenAi {
                    api_key_env: "PATH".to_owned(),
                    base_url: Some(server.url()),
                    timeout_s: Some(5),
                    max_retries: Some(0),
                },
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&lm_complete_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["message"]["content"][0]["text"],
            "live provider seam ok"
        );
        assert_eq!(
            response.value()["result"]["primary"]["cost"],
            json!({
                "input_tokens": 11,
                "output_tokens": 4,
                "lm_calls": 1
            })
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["call_kind"],
            "lm_complete"
        );
        let request_body = server.request_body();
        assert_eq!(request_body["model"], "gpt-4.1-mini");
        assert_eq!(request_body["input"][0]["content"], "solve");
    }

    #[test]
    fn seam_runtime_reports_provider_execution_failure_distinct_from_unwired_method() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service =
            ConfiguredSeamService::from_package(package.clone(), SeamServiceConfig::default())
                .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&two_call_request_with_one_mock_response());

        assert!(response.is_error());
        assert_eq!(
            response.value()["error"]["code"],
            JsonRpcErrorCode::ExecutionFailed.code()
        );
        assert!(
            response.value()["error"]["message"]
                .as_str()
                .unwrap()
                .contains("mock script exhausted")
        );
    }

    #[test]
    fn seam_runtime_executes_proposal_submit_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: proposal_context(),
                capability: Some(proposal_capability()),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&proposal_submit_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["kind"],
            "proposal_batch_receipt"
        );
        assert_eq!(
            response.value()["result"]["primary"]["receipt"],
            "wrec_proposal_batch"
        );
        assert_eq!(
            response.value()["result"]["primary"]["proposal_ids"],
            json!(["prop_proposal_batch_0"])
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["write_kind"],
            "submit_proposal_batch"
        );
    }

    #[test]
    fn seam_runtime_executes_proposal_apply_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: proposal_context(),
                capability: Some(proposal_apply_capability()),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&proposal_apply_request());

        assert!(
            !response.is_error(),
            "unexpected proposal apply error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["kind"],
            "apply_receipt"
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["write_kind"],
            "apply_proposal_batch"
        );
        assert_eq!(
            response.value()["result"]["primary"]["created_candidates"][0],
            "cand_pb_service_apply_applied"
        );
    }

    #[test]
    fn seam_runtime_executes_assessment_submit_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: planexec_context(),
                capability: Some(assessment_submit_capability()),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&assessment_submit_request());

        assert!(
            !response.is_error(),
            "unexpected assessment submit error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["kind"],
            "assessment_batch_receipt"
        );
        assert_eq!(
            response.value()["result"]["primary"]["evaluation_request_id"],
            "evalreq_service"
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["write_kind"],
            "submit_assessments"
        );
    }

    #[test]
    fn seam_runtime_executes_evaluation_request_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: planexec_context(),
                capability: Some(evaluation_request_capability()),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&evaluation_request_request());

        assert!(
            !response.is_error(),
            "unexpected evaluation request error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["kind"],
            "evaluation_request_receipt"
        );
        assert_eq!(
            response.value()["result"]["primary"]["evaluation_request_id"],
            "evalreq_configured"
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["write_kind"],
            "request_evaluation"
        );
    }

    #[test]
    fn seam_runtime_denies_evaluation_request_without_configured_authority() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: planexec_context(),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&evaluation_request_request());

        assert!(
            response.is_error(),
            "evaluation.request should require a grant"
        );
        assert!(
            response.value()["error"]["message"]
                .as_str()
                .unwrap()
                .contains("requires capability"),
            "unexpected denial response: {:?}",
            response.value()
        );
    }

    #[test]
    fn seam_runtime_executes_workspace_release_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: planexec_context(),
                capability: Some(effect_capability()),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&workspace_release_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["kind"],
            "workspace_handle"
        );
        assert_eq!(response.value()["result"]["primary"]["released"], true);
        assert_eq!(
            response.value()["result"]["primary"]["workspace"],
            "ws_planexec_materialized"
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["call_kind"],
            "workspace_materialize"
        );
        assert_eq!(
            response.value()["result"]["receipts"][1]["call_kind"],
            "workspace_release"
        );
    }

    #[test]
    fn seam_runtime_executes_finite_workspace_queries_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: planexec_context(),
                capability: Some(effect_capability()),
                workspace: seeded_workspace_config(),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        for case in finite_workspace_query_cases() {
            let response = runtime.handle_value(&workspace_query_request(&case));
            assert!(
                !response.is_error(),
                "unexpected {} error: {:?}",
                case.method,
                response.value()
            );
            assert_eq!(
                response.value()["result"]["primary"]["kind"],
                case.primary_kind
            );
            assert_eq!(
                response.value()["result"]["receipts"][0]["call_kind"],
                "workspace_materialize"
            );
            assert_eq!(response.value()["result"]["receipts"][1]["kind"], "query");
            (case.assert_primary)(response.value());
        }
    }

    #[test]
    fn seam_runtime_executes_graph_and_case_reads_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: graph_case_context(),
                capability: Some(graph_case_capability()),
                graph: graph_config(),
                cases: case_config(),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        for request in graph_case_read_requests() {
            let response = runtime.handle_value(&request);
            assert!(
                !response.is_error(),
                "unexpected graph/case response for {:?}: {:?}",
                request["method"],
                response.value()
            );
            let method = request["method"].as_str().unwrap();
            match method {
                "leaven/graph.query" => {
                    assert_eq!(response.value()["result"]["primary"]["kind"], "graph_set");
                    assert_eq!(
                        response.value()["result"]["primary"]["items"][0]["event_kind"],
                        "case.loaded"
                    );
                }
                "leaven/case.input" => {
                    assert_eq!(response.value()["result"]["primary"]["kind"], "case_record");
                    assert_eq!(
                        response.value()["result"]["primary"]["input"]["question"],
                        "2 + 3"
                    );
                    assert!(
                        response.value()["result"]["primary"]
                            .get("target")
                            .is_none()
                    );
                }
                "leaven/case.target" => {
                    assert_eq!(response.value()["result"]["primary"]["target"]["answer"], 5);
                    assert!(response.value()["result"]["primary"].get("input").is_none());
                }
                "leaven/case.metadata" => {
                    assert_eq!(
                        response.value()["result"]["primary"]["metadata"]["partition"],
                        "validation"
                    );
                }
                "leaven/case.load" => {
                    assert_eq!(
                        response.value()["result"]["primary"]["input"]["question"],
                        "2 + 3"
                    );
                    assert_eq!(response.value()["result"]["primary"]["target"]["answer"], 5);
                    assert_eq!(
                        response.value()["result"]["primary"]["metadata"]["partition"],
                        "validation"
                    );
                }
                other => panic!("unexpected graph/case request {other}"),
            }
            assert_eq!(response.value()["result"]["receipts"][0]["kind"], "query");
        }
    }

    #[test]
    fn seam_runtime_denies_case_reads_without_configured_case_authority() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: graph_case_context(),
                graph: graph_config(),
                cases: case_config(),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&case_query_request(
            "leaven/case.target",
            "case_target",
            &["target"],
        ));

        assert!(
            response.is_error(),
            "case.read should be denied without a grant"
        );
        assert!(
            response.value()["error"]["message"]
                .as_str()
                .unwrap()
                .contains(
                    "case_query.load execution requires capability-authorized Plan execution"
                ),
            "unexpected denial response: {:?}",
            response.value()
        );
    }

    #[test]
    fn seam_runtime_executes_event_emit_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: planexec_context(),
                capability: Some(effect_capability()),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&event_emit_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["kind"],
            "emit_run_event"
        );
        assert_eq!(
            response.value()["result"]["primary"]["event_id"],
            "event_status"
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["write_kind"],
            "emit_run_event"
        );
    }

    #[test]
    fn seam_runtime_executes_sandbox_exec_in_materialized_workspace() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: planexec_context(),
                capability: Some(effect_capability()),
                workspace: seeded_workspace_config(),
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&sandbox_exec_request());
        assert!(
            !response.is_error(),
            "unexpected sandbox response: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["kind"],
            "sandbox_exec"
        );
        assert_eq!(response.value()["result"]["primary"]["exit_code"], 0);
        assert_eq!(
            response.value()["result"]["primary"]["stdout_ref"]["bytes"],
            "sandbox stdout\n".len()
        );
        assert_eq!(
            response.value()["result"]["primary"]["files"]["reports/out.txt"]["bytes"],
            "sandbox artifact\n".len()
        );
        assert_eq!(
            response.value()["result"]["receipts"][1]["call_kind"],
            "sandbox_exec"
        );
    }

    #[test]
    fn seam_runtime_executes_configured_runner_stage_dispatch() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                stage: SeamStageConfig::MockRunner {
                    text: "runner durable seam ok".to_owned(),
                    summary: "configured runner output".to_owned(),
                },
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&stage_run_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(response.value()["result"]["message"], "stage_run_result");
        assert_eq!(
            response.value()["result"]["stage_call_id"],
            "sc_runner_service"
        );
        assert_eq!(
            response.value()["result"]["output"]["value"],
            "runner durable seam ok"
        );
        assert_eq!(
            response.value()["result"]["output"]["data_classes"],
            json!(["candidate.output"])
        );
    }

    #[test]
    fn seam_runtime_executes_runner_stage_through_command_worker() {
        let fake_worker = fake_stage_worker_bin();
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                stage: SeamStageConfig::CommandRunner {
                    argv: vec![fake_worker.display().to_string()],
                },
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&stage_run_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(response.value()["result"]["message"], "stage_run_result");
        assert_eq!(
            response.value()["result"]["stage_call_id"],
            "sc_runner_service"
        );
        assert_eq!(
            response.value()["result"]["output"]["value"],
            "runner command worker ok"
        );
        assert_eq!(
            response.value()["result"]["output"]["summary"],
            "command worker output"
        );
    }

    #[test]
    fn seam_runtime_services_lm_callback_from_command_worker() {
        let fake_worker = fake_stage_worker_with_lm_callback_bin();
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                lm: SeamLmConfig::Mock {
                    responses: vec![MockLmResponseConfig {
                        text: "callback lm ok".to_owned(),
                        input_tokens: 5,
                        output_tokens: 2,
                    }],
                },
                stage: SeamStageConfig::CommandRunner {
                    argv: vec![fake_worker.display().to_string()],
                },
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&stage_run_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["output"]["value"],
            "runner callback saw callback lm ok"
        );
    }

    #[test]
    fn seam_runtime_services_agent_callback_from_command_worker() {
        let fake_worker = fake_stage_worker_with_agent_callback_bin();
        let fake_codex = fake_codex_bin();
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: planexec_context(),
                capability: Some(effect_capability()),
                agent: SeamAgentConfig::CodexCli {
                    codex_bin: fake_codex.display().to_string(),
                    model: "gpt-5.4-mini".to_owned(),
                    timeout_s: Some(5),
                    codex_home: None,
                    bypass_approvals_and_sandbox: false,
                },
                stage: SeamStageConfig::CommandRunner {
                    argv: vec![fake_worker.display().to_string()],
                },
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&stage_run_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["output"]["value"],
            "runner agent callback saw agentrec_completion"
        );
    }

    #[test]
    fn seam_runtime_services_proposal_submit_callback_from_command_worker() {
        let fake_worker = fake_stage_worker_with_proposal_callback_bin();
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: proposal_context(),
                capability: Some(proposal_capability()),
                stage: SeamStageConfig::CommandRunner {
                    argv: vec![fake_worker.display().to_string()],
                },
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&proposer_stage_run_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(response.value()["result"]["stage"], "proposer");
        assert_eq!(
            response.value()["result"]["output"]["value"],
            "proposer callback saw wrec_proposal_batch"
        );
    }

    #[test]
    fn seam_runtime_reports_unconfigured_runner_as_execution_failure() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service =
            ConfiguredSeamService::from_package(package.clone(), SeamServiceConfig::default())
                .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&stage_run_request());

        assert!(response.is_error());
        assert_eq!(
            response.value()["error"]["code"],
            JsonRpcErrorCode::ExecutionFailed.code()
        );
        assert!(
            response.value()["error"]["message"]
                .as_str()
                .unwrap()
                .contains("does not provide a stage runner")
        );
    }

    #[test]
    fn seam_runtime_executes_agent_run_in_materialized_workspace_through_codex_adapter() {
        let fake_codex = fake_codex_bin();
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let plan = agent_run_request();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                context: planexec_context(),
                capability: Some(effect_capability()),
                agent: SeamAgentConfig::CodexCli {
                    codex_bin: fake_codex.display().to_string(),
                    model: "gpt-5.4-mini".to_owned(),
                    timeout_s: Some(5),
                    codex_home: None,
                    bypass_approvals_and_sandbox: false,
                },
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&plan);

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["kind"],
            "agent_session"
        );
        assert_eq!(response.value()["result"]["primary"]["status"], "completed");
        assert_eq!(
            response.value()["result"]["primary"]["receipt"],
            "agentrec_completion"
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["call_kind"],
            "workspace_materialize"
        );
        assert_eq!(
            response.value()["result"]["receipts"][1]["call_kind"],
            "agent_run"
        );
        assert_eq!(
            response.value()["result"]["primary"]["commands"][1]["status"],
            "completed"
        );
        assert!(
            response.value()["result"]["primary"]["transcript_ref"]["bytes"]
                .as_u64()
                .unwrap()
                > 100
        );
    }

    fn lm_complete_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "lm-1",
            "method": "leaven/lm.complete",
            "params": lm_plan()
        })
    }

    struct FakeOpenAiServer {
        url: String,
        request_rx: mpsc::Receiver<Value>,
    }

    impl FakeOpenAiServer {
        fn start(response_body: String) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let (request_tx, request_rx) = mpsc::channel();
            std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0_u8; 16 * 1024];
                let read = stream.read(&mut buffer).unwrap();
                let raw = String::from_utf8_lossy(&buffer[..read]);
                let body = raw.split("\r\n\r\n").nth(1).unwrap_or_default();
                request_tx
                    .send(serde_json::from_str(body).unwrap())
                    .unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                stream.write_all(response.as_bytes()).unwrap();
            });
            Self {
                url: format!("http://{addr}/v1/responses"),
                request_rx,
            }
        }

        fn url(&self) -> String {
            self.url.clone()
        }

        fn request_body(self) -> Value {
            self.request_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
        }
    }

    fn two_call_request_with_one_mock_response() -> Value {
        let mut plan = lm_plan();
        let second = plan["ops"][0].clone();
        plan["ops"].as_array_mut().unwrap().push(second);
        plan["ops"][1]["name"] = json!("completion_2");
        plan["ops"][1]["idempotency_key"] = json!("lm-service-0002");
        plan["return"] = json!(["completion", "completion_2"]);
        json!({
            "jsonrpc": "2.0",
            "id": "lm-2",
            "method": "leaven/lm.complete",
            "params": plan
        })
    }

    fn agent_run_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "agent-1",
            "method": "leaven/agent.run",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": "planagentservice001",
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [
                    {
                        "kind": "call",
                        "name": "workspace",
                        "idempotency_key": "agent-service-0001",
                        "call": {
                            "kind": "workspace_materialize",
                            "candidate": "cand_planexec",
                            "surface": "program",
                            "mode": "copy_on_write",
                            "lifetime": "manual_release"
                        }
                    },
                    {
                        "kind": "call",
                        "name": "completion",
                        "deps": ["workspace"],
                        "idempotency_key": "agent-service-0002",
                        "call": {
                            "kind": "agent_run",
                            "runtime": "codex-cli",
                            "workspace": "ws_planexec_materialized",
                            "instructions": {
                                "system": "Stay within the workspace.",
                                "task": "Write a short final answer."
                            },
                            "tool_policy": {
                                "allow_shell": false
                            },
                            "output": {
                                "kind": "final_message",
                                "max_bytes": 1024
                            },
                            "limits": {
                                "timeout_s": 5,
                                "max_turns": 1,
                                "max_usd_micro": 1000
                            },
                            "input_classes": ["public"]
                        }
                    }
                ],
                "return": ["workspace", "completion"],
                "commit": {
                    "kind": "graph_writes_atomic",
                    "on_stale": "reject"
                }
            }
        })
    }

    fn workspace_release_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "workspace-release-1",
            "method": "leaven/workspace.release",
            "params": workspace_release_plan()
        })
    }

    fn workspace_release_plan() -> Value {
        json!({
            "schema_version": "leaven.plan.v1",
            "plan_id": "planworkspacereleaseservice001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [
                {
                    "kind": "call",
                    "name": "workspace",
                    "idempotency_key": "workspace-release-service-0001",
                    "call": {
                        "kind": "workspace_materialize",
                        "candidate": "cand_planexec",
                        "surface": "program",
                        "mode": "copy_on_write",
                        "lifetime": "manual_release"
                    }
                },
                {
                    "kind": "call",
                    "name": "release",
                    "deps": ["workspace"],
                    "idempotency_key": "workspace-release-service-0002",
                    "call": {
                        "kind": "workspace_release",
                        "workspace": "ws_planexec_materialized",
                        "force": false
                    }
                }
            ],
            "return": ["release"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        })
    }

    struct WorkspaceQueryCase {
        method: &'static str,
        name: &'static str,
        op: Value,
        primary_kind: &'static str,
        assert_primary: fn(&Value),
    }

    fn finite_workspace_query_cases() -> Vec<WorkspaceQueryCase> {
        vec![
            WorkspaceQueryCase {
                method: "leaven/workspace.read_file",
                name: "file",
                op: json!({
                    "kind": "read_file",
                    "path": "README.md",
                    "expected_data_classes": ["workspace.file"]
                }),
                primary_kind: "workspace_file",
                assert_primary: |value| {
                    assert_eq!(
                        value["result"]["primary"]["content"],
                        "seeded workspace readme\n"
                    );
                },
            },
            WorkspaceQueryCase {
                method: "leaven/workspace.list",
                name: "listing",
                op: json!({"kind": "list", "path": ".", "recursive": false, "max_entries": 10}),
                primary_kind: "workspace_listing",
                assert_primary: |value| {
                    assert_eq!(
                        value["result"]["primary"]["entries"][0]["path"],
                        "README.md"
                    );
                },
            },
            WorkspaceQueryCase {
                method: "leaven/workspace.stat",
                name: "stat",
                op: json!({"kind": "stat", "path": "README.md"}),
                primary_kind: "workspace_listing",
                assert_primary: |value| {
                    assert_eq!(value["result"]["primary"]["entries"][0]["bytes"], 24);
                },
            },
            WorkspaceQueryCase {
                method: "leaven/workspace.digest",
                name: "digest",
                op: json!({"kind": "digest", "path": "README.md", "algorithm": "sha256"}),
                primary_kind: "workspace_snapshot",
                assert_primary: |value| {
                    assert!(
                        value["result"]["primary"]["digest"]
                            .as_str()
                            .unwrap()
                            .starts_with("sha256:")
                    );
                },
            },
            WorkspaceQueryCase {
                method: "leaven/workspace.snapshot",
                name: "snapshot",
                op: json!({"kind": "snapshot"}),
                primary_kind: "workspace_snapshot",
                assert_primary: |value| {
                    assert!(
                        value["result"]["primary"]["digest"]
                            .as_str()
                            .unwrap()
                            .starts_with("blake3:")
                    );
                },
            },
            WorkspaceQueryCase {
                method: "leaven/workspace.capture_artifacts",
                name: "captured",
                op: json!({"kind": "capture_artifacts", "paths": ["README.md"], "max_bytes": 4096}),
                primary_kind: "workspace_listing",
                assert_primary: |value| {
                    assert_eq!(value["result"]["primary"]["entries"][0]["bytes"], 24);
                    assert_eq!(
                        value["result"]["primary"]["entries"][0]["content_base64"],
                        "c2VlZGVkIHdvcmtzcGFjZSByZWFkbWUK"
                    );
                    assert_eq!(
                        value["result"]["primary"]["entries"][0]["blob_ref"]["bytes"],
                        24
                    );
                    assert_eq!(
                        value["result"]["primary"]["entries"][0]["blob_ref"]["sha256"],
                        value["result"]["primary"]["entries"][0]["sha256"]
                    );
                },
            },
            WorkspaceQueryCase {
                method: "leaven/workspace.git_log",
                name: "gitlog",
                op: json!({"kind": "git_log", "max_entries": 5}),
                primary_kind: "workspace_diff",
                assert_primary: |value| {
                    assert!(
                        value["result"]["primary"]["text"]
                            .as_str()
                            .unwrap()
                            .contains("leaven workspace seed")
                    );
                    assert_eq!(
                        value["result"]["primary"]["source_refs"][0]["namespace"],
                        "leaven.workspace.git_log.max_entries"
                    );
                },
            },
            WorkspaceQueryCase {
                method: "leaven/workspace.git_diff",
                name: "gitdiff",
                op: json!({"kind": "git_diff", "against": "seed", "max_bytes": 4096}),
                primary_kind: "workspace_diff",
                assert_primary: |value| {
                    let text = value["result"]["primary"]["text"].as_str().unwrap();
                    assert!(text.contains("-pub fn answer() -> u8 { 42 }"));
                    assert!(text.contains("+pub fn answer() -> u8 { 43 }"));
                    assert_eq!(
                        value["result"]["primary"]["source_refs"][0]["namespace"],
                        "leaven.workspace.git_diff.against"
                    );
                },
            },
            WorkspaceQueryCase {
                method: "leaven/workspace.git_status",
                name: "gitstatus",
                op: json!({"kind": "git_status", "porcelain": true}),
                primary_kind: "workspace_diff",
                assert_primary: |value| {
                    assert!(
                        value["result"]["primary"]["text"]
                            .as_str()
                            .unwrap()
                            .contains(" M src/lib.rs")
                    );
                    assert_eq!(
                        value["result"]["primary"]["source_refs"][0]["namespace"],
                        "leaven.workspace.git_status.porcelain"
                    );
                },
            },
        ]
    }

    fn workspace_query_request(case: &WorkspaceQueryCase) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": format!("workspace-query-{}", case.name),
            "method": case.method,
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": format!("planworkspacequery{}", case.name),
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [
                    workspace_materialize_op("workspace-query-service-0001"),
                    {
                        "kind": "let",
                        "name": case.name,
                        "deps": ["workspace"],
                        "expr": {
                            "kind": "workspace_query",
                            "workspace": "ws_planexec_materialized",
                            "op": case.op
                        }
                    }
                ],
                "return": [case.name],
                "commit": {
                    "kind": "graph_writes_atomic",
                    "on_stale": "reject"
                }
            }
        })
    }

    fn event_emit_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "event-emit-1",
            "method": "leaven/event.emit",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": "planeventemitservice001",
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [
                    {
                        "kind": "write",
                        "name": "status",
                        "idempotency_key": "event-emit-service-0001",
                        "write": {
                            "kind": "emit_run_event",
                            "event_kind": "service.checked",
                            "payload_schema": "fp_schema_sha256_event",
                            "payload": {
                                "ok": true
                            },
                            "visibility": "public"
                        }
                    }
                ],
                "return": ["status"],
                "commit": {
                    "kind": "graph_writes_atomic",
                    "on_stale": "reject"
                }
            }
        })
    }

    fn sandbox_exec_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "sandbox-exec-1",
            "method": "leaven/sandbox.exec",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": "plansandboxexecservice001",
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [
                    workspace_materialize_op("sandbox-exec-service-0001"),
                    {
                        "kind": "call",
                        "name": "sandboxed",
                        "deps": ["workspace"],
                        "idempotency_key": "sandbox-exec-service-0002",
                        "call": {
                            "kind": "sandbox_exec",
                            "workspace": "ws_planexec_materialized",
                            "argv": [
                                "sh",
                                "-c",
                                "mkdir -p reports && printf 'sandbox artifact\n' > reports/out.txt && printf 'sandbox stdout\n'"
                            ],
                            "timeout_s": 5,
                            "output": {
                                "kind": "files",
                                "paths": ["reports/out.txt"],
                                "max_bytes": 4096
                            },
                            "stream_policy": "blob_refs_only",
                            "input_classes": ["public"]
                        }
                    }
                ],
                "return": ["sandboxed"],
                "commit": {
                    "kind": "graph_writes_atomic",
                    "on_stale": "reject"
                }
            }
        })
    }

    fn workspace_materialize_op(idempotency_key: &str) -> Value {
        json!({
            "kind": "call",
            "name": "workspace",
            "idempotency_key": idempotency_key,
            "call": {
                "kind": "workspace_materialize",
                "candidate": "cand_planexec",
                "surface": "program",
                "mode": "copy_on_write",
                "lifetime": "manual_release"
            }
        })
    }

    fn seeded_workspace_config() -> SeamWorkspaceConfig {
        SeamWorkspaceConfig {
            parent: None,
            seed_files: BTreeMap::from([
                (
                    "README.md".to_owned(),
                    "seeded workspace readme\n".to_owned(),
                ),
                (
                    "src/lib.rs".to_owned(),
                    "pub fn answer() -> u8 { 42 }\n".to_owned(),
                ),
            ]),
            git: SeamWorkspaceGitConfig {
                initialize: true,
                post_commit_files: BTreeMap::from([(
                    "src/lib.rs".to_owned(),
                    "pub fn answer() -> u8 { 43 }\n".to_owned(),
                )]),
            },
        }
    }

    fn stage_run_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "stage-1",
            "method": "leaven/stage.run",
            "params": {
                "schema_version": "leaven.stage_run.v1",
                "message": "stage_run_request",
                "stage": "runner",
                "payload": {
                    "schema_version": "leaven.stage_payloads.v1",
                    "role": "runner",
                    "run": "run_stage_service",
                    "stage_call_id": "sc_runner_service",
                    "candidate": "cand_stage_service",
                    "case": "case_stage_service",
                    "case_input": {"question": "2 + 2"},
                    "target_forbidden": true
                }
            }
        })
    }

    fn proposer_stage_run_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "stage-proposer-1",
            "method": "leaven/stage.run",
            "params": {
                "schema_version": "leaven.stage_run.v1",
                "message": "stage_run_request",
                "stage": "proposer",
                "payload": {
                    "schema_version": "leaven.stage_payloads.v1",
                    "role": "proposer",
                    "run": "run_proposal",
                    "stage_call_id": "sc_proposer_service",
                    "base_revision": "rev_proposal_base",
                    "parent": "cand_proposal_parent",
                    "surface_fingerprint": "fp_surface_sha256_program",
                    "reflection_result": {
                        "schema_version": "leaven.stage_payloads.v1",
                        "role": "reflection_result",
                        "summary": "empty inputs fail",
                        "failure_modes": [
                            {
                                "label": "missing_empty_input_guard",
                                "description": "empty inputs fail",
                                "source_refs": ["cand_proposal_parent"]
                            }
                        ],
                        "surface_suggestions": [],
                        "negative_constraints": [],
                        "positive_constraints": [],
                        "source_refs": ["cand_proposal_parent"],
                        "read_receipts": ["qrec_reflection"],
                        "data_classes": ["optimizer.visible"],
                        "confidence": 0.8
                    },
                    "allowed_effects": ["change_from_agent_session"],
                    "allowed_change_schemas": ["fp_schema_sha256_skill_patch"],
                    "source_refs": ["cand_proposal_parent"],
                    "query_policy_fingerprint": "fp_policy_sha256_proposal",
                    "capability_fingerprint": "fp_cap_sha256_proposal"
                }
            }
        })
    }

    fn proposal_submit_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "proposal-1",
            "method": "leaven/proposal.submit_batch",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": "planproposalservice001",
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [
                    {
                        "kind": "write",
                        "name": "proposal_batch",
                        "idempotency_key": "proposal-service-0001",
                        "write": {
                            "kind": "submit_proposal_batch",
                            "semantics": "sequence",
                            "proposals": [
                                {
                                    "effect": {
                                        "kind": "change_from_agent_session",
                                        "target": "cand_parent",
                                        "agent_receipt": "agentrec_codex",
                                        "parser": "leaven.agent_session.skill_patch.v1",
                                        "surface_fingerprint": "fp_surface_sha256_program",
                                        "change_schema": "fp_schema_sha256_skill_patch"
                                    },
                                    "causal": {
                                        "inputs": ["cand_parent"]
                                    },
                                    "informed_by": {
                                        "kind": "literal",
                                        "value": ["qrec_reflection", "agentrec_codex"]
                                    },
                                    "read_receipts": ["qrec_reflection", "agentrec_codex"]
                                }
                            ]
                        }
                    }
                ],
                "return": ["proposal_batch"],
                "commit": {
                    "kind": "graph_writes_atomic",
                    "on_stale": "reject"
                }
            }
        })
    }

    fn proposal_apply_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "proposal-apply-1",
            "method": "leaven/proposal.apply",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": "planproposalapplyservice001",
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [{
                    "kind": "write",
                    "name": "applied",
                    "idempotency_key": "proposal-apply-service-0001",
                    "write": {
                        "kind": "apply_proposal_batch",
                        "proposal_batch": "pb_service_apply",
                        "policy": "apply_first_valid"
                    }
                }],
                "return": ["applied"],
                "commit": {
                    "kind": "graph_writes_atomic",
                    "on_stale": "reject"
                }
            }
        })
    }

    fn assessment_submit_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "assessment-submit-1",
            "method": "leaven/assessment.submit",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": "planassessmentsubmitservice001",
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [{
                    "kind": "write",
                    "name": "assessments",
                    "idempotency_key": "assessment-submit-service-0001",
                    "write": {
                        "kind": "submit_assessments",
                        "evaluation_request_id": "evalreq_service",
                        "assessments": [{
                            "kind": "independent",
                            "candidate": "cand_a",
                            "target": {
                                "case": "case_1"
                            },
                            "score": {
                                "value": 1.0,
                                "output": {
                                    "kind": "structured",
                                    "summary": "candidate answered correctly",
                                    "value": {
                                        "candidate": "cand_a",
                                        "output": "candidate answered correctly"
                                    },
                                    "visibility": "public",
                                    "data_classes": ["candidate.output"]
                                }
                            },
                            "evidence": {
                                "schema_version": "leaven.evidence_envelope.v1",
                                "target_derived": false,
                                "public": {
                                    "summary": "candidate answered correctly",
                                    "data_classes": ["public"]
                                },
                                "redaction_policy": {
                                    "optimizer": "score_only",
                                    "reflector": "score_only",
                                    "operator": "score_only"
                                },
                                "producer": {
                                    "stage_call_id": "sc_assessment_service"
                                },
                                "source_receipts": {
                                    "read": ["qrec_assessment_source"],
                                    "effect": []
                                }
                            },
                            "replayability": "pure_read"
                        }]
                    }
                }],
                "return": ["assessments"],
                "commit": {
                    "kind": "graph_writes_atomic",
                    "on_stale": "reject"
                }
            }
        })
    }

    fn evaluation_request_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "evaluation-request-1",
            "method": "leaven/evaluation.request",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": "planevaluationrequestservice001",
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [{
                    "kind": "write",
                    "name": "evaluation",
                    "idempotency_key": "evaluation-request-service-0001",
                    "write": {
                        "kind": "request_evaluation",
                        "request": {
                            "shape": "independent",
                            "candidates": ["cand_a"],
                            "set": {
                                "kind": "named",
                                "name": "validation"
                            },
                            "granularity": "per_case",
                            "purpose": "validation",
                            "evaluator": "eval_configured"
                        }
                    }
                }],
                "return": ["evaluation"],
                "commit": {
                    "kind": "graph_writes_atomic",
                    "on_stale": "reject"
                }
            }
        })
    }

    fn lm_plan() -> Value {
        json!({
            "schema_version": "leaven.plan.v1",
            "plan_id": "planlmservice001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [
                {
                    "kind": "call",
                    "name": "completion",
                    "idempotency_key": "lm-service-0001",
                    "call": {
                        "kind": "lm_complete",
                        "purpose": "test.seam_service",
                        "model": "gpt-4.1-mini",
                        "messages": [
                            {
                                "role": "developer",
                                "content": [{"kind": "text", "text": "return the final answer"}]
                            },
                            {
                                "role": "user",
                                "content": [{"kind": "text", "text": "solve"}]
                            }
                        ],
                        "output": {
                            "kind": "final_message",
                            "max_bytes": 256
                        },
                        "input_classes": ["public"]
                    }
                }
            ],
            "return": ["completion"],
            "commit": {
                "kind": "no_graph_writes"
            }
        })
    }

    fn repo_root() -> &'static std::path::Path {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap()
    }

    fn planexec_context() -> SeamExecutionContextConfig {
        SeamExecutionContextConfig {
            capability_fingerprint: "fp_cap_sha256_planexec".to_owned(),
            policy_fingerprint: "fp_policy_sha256_planexec".to_owned(),
            base_revision: "rev_planexec_base".to_owned(),
            started_at: "2026-05-23T00:00:00Z".to_owned(),
            completed_at: "2026-05-23T00:00:01Z".to_owned(),
            evaluation_run: None,
            evaluation_request_id: None,
            case_partition: None,
        }
    }

    fn proposal_context() -> SeamExecutionContextConfig {
        SeamExecutionContextConfig {
            capability_fingerprint: "fp_cap_sha256_proposal".to_owned(),
            policy_fingerprint: "fp_policy_sha256_proposal".to_owned(),
            base_revision: "rev_proposal_base".to_owned(),
            started_at: "2026-06-03T00:00:00Z".to_owned(),
            completed_at: "2026-06-03T00:00:01Z".to_owned(),
            evaluation_run: None,
            evaluation_request_id: None,
            case_partition: None,
        }
    }

    fn graph_case_context() -> SeamExecutionContextConfig {
        SeamExecutionContextConfig {
            evaluation_run: Some("run_demo".to_owned()),
            evaluation_request_id: Some("evalreq_01".to_owned()),
            case_partition: Some("validation".to_owned()),
            ..planexec_context()
        }
    }

    fn graph_config() -> SeamGraphConfig {
        SeamGraphConfig {
            items: vec![json!({
                "kind": "event_summary",
                "event_kind": "case.loaded",
                "revision": "rev_tip"
            })],
            data_classes: vec!["public".to_owned()],
            next_cursor: None,
        }
    }

    fn case_config() -> BTreeMap<String, SeamCaseRecordConfig> {
        BTreeMap::from([(
            "case_1".to_owned(),
            SeamCaseRecordConfig {
                case: "case_1".to_owned(),
                input: Some(json!({"question": "2 + 3"})),
                target: Some(json!({"answer": 5})),
                metadata: Some(json!({"partition": "validation"})),
                data_classes: vec![
                    "case.input".to_owned(),
                    "case.target".to_owned(),
                    "case.metadata".to_owned(),
                ],
            },
        )])
    }

    fn graph_case_capability() -> Value {
        json!({
            "schema_version": "leaven.capability.v1",
            "jti": "jti_graph_case_authority",
            "capability_fingerprint": "fp_cap_sha256_planexec",
            "policy_fingerprint": "fp_policy_sha256_planexec",
            "subject_fingerprint": "fp_subject_sha256_planexec",
            "issuer": {
                "kind": "run_engine",
                "id": "engine_local"
            },
            "subject": {
                "kind": "stage_call",
                "run": "run_demo",
                "stage_call_id": "sc_graph_case",
                "role": "scorer"
            },
            "audience": ["leaven.acp.worker"],
            "issued_at": "2026-05-23T00:00:00Z",
            "expires_at": "2026-05-23T00:20:00Z",
            "expiry_behavior": "drain_inflight_no_new_ops",
            "token_binding": {
                "kind": "opaque_lookup",
                "token_id": "ltok_graph_case"
            },
            "revocation": {
                "mode": "issuer_epoch",
                "revocation_epoch": 7,
                "check": "on_every_request"
            },
            "renewal": {
                "mode": "renew_before_expiry",
                "max_extensions": 0,
                "max_total_lifetime_s": 1200
            },
            "grants": [{
                "action": "case.read",
                "resource": {
                    "run": "run_demo",
                    "evaluation_request_id": "evalreq_01"
                },
                "constraints": {
                    "case_fields": ["input", "target", "metadata"],
                    "partitions": ["validation"],
                    "allowed_input_classes": ["case.input", "case.target", "case.metadata"]
                }
            }],
            "budgets": {},
            "execution_policy": {
                "profile": "managed_sandbox",
                "network": "leaven_endpoint_only",
                "subprocess": "deny_except_sandbox_exec",
                "filesystem": "workspace_handles_only",
                "byo_effects": "forbidden"
            },
            "delegation": {
                "may_delegate": false,
                "max_depth": 0,
                "must_attenuate": true,
                "allowed_actions": []
            }
        })
    }

    fn graph_case_read_requests() -> Vec<Value> {
        vec![
            graph_query_request(),
            case_query_request(
                "leaven/case.load",
                "case_load",
                &["input", "target", "metadata"],
            ),
            case_query_request("leaven/case.input", "case_input", &["input"]),
            case_query_request("leaven/case.target", "case_target", &["target"]),
            case_query_request("leaven/case.metadata", "case_metadata", &["metadata"]),
        ]
    }

    fn graph_query_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "graph-query-service",
            "method": "leaven/graph.query",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": "plangraphqueryservice001",
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [{
                    "kind": "let",
                    "name": "events",
                    "expr": {
                        "kind": "graph_query",
                        "source": {
                            "kind": "events"
                        },
                        "projection": {
                            "kind": "ids"
                        },
                        "page": {
                            "limit": 100
                        }
                    }
                }],
                "return": ["events"],
                "commit": {
                    "kind": "no_graph_writes"
                }
            }
        })
    }

    fn case_query_request(method: &str, name: &str, include: &[&str]) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": format!("{name}-service"),
            "method": method,
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": format!("plan{name}service001"),
                "consistency": {
                    "kind": "latest_at_start"
                },
                "mode": {
                    "kind": "execute"
                },
                "ops": [{
                    "kind": "let",
                    "name": name,
                    "expr": {
                        "kind": "case_query",
                        "query": {
                            "kind": "load",
                            "case": {
                                "kind": "case",
                                "run": "run_demo",
                                "id": "case_1"
                            },
                            "include": include,
                            "projection_schema": "fp_schema_sha256_case_projection"
                        }
                    }
                }],
                "return": [name],
                "commit": {
                    "kind": "no_graph_writes"
                }
            }
        })
    }

    fn proposal_capability() -> Value {
        json!({
            "schema_version": "leaven.capability.v1",
            "jti": "jti_proposal_submit_authority",
            "capability_fingerprint": "fp_cap_sha256_proposal",
            "policy_fingerprint": "fp_policy_sha256_proposal",
            "subject_fingerprint": "fp_subject_sha256_proposal",
            "issuer": {
                "kind": "run_engine",
                "id": "engine_local"
            },
            "subject": {
                "kind": "stage_call",
                "run": "run_proposal",
                "stage_call_id": "sc_proposal_submit",
                "role": "proposer"
            },
            "audience": ["leaven.acp.worker"],
            "issued_at": "2026-06-03T00:00:00Z",
            "expires_at": "2026-06-03T00:20:00Z",
            "expiry_behavior": "drain_inflight_no_new_ops",
            "token_binding": {
                "kind": "opaque_lookup",
                "token_id": "ltok_proposal_submit"
            },
            "revocation": {
                "mode": "issuer_epoch",
                "revocation_epoch": 7,
                "check": "on_every_request"
            },
            "renewal": {
                "mode": "renew_before_expiry",
                "max_extensions": 0,
                "max_total_lifetime_s": 1200
            },
            "grants": [
                {
                    "action": "proposal.submit_batch",
                    "resource": {},
                    "constraints": {
                        "effects": ["change_from_agent_session"],
                        "allowed_surfaces": ["fp_surface_sha256_program"],
                        "change_schemas": ["fp_schema_sha256_skill_patch"]
                    }
                }
            ],
            "budgets": {},
            "execution_policy": {
                "profile": "managed_sandbox",
                "network": "leaven_endpoint_only",
                "subprocess": "deny_except_sandbox_exec",
                "filesystem": "workspace_handles_only",
                "byo_effects": "forbidden"
            },
            "delegation": {
                "may_delegate": false,
                "max_depth": 0,
                "must_attenuate": true,
                "allowed_actions": []
            }
        })
    }

    fn proposal_apply_capability() -> Value {
        let mut value = proposal_capability();
        value["jti"] = json!("jti_proposal_apply_authority");
        value["subject"]["stage_call_id"] = json!("sc_proposal_apply");
        value["token_binding"]["token_id"] = json!("ltok_proposal_apply");
        value["grants"] = json!([{
            "action": "proposal.apply_batch",
            "resource": {},
            "constraints": {
                "may_apply": true
            }
        }]);
        value
    }

    fn assessment_submit_capability() -> Value {
        json!({
            "schema_version": "leaven.capability.v1",
            "jti": "jti_assessment_submit_authority",
            "capability_fingerprint": "fp_cap_sha256_planexec",
            "policy_fingerprint": "fp_policy_sha256_planexec",
            "subject_fingerprint": "fp_subject_sha256_planexec",
            "issuer": {
                "kind": "run_engine",
                "id": "engine_local"
            },
            "subject": {
                "kind": "stage_call",
                "run": "run_demo",
                "stage_call_id": "sc_assessment_submit",
                "role": "scorer"
            },
            "audience": ["leaven.acp.worker"],
            "issued_at": "2026-05-23T00:00:00Z",
            "expires_at": "2026-05-23T00:20:00Z",
            "expiry_behavior": "drain_inflight_no_new_ops",
            "token_binding": {
                "kind": "opaque_lookup",
                "token_id": "ltok_assessment_submit"
            },
            "revocation": {
                "mode": "issuer_epoch",
                "revocation_epoch": 7,
                "check": "on_every_request"
            },
            "renewal": {
                "mode": "renew_before_expiry",
                "max_extensions": 0,
                "max_total_lifetime_s": 1200
            },
            "grants": [{
                "action": "assessment.submit",
                "resource": {
                    "evaluation_request_id": "evalreq_service"
                },
                "constraints": {},
                "limits": {
                    "max_rows": 1
                }
            }],
            "budgets": {},
            "execution_policy": {
                "profile": "managed_sandbox",
                "network": "leaven_endpoint_only",
                "subprocess": "deny_except_sandbox_exec",
                "filesystem": "workspace_handles_only",
                "byo_effects": "forbidden"
            },
            "delegation": {
                "may_delegate": false,
                "max_depth": 0,
                "must_attenuate": true,
                "allowed_actions": []
            }
        })
    }

    fn evaluation_request_capability() -> Value {
        let mut value = assessment_submit_capability();
        value["jti"] = json!("jti_evaluation_request_authority");
        value["subject"]["stage_call_id"] = json!("sc_evaluation_request");
        value["token_binding"]["token_id"] = json!("ltok_evaluation_request");
        value["grants"] = json!([{
            "action": "evaluation.request",
            "resource": {
                "candidate_ids": ["cand_a"]
            },
            "constraints": {
                "purposes": ["validation"]
            }
        }]);
        value
    }

    fn effect_capability() -> Value {
        json!({
            "schema_version": "leaven.capability.v1",
            "jti": "jti_planexec_call_authority",
            "capability_fingerprint": "fp_cap_sha256_planexec",
            "policy_fingerprint": "fp_policy_sha256_planexec",
            "subject_fingerprint": "fp_subject_sha256_planexec",
            "issuer": {
                "kind": "run_engine",
                "id": "engine_local"
            },
            "subject": {
                "kind": "stage_call",
                "run": "run_demo",
                "stage_call_id": "sc_planexec_call_authority",
                "role": "scorer"
            },
            "audience": ["leaven.acp.worker"],
            "issued_at": "2026-05-23T00:00:00Z",
            "expires_at": "2026-05-23T00:20:00Z",
            "expiry_behavior": "drain_inflight_no_new_ops",
            "token_binding": {
                "kind": "opaque_lookup",
                "token_id": "ltok_planexec_call_authority"
            },
            "revocation": {
                "mode": "issuer_epoch",
                "revocation_epoch": 7,
                "check": "on_every_request"
            },
            "renewal": {
                "mode": "renew_before_expiry",
                "max_extensions": 2,
                "max_total_lifetime_s": 3600
            },
            "grants": [
                {
                    "action": "workspace.materialize",
                    "resource": {
                        "candidate_ids": ["cand_planexec"]
                    },
                    "constraints": {
                        "workspace_ops": ["materialize"]
                    }
                },
                {
                    "action": "workspace.release",
                    "resource": {
                        "workspace_ids": ["ws_planexec_materialized"]
                    },
                    "constraints": {
                        "workspace_ops": ["release"]
                    }
                },
                {
                    "action": "workspace.read",
                    "resource": {
                        "workspace_ids": ["ws_planexec_materialized"]
                    },
                    "constraints": {
                        "allowed_input_classes": ["candidate.artifact", "workspace.file"],
                        "workspace_ops": [
                            "read_file",
                            "list",
                            "stat",
                            "digest",
                            "snapshot",
                            "capture_artifacts",
                            "git_log",
                            "git_diff",
                            "git_status"
                        ]
                    }
                },
                {
                    "action": "event.emit",
                    "resource": {},
                    "constraints": {}
                },
                {
                    "action": "sandbox.exec",
                    "resource": {
                        "workspace_ids": ["ws_planexec_materialized"]
                    },
                    "constraints": {
                        "allowed_input_classes": ["public"],
                        "workspace_ops": ["exec"],
                        "allowed_commands": ["sh"]
                    },
                    "limits": {
                        "timeout_s": 5
                    }
                },
                {
                    "action": "agent.run",
                    "resource": {
                        "workspace_ids": ["ws_planexec_materialized"]
                    },
                    "constraints": {
                        "allowed_input_classes": ["public"]
                    },
                    "limits": {
                        "timeout_s": 30,
                        "max_usd_micro": 1000
                    }
                }
            ],
            "budgets": {},
            "execution_policy": {
                "profile": "managed_sandbox",
                "network": "leaven_endpoint_only",
                "subprocess": "deny_except_sandbox_exec",
                "filesystem": "workspace_handles_only",
                "byo_effects": "forbidden"
            },
            "delegation": {
                "may_delegate": false,
                "max_depth": 0,
                "must_attenuate": true,
                "allowed_actions": []
            }
        })
    }

    fn fake_codex_bin() -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap().keep();
        let path = dir.join("fake-codex");
        std::fs::write(
            &path,
            r#"#!/bin/sh
last=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    shift
    last="$1"
  fi
  shift || true
done
mkdir -p "$(dirname "$last")"
printf 'fake codex final\n' > "$last"
printf '{"type":"message","content":"ok"}\n'
"#,
        )
        .unwrap();
        make_executable(&path);
        path
    }

    fn fake_stage_worker_bin() -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap().keep();
        let path = dir.join("fake-stage-worker");
        std::fs::write(
            &path,
            r#"#!/bin/sh
read line
case "$line" in
  *'"method":"leaven/stage.run"'*) ;;
  *) printf 'unexpected request: %s\n' "$line" >&2; exit 17 ;;
esac
printf '%s\n' '{"jsonrpc":"2.0","id":"stage-worker-1","result":{"schema_version":"leaven.stage_run.v1","message":"stage_run_result","stage":"runner","stage_call_id":"sc_runner_service","output":{"kind":"text","summary":"command worker output","value":"runner command worker ok","visibility":"optimizer_visible","data_classes":["candidate.output"]}}}'
"#,
        )
        .unwrap();
        make_executable(&path);
        path
    }

    fn fake_stage_worker_with_lm_callback_bin() -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap().keep();
        let path = dir.join("fake-stage-worker-lm");
        std::fs::write(
            &path,
            r#"#!/usr/bin/env python3
import json
import select
import sys

stage = json.loads(sys.stdin.readline())
if stage.get("method") != "leaven/stage.run":
    raise SystemExit(f"unexpected stage request: {stage!r}")
callback = {
    "jsonrpc": "2.0",
    "id": "worker-lm-1",
    "method": "leaven/lm.complete",
    "params": {
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_worker_lm_callback",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "call",
            "name": "completion",
            "idempotency_key": "worker-lm-1",
            "call": {
                "kind": "lm_complete",
                "purpose": "test.command_worker",
                "model": "mock",
                "messages": [{
                    "role": "user",
                    "content": [{"kind": "text", "text": "callback prompt"}]
                }],
                "output": {"kind": "final_message", "max_bytes": 128},
                "input_classes": ["public"]
            }
        }],
        "return": ["completion"],
        "commit": {"kind": "no_graph_writes"}
    }
}
print(json.dumps(callback), flush=True)
ready, _, _ = select.select([sys.stdin], [], [], 5)
if not ready:
    raise SystemExit("timed out waiting for lm.complete callback response")
lm_response = json.loads(sys.stdin.readline())
text = lm_response["result"]["primary"]["message"]["content"][0]["text"]
result = {
    "schema_version": "leaven.stage_run.v1",
    "message": "stage_run_result",
    "stage": "runner",
    "stage_call_id": stage["params"]["payload"]["stage_call_id"],
    "output": {
        "kind": "text",
        "summary": "command worker callback output",
        "value": f"runner callback saw {text}",
        "visibility": "optimizer_visible",
        "data_classes": ["candidate.output"]
    }
}
print(json.dumps({"jsonrpc": "2.0", "id": stage.get("id"), "result": result}), flush=True)
"#,
        )
        .unwrap();
        make_executable(&path);
        path
    }

    fn fake_stage_worker_with_agent_callback_bin() -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap().keep();
        let path = dir.join("fake-stage-worker-agent");
        std::fs::write(
            &path,
            r#"#!/usr/bin/env python3
import json
import select
import sys

stage = json.loads(sys.stdin.readline())
if stage.get("method") != "leaven/stage.run":
    raise SystemExit(f"unexpected stage request: {stage!r}")
callback = {
    "jsonrpc": "2.0",
    "id": "worker-agent-1",
    "method": "leaven/agent.run",
    "params": {
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_worker_agent_callback",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [
            {
                "kind": "call",
                "name": "workspace",
                "idempotency_key": "worker-agent-workspace",
                "call": {
                    "kind": "workspace_materialize",
                    "candidate": "cand_planexec",
                    "surface": "program",
                    "mode": "copy_on_write",
                    "lifetime": "manual_release"
                }
            },
            {
                "kind": "call",
                "name": "completion",
                "deps": ["workspace"],
                "idempotency_key": "worker-agent-run",
                "call": {
                    "kind": "agent_run",
                    "runtime": "codex-cli",
                    "workspace": "ws_planexec_materialized",
                    "instructions": {"task": "write final answer"},
                    "tool_policy": {"allow_shell": False},
                    "output": {"kind": "final_message", "max_bytes": 128},
                    "limits": {"timeout_s": 5, "max_turns": 1, "max_usd_micro": 1000},
                    "input_classes": ["public"]
                }
            }
        ],
        "return": ["workspace", "completion"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"}
    }
}
print(json.dumps(callback), flush=True)
ready, _, _ = select.select([sys.stdin], [], [], 5)
if not ready:
    raise SystemExit("timed out waiting for agent.run callback response")
agent_response = json.loads(sys.stdin.readline())
receipt = agent_response["result"]["primary"]["receipt"]
result = {
    "schema_version": "leaven.stage_run.v1",
    "message": "stage_run_result",
    "stage": "runner",
    "stage_call_id": stage["params"]["payload"]["stage_call_id"],
    "output": {
        "kind": "text",
        "summary": "command worker agent callback output",
        "value": f"runner agent callback saw {receipt}",
        "visibility": "optimizer_visible",
        "data_classes": ["candidate.output"]
    }
}
print(json.dumps({"jsonrpc": "2.0", "id": stage.get("id"), "result": result}), flush=True)
"#,
        )
        .unwrap();
        make_executable(&path);
        path
    }

    fn fake_stage_worker_with_proposal_callback_bin() -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap().keep();
        let path = dir.join("fake-stage-worker-proposal");
        std::fs::write(
            &path,
            r#"#!/usr/bin/env python3
import json
import select
import sys

stage = json.loads(sys.stdin.readline())
if stage.get("method") != "leaven/stage.run":
    raise SystemExit(f"unexpected stage request: {stage!r}")
callback = {
    "jsonrpc": "2.0",
    "id": "worker-proposal-1",
    "method": "leaven/proposal.submit_batch",
    "params": {
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_worker_proposal_callback",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "proposal_batch",
            "idempotency_key": "worker-proposal-submit",
            "write": {
                "kind": "submit_proposal_batch",
                "semantics": "sequence",
                "proposals": [{
                    "effect": {
                        "kind": "change_from_agent_session",
                        "target": "cand_proposal_parent",
                        "agent_receipt": "agentrec_codex",
                        "parser": "leaven.agent_session.skill_patch.v1",
                        "surface_fingerprint": "fp_surface_sha256_program",
                        "change_schema": "fp_schema_sha256_skill_patch"
                    },
                    "causal": {"inputs": ["cand_proposal_parent"]},
                    "informed_by": {"kind": "literal", "value": ["qrec_reflection", "agentrec_codex"]},
                    "read_receipts": ["qrec_reflection", "agentrec_codex"]
                }]
            }
        }],
        "return": ["proposal_batch"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"}
    }
}
print(json.dumps(callback), flush=True)
ready, _, _ = select.select([sys.stdin], [], [], 5)
if not ready:
    raise SystemExit("timed out waiting for proposal.submit_batch callback response")
proposal_response = json.loads(sys.stdin.readline())
receipt = proposal_response["result"]["primary"]["receipt"]
result = {
    "schema_version": "leaven.stage_run.v1",
    "message": "stage_run_result",
    "stage": "proposer",
    "stage_call_id": stage["params"]["payload"]["stage_call_id"],
    "output": {
        "kind": "text",
        "summary": "command worker proposal callback output",
        "value": f"proposer callback saw {receipt}",
        "visibility": "optimizer_visible",
        "data_classes": ["public"]
    }
}
print(json.dumps({"jsonrpc": "2.0", "id": stage.get("id"), "result": result}), flush=True)
"#,
        )
        .unwrap();
        make_executable(&path);
        path
    }

    #[cfg(unix)]
    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &std::path::Path) {}
}
