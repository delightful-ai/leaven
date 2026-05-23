use leaven_kernel::{AmountError, Budget, Cost};
use thiserror::Error;

use super::{AggregateBudgets, CapabilityDenial, CapabilityDenialKind, CapabilityDocument};

const AXIS_USD_MICRO: &str = "usd_micro";
const AXIS_LM_USD_MICRO: &str = "lm.usd_micro";
const AXIS_AGENT_USD_MICRO: &str = "agent.usd_micro";
const AXIS_HUMAN_USD_MICRO: &str = "human.usd_micro";
const AXIS_SANDBOX_USD_MICRO: &str = "sandbox.usd_micro";
const AXIS_EVALUATOR_USD_MICRO: &str = "evaluator.usd_micro";
const AXIS_WALL_MS: &str = "wall_ms";
const AXIS_PLAN_NODES: &str = "plan_nodes";
const AXIS_MATERIALIZED_BYTES: &str = "materialized_bytes";
const MAX_SAFE_RUNTIME_INTEGER: u64 = 9_007_199_254_740_991;

/// Aggregate capability-budget usage checked across grants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CapabilityBudgetUsage {
    other_usd_micro: u64,
    lm_usd_micro: u64,
    agent_usd_micro: u64,
    human_usd_micro: u64,
    sandbox_usd_micro: u64,
    evaluator_usd_micro: u64,
    wall_ms: u64,
    concurrent_calls: u64,
    plan_nodes: u64,
    materialized_bytes: u64,
}

impl CapabilityBudgetUsage {
    /// Records uncategorized spend against only the aggregate total.
    pub const fn usd_micro(amount: u64) -> Self {
        Self {
            other_usd_micro: amount,
            ..Self::zero()
        }
    }

    /// Records LM spend against the role-specific and aggregate totals.
    pub const fn lm_usd_micro(amount: u64) -> Self {
        Self {
            lm_usd_micro: amount,
            ..Self::zero()
        }
    }

    /// Records agent spend against the role-specific and aggregate totals.
    pub const fn agent_usd_micro(amount: u64) -> Self {
        Self {
            agent_usd_micro: amount,
            ..Self::zero()
        }
    }

    /// Records human-review spend against the role-specific and aggregate totals.
    pub const fn human_usd_micro(amount: u64) -> Self {
        Self {
            human_usd_micro: amount,
            ..Self::zero()
        }
    }

    /// Records sandbox spend against the aggregate total.
    pub const fn sandbox_usd_micro(amount: u64) -> Self {
        Self {
            sandbox_usd_micro: amount,
            ..Self::zero()
        }
    }

    /// Records evaluator spend against the aggregate total.
    pub const fn evaluator_usd_micro(amount: u64) -> Self {
        Self {
            evaluator_usd_micro: amount,
            ..Self::zero()
        }
    }

    /// Reserves concurrent-call capacity.
    pub const fn concurrent_calls(count: u64) -> Self {
        Self {
            concurrent_calls: count,
            ..Self::zero()
        }
    }

    /// Adds concurrent-call capacity to this usage reservation.
    #[must_use]
    pub const fn with_concurrent_calls(mut self, count: u64) -> Self {
        self.concurrent_calls = count;
        self
    }

    /// Records wall-clock usage in milliseconds.
    pub const fn wall_ms(amount: u64) -> Self {
        Self {
            wall_ms: amount,
            ..Self::zero()
        }
    }

    /// Records plan-node materialization usage.
    pub const fn plan_nodes(count: u64) -> Self {
        Self {
            plan_nodes: count,
            ..Self::zero()
        }
    }

    /// Records materialized bytes.
    pub const fn materialized_bytes(count: u64) -> Self {
        Self {
            materialized_bytes: count,
            ..Self::zero()
        }
    }

