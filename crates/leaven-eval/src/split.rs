//! Dataset split roles and membership.

use std::collections::{BTreeMap, BTreeSet};

use leaven_core::{CaseSetVersion, PartitionId};
use leaven_kernel::{CaseId, Fingerprint, FingerprintBuilder};

use crate::DatasetSplitsError;

/// Conventional meaning assigned to one partition.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum SplitRole {
    /// Optimizer-driving training/search cases.
    Train,
    /// Held-out development/validation cases.
    Validation,
    /// Final report-only test cases.
    Test,
    /// Optimizer search partition when distinct from train.
    Search,
    /// Explicit probe partition.
    Probe,
    /// Report-only partition.
    ReportOnly,
    /// Domain-specific split role.
    Custom(smol_str::SmolStr),
}

impl SplitRole {
    /// Conventional partition id for this role.
    #[must_use]
    pub fn partition_id(&self) -> PartitionId {
        match self {
            Self::Train => PartitionId::from("TRAIN"),
            Self::Validation => PartitionId::from("VALIDATION"),
            Self::Test => PartitionId::from("TEST"),
            Self::Search => PartitionId::from("SEARCH"),
            Self::Probe => PartitionId::from("PROBE"),
            Self::ReportOnly => PartitionId::from("REPORT_ONLY"),
            Self::Custom(name) => PartitionId::new(name.to_string()),
        }
    }
}

/// Whether split membership must be disjoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SplitPolicy {
    /// A case may appear in only one split.
    DisjointRequired,
    /// Overlap is intentional and documented.
    OverlapAllowed {
        /// Human reason for overlap.
        reason: String,
    },
}

/// Durable split membership and fingerprint.
#[derive(Clone, Debug)]
pub struct DatasetSplits {
    version: CaseSetVersion,
    roles: BTreeMap<PartitionId, SplitRole>,
    cases: BTreeMap<PartitionId, Vec<CaseId>>,
    policy: SplitPolicy,
    fingerprint: Fingerprint,
}

impl DatasetSplits {
    /// Builds split membership and verifies the declared policy.
    pub fn new(
        version: CaseSetVersion,
        roles: BTreeMap<PartitionId, SplitRole>,
        cases: BTreeMap<PartitionId, Vec<CaseId>>,
        known_cases: &BTreeSet<CaseId>,
        policy: SplitPolicy,
    ) -> Result<Self, DatasetSplitsError> {
        for id in cases.values().flatten() {
            if !known_cases.contains(id) {
                return Err(DatasetSplitsError::UnknownCase(*id));
            }
        }
        if matches!(policy, SplitPolicy::DisjointRequired) {
            let mut seen = BTreeMap::<CaseId, SplitRole>::new();
            for (partition, ids) in &cases {
                let role = roles
                    .get(partition)
                    .cloned()
                    .unwrap_or_else(|| SplitRole::Custom(partition.0.to_string().into()));
                for id in ids {
                    if let Some(left) = seen.insert(*id, role.clone()) {
                        return Err(DatasetSplitsError::OverlappingCase {
                            case: *id,
                            left,
                            right: role,
                        });
                    }
                }
            }
        }
        let mut fingerprint = FingerprintBuilder::new();
        fingerprint.update(version.0.as_bytes());
        for (partition, role) in &roles {
            fingerprint.update(partition.0.as_bytes());
            fingerprint.update(format!("{role:?}").as_bytes());
        }
        for (partition, ids) in &cases {
            fingerprint.update(partition.0.as_bytes());
            for id in ids {
                fingerprint.update(id.0.to_le_bytes());
            }
        }
        Ok(Self {
            version,
            roles,
            cases,
            policy,
            fingerprint: fingerprint.finish(),
        })
    }

    /// Role for a partition.
    #[must_use]
    pub fn role(&self, partition: &PartitionId) -> Option<&SplitRole> {
        self.roles.get(partition)
    }

    /// Case ids in a partition.
    #[must_use]
    pub fn cases(&self, partition: &PartitionId) -> Option<&[CaseId]> {
        self.cases.get(partition).map(Vec::as_slice)
    }

    /// Split fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// Case-set version.
    #[must_use]
    pub const fn version(&self) -> &CaseSetVersion {
        &self.version
    }

    /// Split policy.
    #[must_use]
    pub const fn policy(&self) -> &SplitPolicy {
        &self.policy
    }
}
