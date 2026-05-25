//! Durable resume compatibility manifests.

use std::{
    collections::BTreeMap,
    fs,
    fs::OpenOptions,
    io,
    io::Write,
    path::{Path, PathBuf},
};

use leaven_core::CaseSetVersion;
use leaven_engine::{CachePolicy, OptimizerCompatibility};
use leaven_eval::{Case, DatasetSplits};
use leaven_kernel::{Budget, Fingerprint, FingerprintBuilder};
use serde::{Deserialize, Serialize};

use crate::result::RunCompatibilitySummary;

const MANIFEST_FILE: &str = "compatibility.json";
const MANIFEST_SCHEMA: &str = "leaven-run.compatibility.v4";

/// Runtime slot whose behavior must be explicitly fingerprinted for durable runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeKind {
    /// Candidate execution runtime.
    Runner,
    /// Score/judge runtime.
    Scorer,
}

impl RuntimeKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Runner => "runner",
            Self::Scorer => "scorer",
        }
    }
}

/// Typed runtime compatibility declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeFingerprint {
    fingerprint: Fingerprint,
}

impl RuntimeFingerprint {
    /// Builds a runtime fingerprint from a stable behavior fingerprint.
    #[must_use]
    pub const fn new(fingerprint: Fingerprint) -> Self {
        Self { fingerprint }
    }

    /// Raw behavior fingerprint.
    #[must_use]
    pub const fn fingerprint(self) -> Fingerprint {
        self.fingerprint
    }
}

/// Identity ingredients used by the product scoring evaluator.
#[derive(Clone, Debug)]
pub struct ScoringEvaluatorIdentity {
    /// Human/debug label for this evaluator shape.
    pub label: String,
    /// Runner behavior identity.
    pub runner: RuntimeFingerprint,
    /// Scorer behavior identity.
    pub scorer: RuntimeFingerprint,
    /// Dataset content identity.
    pub dataset: Fingerprint,
    /// Split role/membership identity.
    pub splits: Fingerprint,
    /// Evaluation cache semantics declared by this evaluator.
    pub cache_policy: CachePolicy,
}

impl ScoringEvaluatorIdentity {
    pub(crate) fn fingerprint(&self) -> Fingerprint {
        let mut fingerprint = FingerprintBuilder::new();
        fingerprint.update(b"leaven-run.scoring-evaluator.v1");
        fingerprint.update(self.label.as_bytes());
        fingerprint.update(self.runner.fingerprint().0);
        fingerprint.update(self.scorer.fingerprint().0);
        fingerprint.update(self.dataset.0);
        fingerprint.update(self.splits.0);
        fingerprint.update(b"granularity:per-case");
        fingerprint.update(b"aggregation:casewise-average-report");
        fingerprint.update(
            serde_json::to_vec(&self.cache_policy)
                .expect("cache policy serializes")
                .as_slice(),
        );
        fingerprint.finish()
    }
}

/// Durable run compatibility manifest stored beside a local run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunCompatibilityManifest {
    /// Manifest schema.
    pub schema: String,
    /// Product run kind.
    pub run_kind: String,
    /// Problem-shape compatibility placeholder for this slice.
    pub problem: Fingerprint,
    /// Case content and split compatibility.
    pub dataset: DatasetCompatibility,
    /// Runner behavior identity.
    pub runner: RuntimeFingerprint,
    /// Scorer behavior identity.
    pub scorer: RuntimeFingerprint,
    /// Composed evaluator/cache-key identity.
    pub evaluator: RuntimeFingerprint,
    /// Optimizer behavior compatibility, when the optimizer declares it.
    pub optimizer: Option<OptimizerCompatibility>,
    /// Role-specific LM runtime fingerprints declared by product adapters.
    pub lm_roles: BTreeMap<String, RuntimeFingerprint>,
    /// Cache semantic compatibility declaration.
    pub cache: String,
    /// Budget limit compatibility declaration.
    pub budget: String,
}

impl RunCompatibilityManifest {
    pub(crate) fn new(
        dataset: DatasetCompatibility,
        runner: RuntimeFingerprint,
        scorer: RuntimeFingerprint,
        evaluator: RuntimeFingerprint,
        optimizer: Option<OptimizerCompatibility>,
        lm_roles: BTreeMap<String, RuntimeFingerprint>,
        cache_policy: CachePolicy,
        budget: Budget,
    ) -> Self {
        Self {
            schema: MANIFEST_SCHEMA.to_owned(),
            run_kind: "leaven-run.optimize".to_owned(),
            problem: problem_placeholder(),
            dataset,
            runner,
            scorer,
            evaluator,
            optimizer,
            lm_roles,
            cache: cache_compatibility(&cache_policy),
            budget: budget_compatibility(&budget),
        }
    }

