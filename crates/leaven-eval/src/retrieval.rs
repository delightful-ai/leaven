//! Ranked retrieval evaluation data and metrics.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::RankedRetrievalEvaluationError;

/// Opaque item id for ranked retrieval evaluation.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct RetrievalItemId(String);

impl From<&str> for RetrievalItemId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for RetrievalItemId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for RetrievalItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One retrieval query with one or more relevant item ids.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RetrievalQuery {
    query_id: String,
    relevant_items: BTreeSet<RetrievalItemId>,
}

impl RetrievalQuery {
    /// Builds a retrieval query.
    pub fn new(
        query_id: impl Into<String>,
        relevant_items: impl IntoIterator<Item = RetrievalItemId>,
    ) -> Result<Self, RankedRetrievalEvaluationError> {
        let query_id = query_id.into();
        let relevant_items = relevant_items.into_iter().collect::<BTreeSet<_>>();
        if relevant_items.is_empty() {
            return Err(RankedRetrievalEvaluationError::EmptyRelevantItems { query_id });
        }
        Ok(Self {
            query_id,
            relevant_items,
        })
    }

    /// Query id.
    #[must_use]
    pub fn query_id(&self) -> &str {
        &self.query_id
    }

    /// Relevant item ids.
    #[must_use]
    pub const fn relevant_items(&self) -> &BTreeSet<RetrievalItemId> {
        &self.relevant_items
    }
}

/// Ranked retrieval output for one query.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RetrievalRanking {
    query_id: String,
    ranked_items: Vec<RetrievalItemId>,
}

impl RetrievalRanking {
    /// Builds a ranked retrieval output and refuses repeated items.
    pub fn new(
        query_id: impl Into<String>,
        ranked_items: impl IntoIterator<Item = RetrievalItemId>,
    ) -> Result<Self, RankedRetrievalEvaluationError> {
        let query_id = query_id.into();
        let mut seen = BTreeSet::new();
        let mut ranked = Vec::new();
        for item in ranked_items {
            if !seen.insert(item.clone()) {
                return Err(RankedRetrievalEvaluationError::DuplicateRankedItem { query_id, item });
            }
            ranked.push(item);
        }
        Ok(Self {
            query_id,
            ranked_items: ranked,
        })
    }

    /// Query id.
    #[must_use]
    pub fn query_id(&self) -> &str {
        &self.query_id
    }

    /// Ranked item ids.
    #[must_use]
    pub fn ranked_items(&self) -> &[RetrievalItemId] {
        &self.ranked_items
    }
}

/// Ranked retrieval evaluation with Recall@K helpers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedRetrievalEvaluation {
    queries: BTreeMap<String, RetrievalQuery>,
    rankings: BTreeMap<String, RetrievalRanking>,
}

impl RankedRetrievalEvaluation {
    /// Builds a retrieval evaluation over a declared candidate universe.
    pub fn evaluate(
        candidate_universe: impl IntoIterator<Item = RetrievalItemId>,
        queries: impl IntoIterator<Item = RetrievalQuery>,
        rankings: impl IntoIterator<Item = RetrievalRanking>,
    ) -> Result<Self, RankedRetrievalEvaluationError> {
        let candidate_universe = candidate_universe.into_iter().collect::<BTreeSet<_>>();
        if candidate_universe.is_empty() {
            return Err(RankedRetrievalEvaluationError::EmptyCandidateUniverse);
        }

        let mut query_map = BTreeMap::new();
        for query in queries {
            if query_map.contains_key(query.query_id()) {
                return Err(RankedRetrievalEvaluationError::DuplicateQuery {
                    query_id: query.query_id,
                });
            }
            for item in query.relevant_items() {
                if !candidate_universe.contains(item) {
                    return Err(RankedRetrievalEvaluationError::UnknownRelevantItem {
                        query_id: query.query_id.clone(),
                        item: item.clone(),
                    });
                }
            }
            query_map.insert(query.query_id.clone(), query);
        }
        if query_map.is_empty() {
            return Err(RankedRetrievalEvaluationError::EmptyQueries);
        }

        let mut ranking_map = BTreeMap::new();
        for ranking in rankings {
            if !query_map.contains_key(ranking.query_id()) {
                return Err(RankedRetrievalEvaluationError::UnknownRankingQuery {
                    query_id: ranking.query_id,
                });
            }
            if ranking_map.contains_key(ranking.query_id()) {
                return Err(RankedRetrievalEvaluationError::DuplicateRanking {
                    query_id: ranking.query_id,
                });
            }
            for item in ranking.ranked_items() {
                if !candidate_universe.contains(item) {
                    return Err(RankedRetrievalEvaluationError::UnknownRankedItem {
                        query_id: ranking.query_id.clone(),
                        item: item.clone(),
                    });
                }
            }
            ranking_map.insert(ranking.query_id.clone(), ranking);
        }
        for query_id in query_map.keys() {
            if !ranking_map.contains_key(query_id) {
                return Err(RankedRetrievalEvaluationError::MissingRanking {
                    query_id: query_id.clone(),
                });
            }
        }

        Ok(Self {
            queries: query_map,
            rankings: ranking_map,
        })
    }

    /// Number of evaluated queries.
    #[must_use]
    pub fn query_count(&self) -> usize {
        self.queries.len()
    }

    /// Number of queries with at least one relevant item in the top `k`.
    #[must_use]
    pub fn hit_count_at(&self, k: usize) -> usize {
        self.queries
            .iter()
            .filter(|(query_id, query)| {
                self.rankings
                    .get(*query_id)
                    .expect("evaluation construction requires every query to have a ranking")
                    .ranked_items()
                    .iter()
                    .take(k)
                    .any(|item| query.relevant_items().contains(item))
            })
            .count()
    }

    /// Recall@K, counting a query as a hit when any relevant item appears in top `k`.
    #[must_use]
    pub fn recall_at(&self, k: usize) -> f64 {
        let hits = u32::try_from(self.hit_count_at(k)).expect("retrieval hit count fits in u32");
        let queries = u32::try_from(self.query_count()).expect("retrieval query count fits in u32");
        f64::from(hits) / f64::from(queries)
    }
}
