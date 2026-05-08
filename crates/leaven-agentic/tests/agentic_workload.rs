use std::collections::BTreeMap;

use leaven_agent::FakeAgentRuntime;
use leaven_agentic::{
    AgentCase, AgentRunPreflight, AgentWorkload, CasePartitionId, CasePartitions, CaseSuite,
    CaseTarget, PreflightSeverity,
};
use leaven_core::{Artifact, ArtifactIdentity};
use leaven_kernel::{CaseId, ContentId};

#[test]
fn case_suite_fingerprint_changes_when_case_content_or_partitions_change() {
    let case = AgentCase::text(
        CaseId::new(0),
        "input",
        CaseTarget::Text("answer".to_owned()),
    );
    let baseline = CaseSuite::from_cases([case.clone()]).unwrap();

    let changed_case = AgentCase::text(case.id, "different", CaseTarget::Text("answer".to_owned()));
    let changed_cases = CaseSuite::from_cases([changed_case]).unwrap();

    let partitions = CasePartitions::with_all(vec![case.id])
        .with_partition(CasePartitionId::from("feedback"), vec![case.id]);
    let mut map = BTreeMap::new();
    map.insert(case.id, case);
    let changed_partitions = CaseSuite::new(map, partitions).unwrap();

    assert_ne!(baseline.fingerprint(), changed_cases.fingerprint());
    assert_ne!(baseline.fingerprint(), changed_partitions.fingerprint());
}

#[test]
fn case_suite_rejects_duplicate_ids_and_missing_partition_targets() {
    let id = CaseId::new(10);
    let one = AgentCase::text(id, "one", CaseTarget::None);
    let two = AgentCase::text(id, "two", CaseTarget::None);

    let duplicate = CaseSuite::from_cases([one.clone(), two]).unwrap_err();
    assert!(duplicate.to_string().contains("duplicate agent case id"));

    let partitions = CasePartitions::with_all(vec![CaseId::new(99)]);
    let mut cases = BTreeMap::new();
    cases.insert(id, one);

    let missing = CaseSuite::new(cases, partitions).unwrap_err();
    assert!(missing.to_string().contains("references missing case"));
}

#[test]
fn preflight_reports_cases_partitions_hidden_targets_artifact_and_runtime() {
    let hidden = ContentId::from_bytes([9; 32]);
    let suite = CaseSuite::from_cases([AgentCase::text(
        CaseId::new(1),
        "question",
        CaseTarget::Hidden(hidden),
    )])
    .unwrap();
    let workload = AgentWorkload::new(suite);
    let runtime = FakeAgentRuntime::new(Vec::new());

    let report = AgentRunPreflight::new()
        .artifact(&ValidArtifact)
        .workload(&workload)
        .runtime(&runtime)
        .check();

    assert!(!report.has_errors());
    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Warning
            && finding.check == "trust"
            && finding.message.contains("hidden targets")
    }));
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.check == "runtime")
    );
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.check == "artifact")
    );
}

#[test]
fn preflight_flags_empty_case_suite_and_artifact_validation_errors() {
    let suite = CaseSuite::from_cases([]).unwrap();
    let workload = AgentWorkload::new(suite);

    let report = AgentRunPreflight::new()
        .artifact(&InvalidArtifact)
        .workload(&workload)
        .check();

    assert!(report.has_errors());
    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error
            && finding.check == "artifact"
            && finding.message.contains("invalid artifact")
    }));
    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error && finding.check == "cases"
    }));
    assert!(report.findings().iter().any(|finding| {
        finding.severity == PreflightSeverity::Error && finding.check == "partitions"
    }));
}

#[derive(Clone)]
struct ValidArtifact;

impl Artifact for ValidArtifact {
    type Change = ();
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External("valid".to_owned())
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self)
    }
}

#[derive(Clone)]
struct InvalidArtifact;

impl Artifact for InvalidArtifact {
    type Change = ();
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External("invalid".to_owned())
    }

    fn validate(&self) -> Result<(), Self::ApplyError> {
        Err(TestArtifactError)
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid artifact")]
struct TestArtifactError;
