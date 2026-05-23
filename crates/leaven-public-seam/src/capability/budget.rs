use super::{AggregateBudgets, CapabilityDenial, CapabilityDenialKind, CapabilityDocument};

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
