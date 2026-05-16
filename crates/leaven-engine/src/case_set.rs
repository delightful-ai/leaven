//! Case-set storage and evaluation-set resolution.

use std::collections::BTreeMap;
use std::fmt;

use indexmap::IndexMap;
use leaven_core::{CaseSetVersion, EvaluationSet, PartitionId, ResolvedEvaluationSet, Tag};
use leaven_kernel::{CaseId, ResolvedEvaluationSetId};
use thiserror::Error;

/// Concrete case collection used to resolve evaluation-set expressions.
pub struct CaseSet<C> {
    cases: IndexMap<CaseId, C>,
    partitions: BTreeMap<PartitionId, Vec<CaseId>>,
    version: u64,
}

impl<C> CaseSet<C> {
    /// Starts a case-set builder.
    #[must_use]
    pub fn builder() -> CaseSetBuilder<C> {
        CaseSetBuilder::default()
    }

    /// Builds a case set from ordered cases.
    #[must_use]
    pub fn new(cases: Vec<C>) -> Self {
        Self::builder().cases(cases).build()
    }

    /// Builds a case set from explicit case identifiers and ordered cases.
    #[must_use]
    pub fn from_entries(cases: impl IntoIterator<Item = (CaseId, C)>) -> Self {
        Self {
            cases: cases.into_iter().collect(),
            partitions: BTreeMap::new(),
            version: 0,
        }
    }

    /// Adds or replaces a named partition and increments the case-set version.
    #[must_use]
    pub fn with_partition(mut self, partition: PartitionId, case_ids: Vec<CaseId>) -> Self {
        self.partitions.insert(partition, case_ids);
        self.version += 1;
        self
    }

    /// Looks up one case by its identifier.
    ///
    /// Returns `None` when the case set does not contain `case`.
    #[must_use]
    pub fn get(&self, case: CaseId) -> Option<&C> {
        self.cases.get(&case)
    }

    /// Resolves an evaluation-set expression against this case set.
    pub fn resolve(
        &self,
        set: &EvaluationSet,
    ) -> Result<ResolvedEvaluationSet, EvaluationResolveError> {
        let case_ids = self.resolve_ids(set)?;
        Ok(ResolvedEvaluationSet {
            id: ResolvedEvaluationSetId::new(),
            expr: set.clone(),
            case_ids,
            resolved_at: leaven_kernel::now(),
            case_set_version: CaseSetVersion(self.version.to_string()),
        })
    }

    fn resolve_ids(&self, set: &EvaluationSet) -> Result<Vec<CaseId>, EvaluationResolveError> {
        match set {
            EvaluationSet::Unscoped | EvaluationSet::All => {
                Ok(self.cases.keys().copied().collect())
            }
            EvaluationSet::Partition(partition) => self
                .partitions
                .get(partition)
                .cloned()
                .ok_or_else(|| EvaluationResolveError::UnknownPartition(partition.clone())),
            EvaluationSet::Cases(case_ids) => {
                for id in case_ids {
                    if !self.cases.contains_key(id) {
                        return Err(EvaluationResolveError::UnknownCase(*id));
                    }
                }
                Ok(case_ids.clone())
            }
            EvaluationSet::Tagged(tag) => Err(EvaluationResolveError::UnsupportedSet(
                UnsupportedEvaluationSet::Tagged(tag.clone()),
            )),
            EvaluationSet::Recent { window } => Ok(self
                .cases
                .keys()
                .rev()
                .take(window.limit)
                .copied()
                .collect()),
            EvaluationSet::Sample { of, n, seed } => {
                let mut ids = self.resolve_ids(of)?;
                if !ids.is_empty() {
                    let rotation = usize::try_from(*seed % ids.len() as u64)
                        .expect("modulo result fits usize");
                    ids.rotate_left(rotation);
                }
                ids.truncate(*n);
                Ok(ids)
            }
            EvaluationSet::Stratified { by, .. } => Err(EvaluationResolveError::UnsupportedSet(
                UnsupportedEvaluationSet::Stratified { by: by.clone() },
            )),
            EvaluationSet::Union(sets) => {
                let mut ids = Vec::new();
                for set in sets {
                    ids.extend(self.resolve_ids(set)?);
                }
                ids.sort();
                ids.dedup();
                Ok(ids)
            }
            EvaluationSet::Intersect(sets) => {
                let Some((first, rest)) = sets.split_first() else {
                    return Ok(Vec::new());
                };
                let mut ids = self.resolve_ids(first)?;
                for set in rest {
                    let next = self.resolve_ids(set)?;
                    ids.retain(|id| next.contains(id));
                }
                Ok(ids)
            }
            EvaluationSet::Difference(left, right) => {
                let mut ids = self.resolve_ids(left)?;
                let rhs = self.resolve_ids(right)?;
                ids.retain(|id| !rhs.contains(id));
                Ok(ids)
            }
        }
    }
}

/// Builder for [`CaseSet`].
pub struct CaseSetBuilder<C> {
    cases: Vec<C>,
}

impl<C> Default for CaseSetBuilder<C> {
    fn default() -> Self {
        Self { cases: Vec::new() }
    }
}

impl<C> CaseSetBuilder<C> {
    /// Supplies ordered cases.
    #[must_use]
    pub fn cases(mut self, cases: Vec<C>) -> Self {
        self.cases = cases;
        self
    }

    /// Builds the case set.
    #[must_use]
    pub fn build(self) -> CaseSet<C> {
        CaseSet {
            cases: self
                .cases
                .into_iter()
                .enumerate()
                .map(|(index, case)| (CaseId::from_index(index), case))
                .collect(),
            partitions: BTreeMap::new(),
            version: 0,
        }
    }
}

/// Unsupported evaluation-set shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsupportedEvaluationSet {
    /// Tagged sets require a tag index that the current resolver does not own.
    Tagged(Tag),
    /// Stratified sets require a tag index that the current resolver does not own.
    Stratified { by: Tag },
}

impl fmt::Display for UnsupportedEvaluationSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tagged(tag) => write!(f, "tagged:{}", tag.0),
            Self::Stratified { by } => write!(f, "stratified-by:{}", by.0),
        }
    }
}

/// Error returned when resolving an evaluation-set expression.
#[derive(Debug, Error)]
pub enum EvaluationResolveError {
    /// The requested partition is not present in the case set.
    #[error("unknown evaluation partition `{0:?}`")]
    UnknownPartition(PartitionId),
    /// The requested case ID is not present in the case set.
    #[error("unknown case `{0}`")]
    UnknownCase(CaseId),
    /// The evaluation-set expression is valid vocabulary but unsupported here.
    #[error("unsupported evaluation set: {0}")]
    UnsupportedSet(UnsupportedEvaluationSet),
    /// A case set was required but none was installed.
    #[error("case set is required to resolve evaluation set")]
    MissingCaseSet,
}
