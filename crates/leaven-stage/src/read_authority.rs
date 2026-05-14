use leaven_kernel::{Cost, StageQueryId};

use crate::receipt::QueryTiming;
use crate::{QueryRecord, QueryRecordEffect, StageQuery};

pub struct StageReadAuthority;

#[derive(Clone, Debug)]
pub struct QueryResult {
    pub query_id: StageQueryId,
    pub timing: QueryTiming,
    pub query: StageQuery,
    pub effect: QueryEffect,
    pub cost: Cost,
}

#[derive(Clone, Debug)]
pub enum QueryEffect {
    ReturnedSummary(String),
    PolicyDenied(String),
}

impl QueryResult {
    #[must_use]
    pub fn into_record(self) -> QueryRecord {
        QueryRecord {
            query_id: self.query_id,
            timing: self.timing,
            query: self.query,
            effect: match self.effect {
                QueryEffect::ReturnedSummary(summary) => {
                    QueryRecordEffect::ReturnedSummary(summary)
                }
                QueryEffect::PolicyDenied(message) => QueryRecordEffect::PolicyDenied(message),
            },
            entries: Vec::new(),
            cost: self.cost,
        }
    }
}
