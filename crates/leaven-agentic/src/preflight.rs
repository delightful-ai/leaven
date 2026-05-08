//! Deterministic preflight checks for agentic runs.

use leaven_agent::{AgentRuntime, OutputContract};
use leaven_core::Artifact;
use leaven_engine::MaterializeContext;
use leaven_kernel::{CandidateId, Fingerprint};
use leaven_workspace::{WithWorkspaceError, WorkspaceConfig, WorkspaceFactory};

use crate::{AgentCase, AgentCasePresentationInput, AgentCasePresenter, AgentWorkload, CaseSuite};

/// Preflight report for an agentic run configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgentRunPreflightReport {
    findings: Vec<PreflightFinding>,
}

impl AgentRunPreflightReport {
    /// Appends an OK finding.
    pub fn ok(&mut self, check: impl Into<String>, message: impl Into<String>) -> &mut Self {
        self.push(PreflightSeverity::Ok, check, message)
    }

    /// Appends a warning finding.
    pub fn warn(&mut self, check: impl Into<String>, message: impl Into<String>) -> &mut Self {
        self.push(PreflightSeverity::Warning, check, message)
    }

    /// Appends an error finding.
    pub fn error(&mut self, check: impl Into<String>, message: impl Into<String>) -> &mut Self {
        self.push(PreflightSeverity::Error, check, message)
    }

    /// Returns all findings.
    #[must_use]
    pub fn findings(&self) -> &[PreflightFinding] {
        &self.findings
    }

    /// Returns true if any check failed.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity == PreflightSeverity::Error)
    }

    fn push(
        &mut self,
        severity: PreflightSeverity,
        check: impl Into<String>,
        message: impl Into<String>,
    ) -> &mut Self {
        self.findings.push(PreflightFinding {
            severity,
            check: check.into(),
            message: message.into(),
        });
        self
    }
}

/// One actionable preflight finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreflightFinding {
    pub severity: PreflightSeverity,
    pub check: String,
    pub message: String,
}

/// Severity of a preflight finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreflightSeverity {
    Ok,
    Warning,
    Error,
}

/// Deterministic preflight builder.
#[derive(Clone, Debug, Default)]
pub struct AgentRunPreflight {
    report: AgentRunPreflightReport,
}

impl AgentRunPreflight {
    /// Starts an empty preflight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks artifact validation.
    #[must_use]
    pub fn artifact<A>(mut self, artifact: &A) -> Self
    where
        A: Artifact,
    {
        match artifact.validate() {
            Ok(()) => {
                self.report.ok("artifact", "artifact validates");
            }
            Err(error) => {
                self.report
                    .error("artifact", format!("artifact validation failed: {error}"));
            }
        }
        self
    }

    /// Checks workload case and partition shape.
    #[must_use]
    pub fn workload(self, workload: &AgentWorkload) -> Self {
        self.case_suite(workload.cases())
    }

    /// Checks case-suite shape.
    #[must_use]
    pub fn case_suite(mut self, suite: &CaseSuite) -> Self {
        if suite.is_empty() {
            self.report.error("cases", "case suite has no cases");
        } else {
            self.report
                .ok("cases", format!("{} case(s) loaded", suite.cases().len()));
        }

        if suite.partitions().named().is_empty() {
            self.report
                .error("partitions", "case suite has no partitions");
        } else if suite.partitions().has_empty_partition() {
            self.report
                .error("partitions", "one or more case partitions are empty");
        } else {
            self.report.ok(
                "partitions",
                format!(
                    "{} partition(s) configured",
                    suite.partitions().named().len()
                ),
            );
        }

        if suite.cases().values().any(|case| case.target.is_hidden()) {
            self.report.warn(
                "trust",
                "hidden targets are scorer-visible and must not be materialized by presenters",
            );
        } else {
            self.report.ok("trust", "no hidden case targets configured");
        }

        self.report.ok(
            "case-fingerprint",
            format!(
                "case suite fingerprint {}",
                short_fingerprint(suite.fingerprint())
            ),
        );

        self
    }

