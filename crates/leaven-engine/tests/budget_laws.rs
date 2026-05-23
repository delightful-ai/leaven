use leaven_engine::{BudgetLedger, RunContext};
use leaven_kernel::{Amount, Budget, BudgetDimension, Cost, RunId, StageId};

mod support;

use support::{TestProblem, graph_and_budget};

fn usd_micro(amount: f64) -> Cost {
    Cost::custom("usd_micro", amount).unwrap()
}

fn role_usd_micro(role: &str, amount: f64) -> Cost {
    Cost::custom(format!("{role}.usd_micro"), amount).unwrap()
}

#[test]
fn budget_ledger_charges_each_supported_axis() {
    let mut ledger = BudgetLedger::new(Budget {
        metric_calls: Some(2),
        llm_calls: Some(2),
        seconds: Some(Amount::new(2.0).unwrap()),
        ..Budget::unlimited()
    });

    ledger
        .charge(StageId::custom("metric"), Cost::metric_calls(1))
        .unwrap();
    ledger
        .charge(StageId::custom("llm"), Cost::llm_calls(1))
        .unwrap();
    let snapshot = ledger
        .charge(StageId::custom("seconds"), Cost::seconds(1.5).unwrap())
        .unwrap();

    assert_eq!(snapshot.spent.metric_calls, 1);
    assert_eq!(snapshot.spent.llm_calls, 1);
    assert!((snapshot.spent.seconds.as_f64() - 1.5).abs() < f64::EPSILON);
    assert_eq!(snapshot.stages.len(), 3);
}

#[test]
fn budget_ledger_refuses_each_supported_axis_without_charging() {
    let mut ledger = BudgetLedger::new(Budget {
        metric_calls: Some(0),
        llm_calls: Some(0),
        seconds: Some(Amount::zero()),
        ..Budget::unlimited()
    });

    let metric = ledger
        .charge(StageId::custom("metric"), Cost::metric_calls(1))
        .unwrap_err();
    let llm = ledger
        .charge(StageId::custom("llm"), Cost::llm_calls(1))
        .unwrap_err();
    let seconds = ledger
        .charge(StageId::custom("seconds"), Cost::seconds(0.1).unwrap())
        .unwrap_err();

    assert_eq!(metric.dimension, BudgetDimension::MetricCalls);
    assert_eq!(llm.dimension, BudgetDimension::LlmCalls);
    assert_eq!(seconds.dimension, BudgetDimension::Seconds);
    assert!(ledger.snapshot().spent.is_zero());
}

#[test]
fn budget_ledger_enforces_aggregate_and_role_specific_custom_axes() {
    let mut ledger = BudgetLedger::new(
        Budget::unlimited()
            .with_axis_limit("usd_micro", 100_000.0)
            .unwrap()
            .with_axis_limit("lm.usd_micro", 80_000.0)
            .unwrap()
            .with_axis_limit("agent.usd_micro", 80_000.0)
            .unwrap(),
    );

    ledger
        .charge(
            StageId::custom("lm.complete"),
            usd_micro(70_000.0).combine(&role_usd_micro("lm", 70_000.0)),
        )
        .unwrap();

    let aggregate = ledger
        .charge(
            StageId::custom("agent.run"),
            usd_micro(40_000.0).combine(&role_usd_micro("agent", 40_000.0)),
        )
        .unwrap_err();

    assert_eq!(
        aggregate.dimension,
        BudgetDimension::Other("usd_micro".to_owned())
    );
    assert_eq!(
        ledger
            .snapshot()
            .spent
            .other
            .get("usd_micro")
            .copied()
            .unwrap(),
        Amount::new(70_000.0).unwrap()
    );

    let mut ledger = BudgetLedger::new(
        Budget::unlimited()
            .with_axis_limit("usd_micro", 200_000.0)
            .unwrap()
            .with_axis_limit("lm.usd_micro", 80_000.0)
            .unwrap(),
    );
    let role = ledger
        .charge(
            StageId::custom("lm.complete"),
            usd_micro(90_000.0).combine(&role_usd_micro("lm", 90_000.0)),
        )
        .unwrap_err();

    assert_eq!(
        role.dimension,
        BudgetDimension::Other("lm.usd_micro".to_owned())
    );
    assert!(ledger.snapshot().spent.is_zero());
}

#[test]
fn budget_ledger_enforces_concurrent_call_reservations() {
    let mut ledger = BudgetLedger::new(Budget {
        concurrent_calls: Some(1),
        ..Budget::unlimited()
    });

    let first = ledger
        .begin_concurrent_call(StageId::custom("lm.complete"))
        .unwrap();
    assert_eq!(first.in_flight_calls, 1);

    let refused = ledger
        .begin_concurrent_call(StageId::custom("agent.run"))
        .unwrap_err();
    assert_eq!(refused.dimension, BudgetDimension::ConcurrentCalls);
    assert_eq!(ledger.snapshot().in_flight_calls, 1);

    ledger.end_concurrent_call();
    assert_eq!(ledger.snapshot().in_flight_calls, 0);
    ledger
        .begin_concurrent_call(StageId::custom("sandbox.exec"))
        .unwrap();
    assert_eq!(ledger.snapshot().in_flight_calls, 1);
}

#[test]
fn budget_amounts_refuse_nan_infinite_and_negative_values() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        assert!(Amount::new(value).is_err());
        assert!(Cost::seconds(value).is_err());
        assert!(Budget::seconds(value).is_err());
    }
}

#[test]
fn budget_seconds_cannot_be_bypassed_by_nan_costs() {
    let ledger = BudgetLedger::new(Budget::seconds(0.0).unwrap());
    let invalid_cost = Cost::seconds(f64::NAN);

    assert!(invalid_cost.is_err());
    assert!(ledger.snapshot().spent.is_zero());
}

#[test]
fn budget_handles_can_charge_substages() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let mut proposal_ctx = ctx.proposal_context(StageId::custom("root"));
    let mut sub = proposal_ctx
        .budget_handle()
        .sub_stage(StageId::custom("child"));

    let snapshot = sub.charge(Cost::metric_calls(1)).unwrap();

    assert_eq!(snapshot.spent.metric_calls, 1);
    assert!(snapshot.stages.contains_key(&StageId::custom("child")));
}

#[test]
fn default_graph_and_budget_are_empty_and_unlimited() {
    let mut graph = leaven_engine::RunGraph::<TestProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);

    assert_eq!(ctx.graph().candidate_count(), 0);
    assert!(ctx.budget().limit.metric_calls.is_none());
}
