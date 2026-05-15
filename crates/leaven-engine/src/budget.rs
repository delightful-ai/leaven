//! Engine-owned budget ledger.

use std::collections::BTreeMap;

use leaven_kernel::{Budget, BudgetDimension, BudgetExceeded, BudgetSnapshot, Cost, StageId};

#[derive(Clone, Debug)]
pub struct BudgetLedger {
    limit: Budget,
    spent: Cost,
    stages: BTreeMap<StageId, Cost>,
}

impl BudgetLedger {
    #[must_use]
    pub fn new(limit: Budget) -> Self {
        Self {
            limit,
            spent: Cost::zero(),
            stages: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn from_snapshot(snapshot: BudgetSnapshot) -> Self {
        Self {
            limit: snapshot.limit,
            spent: snapshot.spent,
            stages: snapshot.stages,
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> BudgetSnapshot {
        BudgetSnapshot {
            spent: self.spent.clone(),
            limit: self.limit.clone(),
            stages: self.stages.clone(),
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

        self.spent = projected;
        self.stages
            .entry(stage)
            .and_modify(|spent| *spent = spent.clone().combine(&cost))
            .or_insert(cost);
        Ok(self.snapshot())
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

    #[must_use]
    pub fn sub_stage(&mut self, stage: StageId) -> BudgetHandle<'_> {
        BudgetHandle {
            ledger: self.ledger,
            stage,
        }
    }
}
