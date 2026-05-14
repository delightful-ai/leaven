//! Agentic case and workload vocabulary.

use std::collections::{BTreeMap, BTreeSet};

use leaven_kernel::{CaseId, ContentId, Fingerprint, FingerprintBuilder, MetadataBag};
use leaven_workspace::WorkspacePath;
use serde::{Deserialize, Serialize};

use crate::AgenticAdapterError;

/// A deterministic suite of agentic task cases.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseSuite {
    cases: BTreeMap<CaseId, AgentCase>,
    partitions: CasePartitions,
    fingerprint: Fingerprint,
}

impl CaseSuite {
    /// Constructs a suite and computes its behavior fingerprint.
    ///
    /// # Errors
    ///
    /// Returns [`AgenticAdapterError`] when case ids are duplicated or a
    /// partition references a case id that is not present in the suite.
    pub fn from_cases(
        cases: impl IntoIterator<Item = AgentCase>,
    ) -> Result<Self, AgenticAdapterError> {
        let mut by_id = BTreeMap::new();
        let mut all = Vec::new();
        for case in cases {
            let id = case.id;
            if by_id.insert(id, case).is_some() {
                return Err(AgenticAdapterError::Input(format!(
                    "duplicate agent case id `{id}`"
                )));
            }
            all.push(id);
        }
        let partitions = CasePartitions::with_all(all);
        Self::new(by_id, partitions)
    }

    /// Constructs a suite with explicit partitions.
    ///
    /// # Errors
    ///
    /// Returns [`AgenticAdapterError`] when any partition references a missing
    /// case id.
    pub fn new(
        cases: BTreeMap<CaseId, AgentCase>,
        partitions: CasePartitions,
    ) -> Result<Self, AgenticAdapterError> {
        partitions.validate_against(cases.keys().copied())?;
        let fingerprint = fingerprint_suite(&cases, &partitions)?;
        Ok(Self {
            cases,
            partitions,
            fingerprint,
        })
    }

    /// Returns cases keyed by stable case id.
    #[must_use]
    pub const fn cases(&self) -> &BTreeMap<CaseId, AgentCase> {
        &self.cases
    }

    /// Returns suite partitions.
    #[must_use]
    pub const fn partitions(&self) -> &CasePartitions {
        &self.partitions
    }

    /// Returns the suite fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// Returns true when no cases are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}

/// Named case partitions used by evaluators and preflight.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CasePartitions {
    named: BTreeMap<CasePartitionId, Vec<CaseId>>,
}

impl CasePartitions {
    /// Constructs a partition set containing an `all` partition.
    #[must_use]
    pub fn with_all(cases: Vec<CaseId>) -> Self {
        let mut named = BTreeMap::new();
        named.insert(CasePartitionId::all(), cases);
        Self { named }
    }

    /// Adds or replaces a partition.
    #[must_use]
    pub fn with_partition(mut self, id: CasePartitionId, cases: Vec<CaseId>) -> Self {
        self.named.insert(id, cases);
        self
    }

    /// Returns named partitions.
    #[must_use]
    pub const fn named(&self) -> &BTreeMap<CasePartitionId, Vec<CaseId>> {
        &self.named
    }

    /// Validates that every partition target exists.
    ///
    /// # Errors
    ///
    /// Returns [`AgenticAdapterError`] when a partition references a missing
    /// case id.
    pub fn validate_against(
        &self,
        case_ids: impl IntoIterator<Item = CaseId>,
    ) -> Result<(), AgenticAdapterError> {
        let known = case_ids.into_iter().collect::<BTreeSet<_>>();
        for (partition, ids) in &self.named {
            for id in ids {
                if !known.contains(id) {
                    return Err(AgenticAdapterError::Input(format!(
                        "partition `{}` references missing case `{id}`",
                        partition.as_str()
                    )));
                }
            }
        }
        Ok(())
    }

    /// Returns true when any configured partition is empty.
    #[must_use]
    pub fn has_empty_partition(&self) -> bool {
        self.named.values().any(Vec::is_empty)
    }
}

/// Stable user-facing partition name.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CasePartitionId(String);

impl CasePartitionId {
    /// Constructs a partition id.
    ///
    /// # Errors
    ///
    /// Returns [`AgenticAdapterError`] when the id is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, AgenticAdapterError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AgenticAdapterError::Input(
                "case partition id cannot be empty".to_owned(),
            ));
        }
        Ok(Self(value))
    }

    /// Standard partition containing every case in the suite.
    #[must_use]
    pub fn all() -> Self {
        Self("all".to_owned())
    }

    /// Returns the string id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CasePartitionId {
    fn from(value: &str) -> Self {
        Self::new(value).expect("static case partition ids must be non-empty")
    }
}

