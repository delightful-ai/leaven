//! Deterministic preflight checks for agentic runs.

use leaven_agent::{AgentRuntime, OutputContract};
use leaven_core::Artifact;
use leaven_kernel::Fingerprint;

use crate::{AgentWorkload, CaseSuite};

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
        match contract {
            OutputContract::Files { paths } => {
                if paths.is_empty() {
                    self.report
                        .error("output-contract", "file output contract has no paths");
                } else {
                    self.report.ok(
                        "output-contract",
                        format!("{} required output file(s)", paths.len()),
                    );
                }
            }
            OutputContract::JsonFile { path, schema } => {
                if schema.is_some() {
                    self.report.ok(
                        "output-contract",
                        format!("JSON output `{}` with schema", path.as_str()),
                    );
                } else {
                    self.report.ok(
                        "output-contract",
                        format!("JSON output `{}` without schema", path.as_str()),
                    );
                }
            }
            OutputContract::FinalMessage => {
                self.report
                    .ok("output-contract", "final assistant message required");
            }
            OutputContract::WorkspaceDiff { roots } => {
                if roots.is_empty() {
                    self.report.warn(
                        "output-contract",
                        "workspace-diff contract has no explicit roots",
                    );
                } else {
                    self.report.ok(
                        "output-contract",
                        format!("workspace diff over {} root(s)", roots.len()),
                    );
                }
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

fn short_fingerprint(fingerprint: Fingerprint) -> String {
    hex::encode(&fingerprint.0[..8])
}
