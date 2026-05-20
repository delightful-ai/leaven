//! Evaluation case samplers.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use leaven_kernel::CaseId;
use smol_str::SmolStr;

use crate::SamplerError;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CategorySample {
    pub category: SmolStr,
    pub case: CaseId,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CategoryRoundRobinSampler {
    pools: BTreeMap<SmolStr, Vec<CaseId>>,
    categories: Vec<SmolStr>,
    categories_per_batch: NonZeroUsize,
    samples_per_category: NonZeroUsize,
    category_cursor: usize,
    case_cursors: BTreeMap<SmolStr, usize>,
    sampled_cases: BTreeSet<CaseId>,
}

impl CategoryRoundRobinSampler {
    pub fn new(
        pools: BTreeMap<SmolStr, Vec<CaseId>>,
        categories_per_batch: NonZeroUsize,
        samples_per_category: NonZeroUsize,
    ) -> Result<Self, SamplerError> {
        if pools.is_empty() {
            return Err(SamplerError::NoCategories);
        }

        let mut seen = BTreeMap::<CaseId, SmolStr>::new();
        for (category, cases) in &pools {
            if cases.is_empty() {
                return Err(SamplerError::EmptyCategory(category.clone()));
            }
            for case in cases {
                if let Some(left) = seen.insert(*case, category.clone()) {
                    return Err(SamplerError::DuplicateCaseCategory {
                        case: *case,
                        left,
                        right: category.clone(),
                    });
                }
            }
        }

        let categories = pools.keys().cloned().collect::<Vec<_>>();
        let case_cursors = categories
            .iter()
            .cloned()
            .map(|category| (category, 0))
            .collect();
        Ok(Self {
            pools,
            categories,
            categories_per_batch,
            samples_per_category,
            category_cursor: 0,
            case_cursors,
            sampled_cases: BTreeSet::new(),
        })
    }

    #[must_use]
    pub const fn category_cursor(&self) -> usize {
        self.category_cursor
    }

    #[must_use]
    pub fn case_cursor(&self, category: &str) -> Option<usize> {
        self.case_cursors.get(category).copied()
    }

    #[must_use]
    pub fn categories(&self) -> &[SmolStr] {
        &self.categories
    }

    #[must_use]
    pub fn next_batch(&mut self) -> Vec<CategorySample> {
        let selected_category_count = self.categories_per_batch.get().min(self.categories.len());
        let mut batch = Vec::new();
        for offset in 0..selected_category_count {
            let category_index = (self.category_cursor + offset) % self.categories.len();
            let category = self.categories[category_index].clone();
            self.take_category_samples(&category, &mut batch);
        }
        self.category_cursor += selected_category_count;
        batch
    }

    fn take_category_samples(&mut self, category: &SmolStr, batch: &mut Vec<CategorySample>) {
        let pool = self
            .pools
            .get(category)
            .expect("constructor rejects missing category pools");
        let sample_count = self.samples_per_category.get().min(pool.len());
        let cursor = self
            .case_cursors
            .get_mut(category)
            .expect("constructor initializes every category cursor");
        for _ in 0..sample_count {
            let case_index = *cursor % pool.len();
            batch.push(CategorySample {
                category: category.clone(),
                case: pool[case_index],
            });
            self.sampled_cases.insert(pool[case_index]);
            *cursor += 1;
        }
    }

    #[must_use]
    pub fn sampled_cases(&self) -> BTreeSet<CaseId> {
        self.sampled_cases.clone()
    }
}
