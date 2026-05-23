//! Engine-owned budget ledger.

use std::collections::BTreeMap;

use leaven_kernel::{Budget, BudgetDimension, BudgetExceeded, BudgetSnapshot, Cost, StageId};

#[derive(Clone, Debug)]
pub struct BudgetLedger {
    limit: Budget,
    spent: Cost,
    stages: BTreeMap<StageId, Cost>,
    in_flight_calls: u64,
}

impl BudgetLedger {
    #[must_use]
    pub fn new(limit: Budget) -> Self {
        Self {
            limit,
            spent: Cost::zero(),
            stages: BTreeMap::new(),
            in_flight_calls: 0,
        }
    }

    #[must_use]
    pub fn from_snapshot(snapshot: BudgetSnapshot) -> Self {
        Self {
            limit: snapshot.limit,
            spent: snapshot.spent,
            stages: snapshot.stages,
            in_flight_calls: snapshot.in_flight_calls,
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> BudgetSnapshot {
        BudgetSnapshot {
            spent: self.spent.clone(),
            limit: self.limit.clone(),
            stages: self.stages.clone(),
            in_flight_calls: self.in_flight_calls,
        }
    }

    pub fn set_limit(&mut self, limit: Budget) {
        self.limit = limit;
    }

    pub fn charge(&mut self, stage: StageId, cost: Cost) -> Result<BudgetSnapshot, BudgetExceeded> {
        let projected = self.spent.clone().combine(&cost);
        if let Some(limit) = self.limit.metric_calls {
            if projected.metric_calls > limit {
                return Err(BudgetExceeded {
                    stage,
                    requested: Box::new(cost),
                    snapshot: Box::new(self.snapshot()),
                    dimension: BudgetDimension::MetricCalls,
                });
            }
        }
        if let Some(limit) = self.limit.llm_calls {
            if projected.llm_calls > limit {
                return Err(BudgetExceeded {
                    stage,
                    requested: Box::new(cost),
                    snapshot: Box::new(self.snapshot()),
                    dimension: BudgetDimension::LlmCalls,
                });
            }
        }
        if let Some(limit) = self.limit.seconds {
            if projected.seconds > limit {
                return Err(BudgetExceeded {
                    stage,
                    requested: Box::new(cost),
                    snapshot: Box::new(self.snapshot()),
                    dimension: BudgetDimension::Seconds,
                });
            }
        }
        for (axis, limit) in &self.limit.other {
            let projected_amount = projected.other.get(axis).copied().unwrap_or_default();
            if projected_amount > *limit {
                return Err(BudgetExceeded {
                    stage,
                    requested: Box::new(cost),
                    snapshot: Box::new(self.snapshot()),
                    dimension: BudgetDimension::Other(axis.clone()),
                });
            }
        }

        self.spent = projected;
        self.stages
            .entry(stage)
            .and_modify(|spent| *spent = spent.clone().combine(&cost))
            .or_insert(cost);
        Ok(self.snapshot())
    }

    pub fn begin_concurrent_call(
        &mut self,
        stage: StageId,
    ) -> Result<BudgetSnapshot, BudgetExceeded> {
        let projected = self.in_flight_calls.saturating_add(1);
        if let Some(limit) = self.limit.concurrent_calls {
            if projected > limit {
                return Err(BudgetExceeded {
                    stage,
                    requested: Box::new(Cost::zero()),
                    snapshot: Box::new(self.snapshot()),
                    dimension: BudgetDimension::ConcurrentCalls,
                });
            }
        }
        self.in_flight_calls = projected;
        Ok(self.snapshot())
    }

    pub fn end_concurrent_call(&mut self) {
        self.in_flight_calls = self.in_flight_calls.saturating_sub(1);
    }
}

impl Default for BudgetLedger {
    fn default() -> Self {
        Self::new(Budget::unlimited())
    }
}

pub struct BudgetHandle<'a> {
    ledger: &'a mut BudgetLedger,
    stage: StageId,
}

impl<'a> BudgetHandle<'a> {
    pub(crate) fn new(ledger: &'a mut BudgetLedger, stage: StageId) -> Self {
        Self { ledger, stage }
    }

    #[must_use]
    pub fn snapshot(&self) -> BudgetSnapshot {
        self.ledger.snapshot()
    }

    pub fn charge(&mut self, cost: Cost) -> Result<BudgetSnapshot, BudgetExceeded> {
        self.ledger.charge(self.stage.clone(), cost)
    }

    pub fn begin_concurrent_call(&mut self) -> Result<BudgetSnapshot, BudgetExceeded> {
        self.ledger.begin_concurrent_call(self.stage.clone())
    }

    pub fn end_concurrent_call(&mut self) {
        self.ledger.end_concurrent_call();
    }

    #[must_use]
    pub fn sub_stage(&mut self, stage: StageId) -> BudgetHandle<'_> {
        BudgetHandle {
            ledger: self.ledger,
            stage,
        }
    }
}
