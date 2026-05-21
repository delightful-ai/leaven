//! Dataset split roles and membership.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use leaven_core::{CaseSetVersion, PartitionId};
use leaven_kernel::{CaseId, Fingerprint, FingerprintBuilder};
use smol_str::SmolStr;

use crate::{DatasetSplitsError, SplitUsePolicy};

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

/// Paper-declared split manifest with required roles and split-use policy.
#[derive(Clone, Debug)]
pub struct DatasetSplitManifest {
    splits: DatasetSplits,
    required_roles: BTreeSet<SplitRole>,
    use_policy: SplitUsePolicy,
}

impl DatasetSplitManifest {
    /// Builds a manifest and refuses any required role that is absent or empty.
    pub fn new(
        splits: DatasetSplits,
        required_roles: impl IntoIterator<Item = SplitRole>,
        use_policy: SplitUsePolicy,
    ) -> Result<Self, DatasetSplitsError> {
        let required_roles = required_roles.into_iter().collect::<BTreeSet<_>>();
        for role in &required_roles {
            match splits.cases(&role.partition_id()) {
                Some(cases) if !cases.is_empty() => {}
                _ => return Err(DatasetSplitsError::EmptyRequiredSplit { role: role.clone() }),
            }
        }
        Ok(Self {
            splits,
            required_roles,
            use_policy,
        })
    }

    /// Split membership by role.
    #[must_use]
    pub fn cases_for_role(&self, role: &SplitRole) -> Option<&[CaseId]> {
        self.splits.cases(&role.partition_id())
    }

    /// Underlying split membership.
    #[must_use]
    pub const fn splits(&self) -> &DatasetSplits {
        &self.splits
    }

    /// Declared nonempty roles required by the paper or benchmark.
    #[must_use]
    pub const fn required_roles(&self) -> &BTreeSet<SplitRole> {
        &self.required_roles
    }

    /// Split-use policy attached to this manifest.
    #[must_use]
    pub const fn use_policy(&self) -> &SplitUsePolicy {
        &self.use_policy
    }
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

/// Deterministic exact-count split construction over caller-declared strata.
#[derive(Clone, Debug)]
pub struct StratifiedSplitBuilder {
    strata: BTreeMap<SmolStr, Vec<CaseId>>,
    role_counts: Vec<(SplitRole, usize)>,
}

/// Deterministic split construction over an upstream row-order manifest.
#[derive(Clone, Debug)]
pub struct RowOrderSplitBuilder {
    ordered_cases: Vec<CaseId>,
    role_ranges: Vec<(SplitRole, Range<usize>)>,
}

impl RowOrderSplitBuilder {
    /// Builds a split builder from a trusted ordered case manifest.
    #[must_use]
    pub fn new(ordered_cases: Vec<CaseId>) -> Self {
        Self {
            ordered_cases,
            role_ranges: Vec::new(),
        }
    }

    /// Assigns a half-open row range to one split role.
    #[must_use]
    pub fn role_range(mut self, role: SplitRole, range: Range<usize>) -> Self {
        self.role_ranges.push((role, range));
        self
    }

    /// Builds disjoint split membership from the row ranges.
    pub fn build(self, version: CaseSetVersion) -> Result<DatasetSplits, DatasetSplitsError> {
        let known_cases = self.ordered_cases.iter().copied().collect::<BTreeSet<_>>();
        let mut roles = BTreeMap::new();
        let mut cases = BTreeMap::new();
        for (role, range) in self.role_ranges {
            if range.start > range.end || range.end > self.ordered_cases.len() {
                return Err(DatasetSplitsError::InvalidRowRange {
                    role,
                    start: range.start,
                    end: range.end,
                    len: self.ordered_cases.len(),
                });
            }
            let partition = role.partition_id();
            if roles.contains_key(&partition) {
                return Err(DatasetSplitsError::DuplicateSplitRole(role));
            }
            let ids = self.ordered_cases[range].to_vec();
            roles.insert(partition.clone(), role);
            cases.insert(partition, ids);
        }

        DatasetSplits::new(
            version,
            roles,
            cases,
            &known_cases,
            SplitPolicy::DisjointRequired,
        )
    }
}

impl StratifiedSplitBuilder {
    /// Builds a split builder from trusted category/stratum pools.
    pub fn new(strata: BTreeMap<SmolStr, Vec<CaseId>>) -> Result<Self, DatasetSplitsError> {
        let mut seen = BTreeMap::<CaseId, SmolStr>::new();
        for (stratum, ids) in &strata {
            for id in ids {
                if let Some(left) = seen.insert(*id, stratum.clone()) {
                    return Err(DatasetSplitsError::DuplicateStratifiedCase {
                        case: *id,
                        left,
                        right: stratum.clone(),
                    });
                }
            }
        }
        Ok(Self {
            strata,
            role_counts: Vec::new(),
        })
    }

