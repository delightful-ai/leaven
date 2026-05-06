use leaven_engine::{BudgetLedger, RunContext};
use leaven_kernel::{Amount, Budget, BudgetDimension, Cost, RunId, StageId};

mod support;

use support::{TestProblem, graph_and_budget};

#[test]
fn budget_ledger_charges_each_supported_axis() {
    let mut ledger = BudgetLedger::new(Budget {
        metric_calls: Some(2),
        llm_calls: Some(2),
        seconds: Some(Amount::new(2.0).unwrap()),
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