    pub(crate) fn summary(&self) -> RunCompatibilitySummary {
        RunCompatibilitySummary {
            schema: self.schema.clone(),
            run_kind: self.run_kind.clone(),
            dataset: fingerprint_hex(self.dataset.content),
            splits: fingerprint_hex(self.dataset.splits),
            case_set_version: self.dataset.case_set_version.clone(),
            runner: fingerprint_hex(self.runner.fingerprint()),
            scorer: fingerprint_hex(self.scorer.fingerprint()),
            evaluator: fingerprint_hex(self.evaluator.fingerprint()),
            optimizer: optimizer_summary(self.optimizer.as_ref()),
            cache: self.cache.clone(),
            budget: self.budget.clone(),
            lm_role_count: self.lm_roles.len(),
        }
    }
}

/// Durable dataset compatibility identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DatasetCompatibility {
    /// Stable case content identity.
    pub content: Fingerprint,
    /// Split role and membership identity.
    pub splits: Fingerprint,
    /// Derived case-set version used by engine cache keys.
    pub case_set_version: String,
}

impl DatasetCompatibility {
    pub(crate) fn new(content: Fingerprint, splits: &DatasetSplits) -> Self {
        Self {
            content,
            splits: splits.fingerprint(),
            case_set_version: splits.version().0.clone(),
        }
    }
}

/// Typed resume compatibility refusal.
#[derive(Debug, thiserror::Error)]
pub enum ResumeCompatibilityError {
    /// The stored manifest could not be read.
    #[error("stored compatibility manifest could not be read at {}", path.display())]
    Read {
        /// Manifest path.
        path: PathBuf,
        /// IO source.
        #[source]
        source: io::Error,
    },
    /// The stored manifest could not be decoded.
    #[error("stored compatibility manifest could not be decoded at {}", path.display())]
    Decode {
        /// Manifest path.
        path: PathBuf,
        /// Decode source.
        #[source]
        source: serde_json::Error,
    },
    /// Case content, target content, or split membership changed.
    #[error("stored dataset compatibility does not match live cases or splits")]
    DatasetFingerprintMismatch {
        /// Stored dataset compatibility.
        stored: Box<DatasetCompatibility>,
        /// Live dataset compatibility.
        live: Box<DatasetCompatibility>,
    },
    /// Runner behavior changed.
    #[error("stored runner fingerprint does not match live runner fingerprint")]
    RunnerFingerprintMismatch {
        /// Stored runtime fingerprint.
        stored: RuntimeFingerprint,
        /// Live runtime fingerprint.
        live: RuntimeFingerprint,
    },
    /// Scorer behavior changed.
    #[error("stored scorer fingerprint does not match live scorer fingerprint")]
    ScorerFingerprintMismatch {
        /// Stored runtime fingerprint.
        stored: RuntimeFingerprint,
        /// Live runtime fingerprint.
        live: RuntimeFingerprint,
    },
    /// Manifest schema changed.
    #[error("stored compatibility schema `{stored}` does not match live schema `{live}`")]
    SchemaMismatch {
        /// Stored schema.
        stored: String,
        /// Live schema.
        live: String,
    },
    /// Composed evaluator identity changed.
    #[error("stored evaluator fingerprint does not match live evaluator fingerprint")]
    EvaluatorFingerprintMismatch {
        /// Stored runtime fingerprint.
        stored: RuntimeFingerprint,
        /// Live runtime fingerprint.
        live: RuntimeFingerprint,
    },
    /// Optimizer behavior changed.
    #[error("stored optimizer compatibility does not match live optimizer compatibility")]
    OptimizerCompatibilityMismatch {
        /// Stored optimizer compatibility.
        stored: Box<Option<OptimizerCompatibility>>,
        /// Live optimizer compatibility.
        live: Box<Option<OptimizerCompatibility>>,
    },
    /// Role-specific LM behavior changed.
    #[error("stored LM role `{role}` fingerprint does not match live LM role fingerprint")]
    LmRoleFingerprintMismatch {
        /// Role whose fingerprint changed.
        role: String,
        /// Stored runtime fingerprint.
        stored: Option<RuntimeFingerprint>,
        /// Live runtime fingerprint.
        live: Option<RuntimeFingerprint>,
    },
    /// Cache semantic compatibility changed.
    #[error("stored cache compatibility does not match live cache compatibility")]
    CacheCompatibilityMismatch,
    /// Budget semantic compatibility changed.
    #[error("stored budget compatibility does not match live budget compatibility")]
    BudgetPolicyMismatch,
}