/// One agentic task case.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentCase {
    pub id: CaseId,
    pub input: CaseInput,
    pub target: CaseTarget,
    pub metadata: MetadataBag,
    pub files: CaseFiles,
    pub setup: Option<SetupScript>,
    pub workspace: Option<WorkspaceRequirement>,
}

impl AgentCase {
    /// Constructs a text-input case.
    #[must_use]
    pub fn text(id: CaseId, input: impl Into<String>, target: CaseTarget) -> Self {
        Self {
            id,
            input: CaseInput::Text(input.into()),
            target,
            metadata: MetadataBag::new(),
            files: CaseFiles::default(),
            setup: None,
            workspace: None,
        }
    }
}

/// Case input made visible to the candidate agent by a presenter.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CaseInput {
    Text(String),
    Messages(Vec<CaseMessage>),
    FileRef(ContentId),
    Structured(serde_json::Value),
}

/// Minimal provider-neutral message shape for case input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CaseMessage {
    pub role: String,
    pub content: String,
}

/// Case target. Hidden targets are scorer-visible, not candidate-visible.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CaseTarget {
    Text(String),
    Structured(serde_json::Value),
    Hidden(ContentId),
    None,
}

impl CaseTarget {
    /// Returns true when the target must not be materialized for the candidate.
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden(_))
    }
}

/// Files that a presenter may materialize for a case.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CaseFiles {
    files: BTreeMap<WorkspacePath, Vec<u8>>,
}

impl CaseFiles {
    /// Adds or replaces a case file.
    pub fn insert(&mut self, path: WorkspacePath, bytes: Vec<u8>) -> &mut Self {
        self.files.insert(path, bytes);
        self
    }

    /// Returns case files keyed by workspace path.
    #[must_use]
    pub const fn files(&self) -> &BTreeMap<WorkspacePath, Vec<u8>> {
        &self.files
    }
}

/// Optional case-local setup command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SetupScript {
    pub command: Vec<String>,
}

/// Workspace capabilities required by a case.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceRequirement {
    BackendNeutral,
    RequiresLocalMount,
    RequiresCommands,
}

/// Agent workload configuration shared by stock agentic evaluators.
///
/// This is candidate-evaluation workload vocabulary: cases, partitions, case
/// inputs, hidden scorer targets, and setup requirements. Optimizer-stage
/// agent workspaces use `AgentStagePlan` / `AgentBacked` / receipts in the
/// stage layer instead of depending on `AgentCase`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentWorkload {
    cases: CaseSuite,
}

impl AgentWorkload {
    /// Constructs a workload from a case suite.
    #[must_use]
    pub const fn new(cases: CaseSuite) -> Self {
        Self { cases }
    }

    /// Constructs a workload from cases and derives the standard `all`
    /// partition.
    ///
    /// # Errors
    ///
    /// Returns [`AgenticAdapterError`] when case ids are duplicated.
    pub fn from_cases(
        cases: impl IntoIterator<Item = AgentCase>,
    ) -> Result<Self, AgenticAdapterError> {
        Ok(Self {
            cases: CaseSuite::from_cases(cases)?,
        })
    }

    /// Constructs a workload from explicit case and partition maps.
    ///
    /// # Errors
    ///
    /// Returns [`AgenticAdapterError`] when any partition references a case id
    /// that is not present.
    pub fn from_parts(
        cases: BTreeMap<CaseId, AgentCase>,
        partitions: CasePartitions,
    ) -> Result<Self, AgenticAdapterError> {
        Ok(Self {
            cases: CaseSuite::new(cases, partitions)?,
        })
    }

    /// Returns the workload case suite.
    #[must_use]
    pub const fn cases(&self) -> &CaseSuite {
        &self.cases
    }

    /// Returns workload partitions.
    #[must_use]
    pub const fn partitions(&self) -> &CasePartitions {
        self.cases.partitions()
    }

    /// Returns the workload fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.cases.fingerprint()
    }

    /// Returns true when no cases are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}

fn fingerprint_suite(
    cases: &BTreeMap<CaseId, AgentCase>,
    partitions: &CasePartitions,
) -> Result<Fingerprint, AgenticAdapterError> {
    let bytes = serde_json::to_vec(&(cases, partitions)).map_err(|error| {
        AgenticAdapterError::Input(format!("case suite fingerprint failed: {error}"))
    })?;
    let mut builder = FingerprintBuilder::new();
    builder
        .update(b"leaven.agentic.case-suite.v1")
        .update(bytes);
    Ok(builder.finish())
}
