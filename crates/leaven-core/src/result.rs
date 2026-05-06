//! Final run result.
//!
//! Stub. The runnable engine's exit type lands in `leaven-engine`;
//! the cold core only declares the public shape.

use crate::cost::BudgetSnapshot;
use crate::graph::events::StopReason;
use crate::ids::{CandidateId, RunId};

#[derive(Clone, Debug)]
pub struct OptimizationResult {
    pub run_id: RunId,
    pub best: Option<CandidateId>,
    pub stop_reason: StopReason,
    pub budget: BudgetSnapshot,
}