    /// Requests an exact number of cases for one split role.
    #[must_use]
    pub fn role_count(mut self, role: SplitRole, count: usize) -> Self {
        self.role_counts.push((role, count));
        self
    }

    /// Builds disjoint split membership with deterministic proportional
    /// allocation across strata.
    pub fn build(self, version: CaseSetVersion) -> Result<DatasetSplits, DatasetSplitsError> {
        let requested = self
            .role_counts
            .iter()
            .map(|(_, count)| *count)
            .sum::<usize>();
        let available = self.strata.values().map(Vec::len).sum::<usize>();
        if requested > available {
            return Err(DatasetSplitsError::InsufficientStratifiedCases {
                requested,
                available,
            });
        }

        let known_cases = self
            .strata
            .values()
            .flat_map(|ids| ids.iter().copied())
            .collect::<BTreeSet<_>>();
        let mut offsets = self
            .strata
            .keys()
            .map(|stratum| (stratum.clone(), 0_usize))
            .collect::<BTreeMap<_, _>>();
        let mut roles = BTreeMap::new();
        let mut cases = BTreeMap::new();

        for (role, count) in &self.role_counts {
            let partition = role.partition_id();
            if roles.contains_key(&partition) {
                return Err(DatasetSplitsError::DuplicateSplitRole(role.clone()));
            }
            let selected = self.take_stratified(*count, &mut offsets);
            roles.insert(partition.clone(), role.clone());
            cases.insert(partition, selected);
        }

        DatasetSplits::new(
            version,
            roles,
            cases,
            &known_cases,
            SplitPolicy::DisjointRequired,
        )
    }

    fn take_stratified(&self, count: usize, offsets: &mut BTreeMap<SmolStr, usize>) -> Vec<CaseId> {
        let remaining = self
            .strata
            .iter()
            .map(|(stratum, ids)| {
                let offset = offsets.get(stratum).copied().unwrap_or_default();
                (stratum.clone(), ids.len() - offset)
            })
            .collect::<BTreeMap<_, _>>();
        let allocation = allocate_proportional(count, &remaining);
        let mut selected = Vec::with_capacity(count);
        for (stratum, take) in allocation {
            let offset = offsets
                .get_mut(&stratum)
                .expect("allocation stratum exists in offsets");
            let ids = self
                .strata
                .get(&stratum)
                .expect("allocation stratum exists in strata");
            selected.extend(ids[*offset..*offset + take].iter().copied());
            *offset += take;
        }
        selected
    }
}

fn allocate_proportional(
    count: usize,
    remaining: &BTreeMap<SmolStr, usize>,
) -> BTreeMap<SmolStr, usize> {
    let total = remaining.values().sum::<usize>();
    let mut allocated = BTreeMap::new();
    if count == 0 || total == 0 {
        return allocated;
    }

    let mut assigned = 0_usize;
    let mut remainders = Vec::new();
    for (stratum, available) in remaining {
        let numerator = count * *available;
        let whole = numerator / total;
        assigned += whole;
        allocated.insert(stratum.clone(), whole);
        remainders.push((numerator % total, stratum.clone()));
    }

    remainders.sort_by(|(left_remainder, left), (right_remainder, right)| {
        right_remainder
            .cmp(left_remainder)
            .then_with(|| left.cmp(right))
    });
    let mut to_assign = count - assigned;
    for (_, stratum) in remainders {
        if to_assign == 0 {
            break;
        }
        let available = remaining
            .get(&stratum)
            .expect("remainder stratum exists in remaining");
        let current = allocated
            .get_mut(&stratum)
            .expect("remainder stratum exists in allocation");
        if *current < *available {
            *current += 1;
            to_assign -= 1;
        }
    }
    allocated
}