    /// Projects this public-seam usage reservation into runtime `Cost` axes.
    ///
    /// USD role spend is charged twice: once to the aggregate `usd_micro`
    /// axis and once to the role-specific axis. The engine ledger therefore
    /// enforces both aggregate and role ceilings from a single runtime charge.
    pub fn runtime_cost(self) -> Result<Cost, CapabilityBudgetProjectionError> {
        let mut cost = Cost::zero();
        cost = add_runtime_axis(
            cost,
            AXIS_USD_MICRO,
            self.total_usd_micro()
                .map_err(|_| CapabilityBudgetProjectionError::UsageOverflow)?,
        )?;
        cost = add_runtime_axis(cost, AXIS_LM_USD_MICRO, self.lm_usd_micro)?;
        cost = add_runtime_axis(cost, AXIS_AGENT_USD_MICRO, self.agent_usd_micro)?;
        cost = add_runtime_axis(cost, AXIS_HUMAN_USD_MICRO, self.human_usd_micro)?;
        cost = add_runtime_axis(cost, AXIS_SANDBOX_USD_MICRO, self.sandbox_usd_micro)?;
        cost = add_runtime_axis(cost, AXIS_EVALUATOR_USD_MICRO, self.evaluator_usd_micro)?;
        cost = add_runtime_axis(cost, AXIS_WALL_MS, self.wall_ms)?;
        cost = add_runtime_axis(cost, AXIS_PLAN_NODES, self.plan_nodes)?;
        add_runtime_axis(cost, AXIS_MATERIALIZED_BYTES, self.materialized_bytes)
    }

    const fn zero() -> Self {
        Self {
            other_usd_micro: 0,
            lm_usd_micro: 0,
            agent_usd_micro: 0,
            human_usd_micro: 0,
            sandbox_usd_micro: 0,
            evaluator_usd_micro: 0,
            wall_ms: 0,
            concurrent_calls: 0,
            plan_nodes: 0,
            materialized_bytes: 0,
        }
    }

    fn total_usd_micro(self) -> Result<u64, CapabilityDenial> {
        let mut total = self.other_usd_micro;
        for amount in [
            self.lm_usd_micro,
            self.agent_usd_micro,
            self.human_usd_micro,
            self.sandbox_usd_micro,
            self.evaluator_usd_micro,
        ] {
            total = checked_usage_add("max_total_usd_micro", total, amount)?;
        }
        Ok(total)
    }
}

impl CapabilityDocument {
    /// Projects aggregate capability budgets into the engine budget primitive.
    ///
    /// This is a lowering helper only. Runtime spending must still be charged
    /// through `leaven-engine::BudgetLedger` / `RunContext`; the public-seam
    /// crate does not mutate or own runtime budget state.
    pub fn runtime_budget_limit(&self) -> Result<Budget, CapabilityBudgetProjectionError> {
        self.budgets.runtime_budget_limit()
    }

    /// Projects a delegated child operation into runtime `Cost` after proving
    /// the child capability is attenuated from this parent.
    ///
    /// Callers must charge the returned cost against the parent's/shared
    /// engine `BudgetLedger`. This helper binds the child authority check to
    /// the runtime cost projection so delegated work does not get its own
    /// independent aggregate budget by accident.
    pub fn delegated_runtime_cost(
        &self,
        child: &Self,
        usage: CapabilityBudgetUsage,
    ) -> Result<Cost, CapabilityBudgetProjectionError> {
        self.validate_delegation(child)
            .map_err(CapabilityBudgetProjectionError::CapabilityDenied)?;
        usage.runtime_cost()
    }
}

/// Error returned when projecting public-seam capability budget values into
/// kernel budget/cost primitives.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum CapabilityBudgetProjectionError {
    /// The public integer value cannot be represented exactly by the current
    /// kernel continuous amount primitive.
    #[error(
        "capability budget `{axis}` value `{amount}` is too large for exact runtime projection"
    )]
    AmountNotExactlyRepresentable {
        /// Budget or cost axis.
        axis: &'static str,
        /// Rejected integer amount.
        amount: u64,
    },
    /// Kernel amount validation rejected the projected numeric value.
    #[error("capability budget `{axis}` value `{amount}` is invalid for runtime projection")]
    InvalidAmount {
        /// Budget or cost axis.
        axis: &'static str,
        /// Rejected integer amount.
        amount: u64,
        /// Kernel validation error.
        source: AmountError,
    },
    /// Summing role usage for the aggregate axis overflowed.
    #[error("capability budget aggregate usage overflowed")]
    UsageOverflow,
    /// Capability delegation or authority validation failed before projection.
    #[error(transparent)]
    CapabilityDenied(#[from] CapabilityDenial),
}

/// Aggregate capability budget ledger.
#[derive(Clone, Debug)]
pub struct CapabilityBudgetLedger {
    budgets: AggregateBudgets,
    spent: CapabilityBudgetUsage,
    in_flight_concurrent_calls: u64,
}

