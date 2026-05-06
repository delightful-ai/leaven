//! Case-set storage and evaluation-set resolution.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use leaven_core::{CaseSetVersion, EvaluationSet, PartitionId, ResolvedEvaluationSet, Tag};
use leaven_kernel::{CaseId, ResolvedEvaluationSetId};
use thiserror::Error;

pub struct CaseSet<C> {
    cases: IndexMap<CaseId, C>,
    partitions: BTreeMap<PartitionId, Vec<CaseId>>,
    version: u64,
}

impl<C> CaseSet<C> {
    #[must_use]
    pub fn builder() -> CaseSetBuilder<C> {
        CaseSetBuilder::default()
    }

    #[must_use]
    pub fn new(cases: Vec<C>) -> Self {
        Self::builder().cases(cases).build()
    }

    #[must_use]
    pub fn with_partition(mut self, partition: PartitionId, case_ids: Vec<CaseId>) -> Self {
        self.partitions.insert(partition, case_ids);
        self.version += 1;
        self
    }

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
            EvaluationSet::Tagged(Tag(tag)) => Err(EvaluationResolveError::UnsupportedSet(
                format!("tagged:{tag}"),
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
            EvaluationSet::Stratified { of, k, .. } => {
                let mut ids = self.resolve_ids(of)?;
                ids.truncate(*k);
                Ok(ids)
            }
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

pub struct CaseSetBuilder<C> {
    cases: Vec<C>,
}

impl<C> Default for CaseSetBuilder<C> {
    fn default() -> Self {
        Self { cases: Vec::new() }
    }
}

impl<C> CaseSetBuilder<C> {
    #[must_use]
    pub fn cases(mut self, cases: Vec<C>) -> Self {
        self.cases = cases;
        self
    }

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

#[derive(Debug, Error)]
pub enum EvaluationResolveError {
    #[error("unknown evaluation partition `{0:?}`")]
    UnknownPartition(PartitionId),
    #[error("unknown case `{0}`")]
    UnknownCase(CaseId),
    #[error("unsupported evaluation set: {0}")]
    UnsupportedSet(String),
    #[error("case set is required to resolve evaluation set")]
    MissingCaseSet,
}