    /// Checks runtime identity without running a provider session.
    #[must_use]
    pub fn runtime<R>(mut self, runtime: &R) -> Self
    where
        R: AgentRuntime,
    {
        let capabilities = runtime.capabilities();
        self.report.ok(
            "runtime",
            format!(
                "{} fingerprint {}",
                runtime.id(),
                short_fingerprint(runtime.fingerprint())
            ),
        );
        self.report.ok(
            "runtime-capabilities",
            format!("workspace access {:?}", capabilities.workspace_access),
        );
        self
    }

    /// Checks output-contract shape without reading workspace outputs.
    #[must_use]
    pub fn output_contract(mut self, contract: &OutputContract) -> Self {
        check_output_contract(&mut self.report, contract);
        self
    }

    /// Runs the presenter on a representative candidate/case without invoking
    /// the agent runtime.
    pub async fn presenter_dry_run<P, Factory, Presenter>(
        mut self,
        candidate_id: CandidateId,
        candidate: &P::Artifact,
        case: &AgentCase,
        factory: &Factory,
        workspace_config: WorkspaceConfig,
        presenter: &Presenter,
        ctx: MaterializeContext<'_, P>,
    ) -> Self
    where
        P: leaven_core::OptimizationProblem,
        Factory: WorkspaceFactory + ?Sized,
        Presenter: AgentCasePresenter<P>,
    {
        let mut workspace = match factory.allocate(workspace_config).await {
            Ok(workspace) => workspace,
            Err(error) => {
                self.report
                    .error("presenter", format!("workspace allocation failed: {error}"));
                return self;
            }
        };

        let stage_result = async {
            let mut view = workspace.view();
            presenter
                .present(
                    AgentCasePresentationInput {
                        candidate_id,
                        candidate,
                        case,
                        graph: ctx.graph().clone(),
                    },
                    &mut view,
                    ctx,
                )
                .await
        }
        .await;
        let cleanup_result = workspace.cleanup().await;

        match (stage_result, cleanup_result) {
            (Ok(presentation), Ok(())) => {
                self.report.ok(
                    "presenter",
                    format!(
                        "presenter wrote {} materialized ref(s)",
                        presentation.value.materialized_refs.len()
                    ),
                );
                check_output_contract(
                    &mut self.report,
                    &presentation.value.request.output_contract,
                );
            }
            (Ok(_), Err(cleanup)) => {
                self.report.error(
                    "presenter-cleanup",
                    format!("workspace cleanup failed: {cleanup}"),
                );
            }
            (Err(error), Ok(())) => {
                self.report
                    .error("presenter", format!("presenter dry run failed: {error}"));
            }
            (Err(stage), Err(cleanup)) => {
                let combined = WithWorkspaceError::StageAndCleanup { stage, cleanup };
                self.report.error(
                    "presenter",
                    format!("presenter and cleanup failed: {combined}"),
                );
            }
        }

        self
    }

    /// Finishes the report.
    #[must_use]
    pub fn check(self) -> AgentRunPreflightReport {
        self.report
    }
}

fn check_output_contract(report: &mut AgentRunPreflightReport, contract: &OutputContract) {
    match contract {
        OutputContract::Files { paths } => {
            if paths.is_empty() {
                report.error("output-contract", "file output contract has no paths");
            } else {
                report.ok(
                    "output-contract",
                    format!("{} required output file(s)", paths.len()),
                );
            }
        }
        OutputContract::JsonFile { path, schema } => {
            if schema.is_some() {
                report.ok(
                    "output-contract",
                    format!("JSON output `{}` with schema", path.as_str()),
                );
            } else {
                report.ok(
                    "output-contract",
                    format!("JSON output `{}` without schema", path.as_str()),
                );
            }
        }
        OutputContract::FinalMessage => {
            report.ok("output-contract", "final assistant message required");
        }
        OutputContract::WorkspaceDiff { roots } => {
            if roots.is_empty() {
                report.warn(
                    "output-contract",
                    "workspace-diff contract has no explicit roots",
                );
            } else {
                report.ok(
                    "output-contract",
                    format!("workspace diff over {} root(s)", roots.len()),
                );
            }
        }
    }
}

fn short_fingerprint(fingerprint: Fingerprint) -> String {
    hex::encode(&fingerprint.0[..8])
}