impl CapabilityBudgetLedger {
    /// Starts a ledger constrained by a capability document's aggregate budgets.
    pub fn new(document: &CapabilityDocument) -> Self {
        Self {
            budgets: document.budgets.clone(),
            spent: CapabilityBudgetUsage::default(),
            in_flight_concurrent_calls: 0,
        }
    }

    /// Reserves budget for an operation and records non-concurrent usage.
    pub fn try_reserve(
        &mut self,
        usage: CapabilityBudgetUsage,
    ) -> Result<CapabilityBudgetReservation, CapabilityDenial> {
        ensure_aggregate_budget(
            "max_total_usd_micro",
            self.budgets.total_usd_micro,
            self.spent.total_usd_micro()?,
            usage.total_usd_micro()?,
        )?;
        ensure_aggregate_budget(
            "max_lm_usd_micro",
            self.budgets.lm_usd_micro,
            self.spent.lm_usd_micro,
            usage.lm_usd_micro,
        )?;
        ensure_aggregate_budget(
            "max_agent_usd_micro",
            self.budgets.agent_usd_micro,
            self.spent.agent_usd_micro,
            usage.agent_usd_micro,
        )?;
        ensure_aggregate_budget(
            "max_human_usd_micro",
            self.budgets.human_usd_micro,
            self.spent.human_usd_micro,
            usage.human_usd_micro,
        )?;
        ensure_aggregate_budget(
            "max_wall_ms",
            self.budgets.wall_ms,
            self.spent.wall_ms,
            usage.wall_ms,
        )?;
        ensure_aggregate_budget(
            "max_plan_nodes",
            self.budgets.plan_nodes,
            self.spent.plan_nodes,
            usage.plan_nodes,
        )?;
        ensure_aggregate_budget(
            "max_materialized_bytes",
            self.budgets.materialized_bytes,
            self.spent.materialized_bytes,
            usage.materialized_bytes,
        )?;
        ensure_aggregate_budget(
            "max_concurrent_calls",
            self.budgets.concurrent_calls,
            self.in_flight_concurrent_calls,
            usage.concurrent_calls,
        )?;

        self.spent = add_budget_usage(self.spent, usage)?;
        self.in_flight_concurrent_calls = self
            .in_flight_concurrent_calls
            .checked_add(usage.concurrent_calls)
            .ok_or_else(|| aggregate_limit_denial("max_concurrent_calls"))?;
        Ok(CapabilityBudgetReservation {
            concurrent_calls: usage.concurrent_calls,
        })
    }

    /// Releases concurrent-call capacity for a completed operation.
    pub fn release(&mut self, reservation: CapabilityBudgetReservation) {
        self.in_flight_concurrent_calls = self
            .in_flight_concurrent_calls
            .saturating_sub(reservation.concurrent_calls);
    }

    /// Total spent in USD micro-units.
    pub fn spent_total_usd_micro(&self) -> u64 {
        self.spent
            .total_usd_micro()
            .expect("stored capability budget usage cannot overflow after reservation")
    }

    /// LM spent in USD micro-units.
    pub const fn spent_lm_usd_micro(&self) -> u64 {
        self.spent.lm_usd_micro
    }

    /// Agent spent in USD micro-units.
    pub const fn spent_agent_usd_micro(&self) -> u64 {
        self.spent.agent_usd_micro
    }

    /// Current in-flight concurrent calls.
    pub const fn in_flight_concurrent_calls(&self) -> u64 {
        self.in_flight_concurrent_calls
    }
}

/// Reservation returned by aggregate budget checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityBudgetReservation {
    concurrent_calls: u64,
}

fn ensure_aggregate_budget(
    key: &'static str,
    limit: Option<u64>,
    spent: u64,
    requested: u64,
) -> Result<(), CapabilityDenial> {
    let Some(limit) = limit else {
        return Ok(());
    };
    let projected = spent
        .checked_add(requested)
        .ok_or_else(|| aggregate_limit_denial(key))?;
    if projected > limit {
        return Err(aggregate_limit_denial(key));
    }
    Ok(())
}