/// Stable content fingerprint for case ids, split roles, inputs, targets, and scorer metadata.
pub fn case_content_fingerprint<I, T>(
    train: &[Case<I, T>],
    validation: &[Case<I, T>],
    test: &[Case<I, T>],
) -> Result<Fingerprint, serde_json::Error>
where
    I: Serialize,
    T: Serialize,
{
    let mut fingerprint = FingerprintBuilder::new();
    fingerprint.update(b"leaven-run.case-content.v1");
    update_cases(&mut fingerprint, b"TRAIN", train)?;
    update_cases(&mut fingerprint, b"VALIDATION", validation)?;
    update_cases(&mut fingerprint, b"TEST", test)?;
    Ok(fingerprint.finish())
}

fn update_cases<I, T>(
    fingerprint: &mut FingerprintBuilder,
    split: &[u8],
    cases: &[Case<I, T>],
) -> Result<(), serde_json::Error>
where
    I: Serialize,
    T: Serialize,
{
    fingerprint.update(split);
    fingerprint.update(cases.len().to_le_bytes());
    for case in cases {
        fingerprint.update(case.id.0.to_le_bytes());
        fingerprint.update(serde_json::to_vec(&case.input)?);
        fingerprint.update(serde_json::to_vec(&case.target)?);
        fingerprint.update(serde_json::to_vec(&case.metadata)?);
    }
    Ok(())
}

pub fn case_set_version(content: Fingerprint) -> CaseSetVersion {
    CaseSetVersion(format!("leaven-run-cases-v1:{}", fingerprint_hex(content)))
}

pub fn store_fresh_manifest(
    run_dir: Option<&Path>,
    manifest: &RunCompatibilityManifest,
) -> Result<(), io::Error> {
    let Some(run_dir) = run_dir else {
        return Ok(());
    };
    fs::create_dir_all(run_dir)?;
    let path = run_dir.join(MANIFEST_FILE);
    let bytes = serde_json::to_vec_pretty(manifest)
        .expect("compatibility manifest contains only serializable fields");
    write_atomic(&path, &bytes)
}

pub fn compare_stored_manifest(
    run_dir: &Path,
    live: &RunCompatibilityManifest,
) -> Result<(), ResumeCompatibilityError> {
    let path = run_dir.join(MANIFEST_FILE);
    let bytes = fs::read(&path).map_err(|source| ResumeCompatibilityError::Read {
        path: path.clone(),
        source,
    })?;
    let stored: RunCompatibilityManifest = serde_json::from_slice(&bytes)
        .map_err(|source| ResumeCompatibilityError::Decode { path, source })?;
    compare_manifests(&stored, live)
}

fn compare_manifests(
    stored: &RunCompatibilityManifest,
    live: &RunCompatibilityManifest,
) -> Result<(), ResumeCompatibilityError> {
    if stored.schema != live.schema {
        return Err(ResumeCompatibilityError::SchemaMismatch {
            stored: stored.schema.clone(),
            live: live.schema.clone(),
        });
    }
    if stored.dataset != live.dataset {
        return Err(ResumeCompatibilityError::DatasetFingerprintMismatch {
            stored: Box::new(stored.dataset.clone()),
            live: Box::new(live.dataset.clone()),
        });
    }
    if stored.runner != live.runner {
        return Err(ResumeCompatibilityError::RunnerFingerprintMismatch {
            stored: stored.runner,
            live: live.runner,
        });
    }
    if stored.scorer != live.scorer {
        return Err(ResumeCompatibilityError::ScorerFingerprintMismatch {
            stored: stored.scorer,
            live: live.scorer,
        });
    }
    if stored.evaluator != live.evaluator {
        return Err(ResumeCompatibilityError::EvaluatorFingerprintMismatch {
            stored: stored.evaluator,
            live: live.evaluator,
        });
    }
    if stored.optimizer != live.optimizer {
        return Err(ResumeCompatibilityError::OptimizerCompatibilityMismatch {
            stored: Box::new(stored.optimizer.clone()),
            live: Box::new(live.optimizer.clone()),
        });
    }
    if stored.lm_roles != live.lm_roles {
        for role in stored.lm_roles.keys().chain(live.lm_roles.keys()) {
            let stored_role = stored.lm_roles.get(role).copied();
            let live_role = live.lm_roles.get(role).copied();
            if stored_role != live_role {
                return Err(ResumeCompatibilityError::LmRoleFingerprintMismatch {
                    role: role.clone(),
                    stored: stored_role,
                    live: live_role,
                });
            }
        }
    }
    if stored.cache != live.cache {
        return Err(ResumeCompatibilityError::CacheCompatibilityMismatch);
    }
    if stored.budget != live.budget {
        return Err(ResumeCompatibilityError::BudgetPolicyMismatch);
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), io::Error> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let temp = path.with_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let result = write_atomic_inner(path, &temp, parent, bytes);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_atomic_inner(
    path: &Path,
    temp: &Path,
    parent: &Path,
    bytes: &[u8],
) -> Result<(), io::Error> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)?;
    let dir = OpenOptions::new().read(true).open(parent)?;
    dir.sync_all()
}

