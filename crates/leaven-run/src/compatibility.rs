//! Durable resume compatibility manifests.

use std::{
    collections::BTreeMap,
    fs,
    io,
    path::{Path, PathBuf},
};

use leaven_core::{CaseSetVersion, PartitionId};
use leaven_eval::{Case, DatasetSplits, SplitRole};
use leaven_kernel::{Fingerprint, FingerprintBuilder};
use serde::{Deserialize, Serialize};

const MANIFEST_FILE: &str = "compatibility.json";
const MANIFEST_SCHEMA: &str = "leaven-run.compatibility.v1";
const CACHE_PLACEHOLDER: &str = "cache:auto/eval-schema-pending/lm-schema-pending";
const BUDGET_PLACEHOLDER: &str = "budget:ledger-compatibility-pending";
const OPTIMIZER_PLACEHOLDER: &str = "optimizer:engine-checkpoint-contract";

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
        fingerprint.update(b"cache:never");
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
    /// Optimizer compatibility placeholder; engine checkpoint restore validates concrete state.
    pub optimizer: String,
    /// Role-specific LM runtime fingerprints. Empty until LM roles are plumbed here.
    pub lm_roles: BTreeMap<String, RuntimeFingerprint>,
    /// Cache semantic compatibility placeholder.
    pub cache: String,
    /// Budget semantic compatibility placeholder.
    pub budget: String,
}

impl RunCompatibilityManifest {
    pub(crate) fn new(
        dataset: DatasetCompatibility,
        runner: RuntimeFingerprint,
        scorer: RuntimeFingerprint,
        evaluator: RuntimeFingerprint,
    ) -> Self {
        Self {
            schema: MANIFEST_SCHEMA.to_owned(),
            run_kind: "leaven-run.optimize".to_owned(),
            problem: problem_placeholder(),
            dataset,
            runner,
            scorer,
            evaluator,
            optimizer: OPTIMIZER_PLACEHOLDER.to_owned(),
            lm_roles: BTreeMap::new(),
            cache: CACHE_PLACEHOLDER.to_owned(),
            budget: BUDGET_PLACEHOLDER.to_owned(),
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

    pub(crate) fn fingerprint(&self) -> Fingerprint {
        let mut fingerprint = FingerprintBuilder::new();
        fingerprint.update(b"leaven-run.dataset-compatibility.v1");
        fingerprint.update(self.content.0);
        fingerprint.update(self.splits.0);
        fingerprint.update(self.case_set_version.as_bytes());
        fingerprint.finish()
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
        stored: DatasetCompatibility,
        /// Live dataset compatibility.
        live: DatasetCompatibility,
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
    /// Composed evaluator identity changed.
    #[error("stored evaluator fingerprint does not match live evaluator fingerprint")]
    EvaluatorFingerprintMismatch {
        /// Stored runtime fingerprint.
        stored: RuntimeFingerprint,
        /// Live runtime fingerprint.
        live: RuntimeFingerprint,
    },
    /// Cache semantic compatibility changed.
    #[error("stored cache compatibility does not match live cache compatibility")]
    CacheCompatibilityMismatch,
    /// Budget semantic compatibility changed.
    #[error("stored budget compatibility does not match live budget compatibility")]
    BudgetPolicyMismatch,
}

/// Stable content fingerprint for case ids, split roles, inputs, targets, and scorer metadata.
pub(crate) fn case_content_fingerprint<I, T>(
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

pub(crate) fn case_set_version(content: Fingerprint) -> CaseSetVersion {
    CaseSetVersion(format!("leaven-run-cases-v1:{}", fingerprint_hex(content)))
}

pub(crate) fn split_fingerprint(
    version: &CaseSetVersion,
    roles: &BTreeMap<PartitionId, SplitRole>,
    cases: &BTreeMap<PartitionId, Vec<leaven_kernel::CaseId>>,
) -> Fingerprint {
    let mut fingerprint = FingerprintBuilder::new();
    fingerprint.update(version.0.as_bytes());
    for (partition, role) in roles {
        fingerprint.update(partition.0.as_bytes());
        fingerprint.update(format!("{role:?}").as_bytes());
    }
    for (partition, ids) in cases {
        fingerprint.update(partition.0.as_bytes());
        for id in ids {
            fingerprint.update(id.0.to_le_bytes());
        }
    }
    fingerprint.finish()
}

pub(crate) fn store_fresh_manifest(
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
    fs::write(path, bytes)
}

pub(crate) fn compare_stored_manifest(
    run_dir: &Path,
    live: &RunCompatibilityManifest,
) -> Result<(), ResumeCompatibilityError> {
    let path = run_dir.join(MANIFEST_FILE);
    let bytes = fs::read(&path).map_err(|source| ResumeCompatibilityError::Read {
        path: path.clone(),
        source,
    })?;
    let stored: RunCompatibilityManifest =
        serde_json::from_slice(&bytes).map_err(|source| ResumeCompatibilityError::Decode {
            path,
            source,
        })?;
    compare_manifests(&stored, live)
}

fn compare_manifests(
    stored: &RunCompatibilityManifest,
    live: &RunCompatibilityManifest,
) -> Result<(), ResumeCompatibilityError> {
    if stored.dataset != live.dataset {
        return Err(ResumeCompatibilityError::DatasetFingerprintMismatch {
            stored: stored.dataset.clone(),
            live: live.dataset.clone(),
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
    if stored.cache != live.cache {
        return Err(ResumeCompatibilityError::CacheCompatibilityMismatch);
    }
    if stored.budget != live.budget {
        return Err(ResumeCompatibilityError::BudgetPolicyMismatch);
    }
    Ok(())
}

fn problem_placeholder() -> Fingerprint {
    let mut fingerprint = FingerprintBuilder::new();
    fingerprint.update(b"leaven-run.problem-placeholder.v1");
    fingerprint.finish()
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