fn add_budget_usage(
    spent: CapabilityBudgetUsage,
    usage: CapabilityBudgetUsage,
) -> Result<CapabilityBudgetUsage, CapabilityDenial> {
    Ok(CapabilityBudgetUsage {
        other_usd_micro: checked_usage_add(
            "max_total_usd_micro",
            spent.other_usd_micro,
            usage.other_usd_micro,
        )?,
        lm_usd_micro: checked_usage_add(
            "max_lm_usd_micro",
            spent.lm_usd_micro,
            usage.lm_usd_micro,
        )?,
        agent_usd_micro: checked_usage_add(
            "max_agent_usd_micro",
            spent.agent_usd_micro,
            usage.agent_usd_micro,
        )?,
        human_usd_micro: checked_usage_add(
            "max_human_usd_micro",
            spent.human_usd_micro,
            usage.human_usd_micro,
        )?,
        sandbox_usd_micro: checked_usage_add(
            "max_total_usd_micro",
            spent.sandbox_usd_micro,
            usage.sandbox_usd_micro,
        )?,
        evaluator_usd_micro: checked_usage_add(
            "max_total_usd_micro",
            spent.evaluator_usd_micro,
            usage.evaluator_usd_micro,
        )?,
        wall_ms: checked_usage_add("max_wall_ms", spent.wall_ms, usage.wall_ms)?,
        concurrent_calls: 0,
        plan_nodes: checked_usage_add("max_plan_nodes", spent.plan_nodes, usage.plan_nodes)?,
        materialized_bytes: checked_usage_add(
            "max_materialized_bytes",
            spent.materialized_bytes,
            usage.materialized_bytes,
        )?,
    })
}

impl AggregateBudgets {
    fn runtime_budget_limit(&self) -> Result<Budget, CapabilityBudgetProjectionError> {
        let mut budget = Budget::unlimited();
        budget.concurrent_calls = self.concurrent_calls;
        budget = add_runtime_limit(budget, AXIS_USD_MICRO, self.total_usd_micro)?;
        budget = add_runtime_limit(budget, AXIS_LM_USD_MICRO, self.lm_usd_micro)?;
        budget = add_runtime_limit(budget, AXIS_AGENT_USD_MICRO, self.agent_usd_micro)?;
        budget = add_runtime_limit(budget, AXIS_HUMAN_USD_MICRO, self.human_usd_micro)?;
        budget = add_runtime_limit(budget, AXIS_WALL_MS, self.wall_ms)?;
        budget = add_runtime_limit(budget, AXIS_PLAN_NODES, self.plan_nodes)?;
        add_runtime_limit(budget, AXIS_MATERIALIZED_BYTES, self.materialized_bytes)
    }
}

fn add_runtime_limit(
    budget: Budget,
    axis: &'static str,
    limit: Option<u64>,
) -> Result<Budget, CapabilityBudgetProjectionError> {
    let Some(limit) = limit else {
        return Ok(budget);
    };
    budget
        .with_axis_limit(axis, exact_runtime_amount(axis, limit)?)
        .map_err(|source| CapabilityBudgetProjectionError::InvalidAmount {
            axis,
            amount: limit,
            source,
        })
}

fn add_runtime_axis(
    cost: Cost,
    axis: &'static str,
    amount: u64,
) -> Result<Cost, CapabilityBudgetProjectionError> {
    if amount == 0 {
        return Ok(cost);
    }
    let axis_cost = Cost::custom(axis, exact_runtime_amount(axis, amount)?).map_err(|source| {
        CapabilityBudgetProjectionError::InvalidAmount {
            axis,
            amount,
            source,
        }
    })?;
    Ok(cost.combine(&axis_cost))
}

fn exact_runtime_amount(
    axis: &'static str,
    amount: u64,
) -> Result<f64, CapabilityBudgetProjectionError> {
    if amount > MAX_SAFE_RUNTIME_INTEGER {
        return Err(
            CapabilityBudgetProjectionError::AmountNotExactlyRepresentable { axis, amount },
        );
    }
    Ok(amount
        .to_string()
        .parse::<f64>()
        .expect("u64 below the exact f64 integer bound parses as finite f64"))
}

fn checked_usage_add(
    key: &'static str,
    spent: u64,
    requested: u64,
) -> Result<u64, CapabilityDenial> {
    spent
        .checked_add(requested)
        .ok_or_else(|| aggregate_limit_denial(key))
}

fn aggregate_limit_denial(key: &'static str) -> CapabilityDenial {
    CapabilityDenial::new(
        CapabilityDenialKind::Limit,
        format!("aggregate budget `{key}` exceeded"),
    )
}