fn problem_placeholder() -> Fingerprint {
    let mut fingerprint = FingerprintBuilder::new();
    fingerprint.update(b"leaven-run.problem-placeholder.v1");
    fingerprint.finish()
}

fn cache_compatibility(policy: &CachePolicy) -> String {
    let policy = serde_json::to_string(policy).expect("cache policy serializes");
    format!("cache:evaluation-policy-json:{policy}")
}

fn budget_compatibility(budget: &Budget) -> String {
    let limit = serde_json::to_string(budget).expect("budget serializes");
    format!("budget:limit-json:{limit}")
}

fn fingerprint_hex(fingerprint: Fingerprint) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in fingerprint.0 {
        out.push(HEX[usize::from(byte >> 4)] as char);
        out.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    out
}

fn optimizer_summary(optimizer: Option<&OptimizerCompatibility>) -> String {
    let Some(optimizer) = optimizer else {
        return "optimizer:undeclared".to_owned();
    };
    let mut summary = format!(
        "optimizer:fingerprint={}",
        fingerprint_hex(optimizer.fingerprint)
    );
    match &optimizer.private_state_policy {
        leaven_engine::PrivateStatePolicy::DerivedFromGraph => {
            summary.push_str(";state=derived-from-graph");
        }
        leaven_engine::PrivateStatePolicy::ExplicitSnapshot { schema, format } => {
            summary.push_str(";state=explicit-snapshot;schema=");
            summary.push_str(&fingerprint_hex(*schema));
            summary.push_str(";format=");
            summary.push_str(match format {
                leaven_engine::StateFormat::Json => "json",
                leaven_engine::StateFormat::Postcard => "postcard",
                leaven_engine::StateFormat::Custom(name) => name.as_str(),
            });
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use leaven_engine::{PrivateStatePolicy, StateFormat};

    use super::*;

    #[test]
    fn optimizer_summaries_disclose_checkpoint_policy_and_format() {
        let derived = OptimizerCompatibility::new(
            Fingerprint::from_bytes([1; 32]),
            PrivateStatePolicy::DerivedFromGraph,
        );
        assert!(optimizer_summary(Some(&derived)).contains(";state=derived-from-graph"));

        let postcard = OptimizerCompatibility::new(
            Fingerprint::from_bytes([2; 32]),
            PrivateStatePolicy::ExplicitSnapshot {
                schema: Fingerprint::from_bytes([3; 32]),
                format: StateFormat::Postcard,
            },
        );
        let summary = optimizer_summary(Some(&postcard));
        assert!(summary.contains(";state=explicit-snapshot"));
        assert!(summary.contains(";format=postcard"));

        let custom = OptimizerCompatibility::new(
            Fingerprint::from_bytes([4; 32]),
            PrivateStatePolicy::ExplicitSnapshot {
                schema: Fingerprint::from_bytes([5; 32]),
                format: StateFormat::Custom("binary-gepa".to_owned()),
            },
        );
        assert!(optimizer_summary(Some(&custom)).contains(";format=binary-gepa"));
    }

    #[test]
    fn compare_manifest_reports_first_differing_lm_role() {
        let mut stored = manifest_with_roles(BTreeMap::from([
            (
                "reflect".to_owned(),
                RuntimeFingerprint::new(Fingerprint::from_bytes([1; 32])),
            ),
            (
                "judge".to_owned(),
                RuntimeFingerprint::new(Fingerprint::from_bytes([2; 32])),
            ),
        ]));
        let live = manifest_with_roles(BTreeMap::from([(
            "reflect".to_owned(),
            RuntimeFingerprint::new(Fingerprint::from_bytes([1; 32])),
        )]));

        let error = compare_manifests(&stored, &live)
            .expect_err("missing live role must reject resume compatibility");
        assert!(matches!(
            error,
            ResumeCompatibilityError::LmRoleFingerprintMismatch {
                role,
                stored: Some(_),
                live: None,
            } if role == "judge"
        ));

        stored.lm_roles.insert(
            "judge".to_owned(),
            RuntimeFingerprint::new(Fingerprint::from_bytes([7; 32])),
        );
        let mut live = live;
        live.lm_roles.insert(
            "judge".to_owned(),
            RuntimeFingerprint::new(Fingerprint::from_bytes([8; 32])),
        );
        let error = compare_manifests(&stored, &live)
            .expect_err("changed live role must reject resume compatibility");
        assert!(matches!(
            error,
            ResumeCompatibilityError::LmRoleFingerprintMismatch {
                role,
                stored: Some(_),
                live: Some(_),
            } if role == "judge"
        ));
    }

    #[test]
    fn manifest_cache_and_budget_are_derived_from_typed_inputs() {
        let deterministic_seed = manifest_with_policy_and_budget(
            CachePolicy::DeterministicWithSeed(7),
            Budget::metric_calls(3),
        );
        let deterministic =
            manifest_with_policy_and_budget(CachePolicy::Deterministic, Budget::metric_calls(3));
        let unlimited = manifest_with_policy_and_budget(
            CachePolicy::DeterministicWithSeed(7),
            Budget::unlimited(),
        );

        assert!(
            deterministic_seed
                .cache
                .starts_with("cache:evaluation-policy-json:")
        );
        assert!(deterministic_seed.budget.starts_with("budget:limit-json:"));
        assert_ne!(deterministic_seed.cache, deterministic.cache);
        assert_ne!(deterministic_seed.budget, unlimited.budget);
    }

    #[test]
    fn atomic_manifest_write_rejects_paths_without_file_names() {
        let error = write_atomic(Path::new(""), b"manifest")
            .expect_err("compatibility manifest writes require a file path");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), "path has no file name");
    }

    #[test]
    fn compare_manifest_reports_missing_manifest_read_error() {
        let run_dir = std::env::temp_dir().join(format!(
            "leaven-run-compatibility-missing-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&run_dir).unwrap();
        let manifest = manifest_with_roles(BTreeMap::new());

        let error = compare_stored_manifest(&run_dir, &manifest)
            .expect_err("missing compatibility manifest must reject resume");

        assert!(matches!(error, ResumeCompatibilityError::Read { .. }));
        std::fs::remove_dir_all(run_dir).unwrap();
    }

    fn manifest_with_roles(
        lm_roles: BTreeMap<String, RuntimeFingerprint>,
    ) -> RunCompatibilityManifest {
        RunCompatibilityManifest::new(
            DatasetCompatibility {
                content: Fingerprint::from_bytes([9; 32]),
                splits: Fingerprint::from_bytes([10; 32]),
                case_set_version: "cases-v1".to_owned(),
            },
            RuntimeFingerprint::new(Fingerprint::from_bytes([11; 32])),
            RuntimeFingerprint::new(Fingerprint::from_bytes([12; 32])),
            RuntimeFingerprint::new(Fingerprint::from_bytes([13; 32])),
            None,
            lm_roles,
            CachePolicy::Never,
            Budget::unlimited(),
        )
    }

    fn manifest_with_policy_and_budget(
        cache_policy: CachePolicy,
        budget: Budget,
    ) -> RunCompatibilityManifest {
        RunCompatibilityManifest::new(
            DatasetCompatibility {
                content: Fingerprint::from_bytes([9; 32]),
                splits: Fingerprint::from_bytes([10; 32]),
                case_set_version: "cases-v1".to_owned(),
            },
            RuntimeFingerprint::new(Fingerprint::from_bytes([11; 32])),
            RuntimeFingerprint::new(Fingerprint::from_bytes([12; 32])),
            RuntimeFingerprint::new(Fingerprint::from_bytes([13; 32])),
            None,
            BTreeMap::new(),
            cache_policy,
            budget,
        )
    }
}
