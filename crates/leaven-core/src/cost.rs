//! Cost and budget primitives.
//!
//! Cost is infrastructure: every metered stage invocation produces a
//! [`Cost`] which the [`BudgetLedger`] aggregates. The ledger tracks
//! per-stage spend and a small set of named global limits. Optimizers
//! that need richer cost dimensions add them to [`Cost::other`].
//!
//! Following the type-design rule that information holds its shape:
//! costs are kept in their original units (calls, tokens, seconds)
//! rather than collapsed into a single scalar. Users who want a scalar
//! budget supply a function that projects [`Cost`] to one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::StageId;

/// Multi-dimensional cost incurred by a stage invocation.
///
/// All dimensions default to zero; stages set only the ones they spend.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    /// Number of full evaluator runs (one input → one assessment).
    pub metric_calls: u64,
    /// Number of LLM invocations.
    pub llm_calls: u64,
    /// Prompt tokens spent across LLM calls.
    pub prompt_tokens: u64,
    /// Completion tokens spent across LLM calls.
    pub completion_tokens: u64,
    /// Wall-clock seconds spent in the stage.
    pub seconds: f64,
    /// Open extension for problem-specific cost dimensions.
    pub other: BTreeMap<String, f64>,
}

impl Cost {
    #[must_use]
    pub fn zero() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn metric_calls(n: u64) -> Self {
        Self {
            metric_calls: n,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn llm_calls(n: u64) -> Self {
        Self {
            llm_calls: n,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn tokens(prompt: u64, completion: u64) -> Self {
        Self {
            prompt_tokens: prompt,
            completion_tokens: completion,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn seconds(s: f64) -> Self {
        Self {
            seconds: s,
            ..Self::default()
        }
    }

    /// Additive combination. The "other" dimension uses sum semantics.
    /// Named `combine` rather than `add` to avoid shadowing
    /// `std::ops::Add::add`.
    #[must_use]
    pub fn combine(mut self, rhs: &Self) -> Self {
        self.metric_calls += rhs.metric_calls;
        self.llm_calls += rhs.llm_calls;
        self.prompt_tokens += rhs.prompt_tokens;
        self.completion_tokens += rhs.completion_tokens;
        self.seconds += rhs.seconds;
        for (k, v) in &rhs.other {
            *self.other.entry(k.clone()).or_insert(0.0) += v;
        }
        self
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.metric_calls == 0
            && self.llm_calls == 0
            && self.prompt_tokens == 0
            && self.completion_tokens == 0
            && self.seconds == 0.0
            && self.other.values().all(|v| *v == 0.0)
    }
}

/// A wrapped value plus the cost paid to produce it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Metered<T> {
    pub value: T,
    pub cost: Cost,
}

impl<T> Metered<T> {
    pub fn new(value: T, cost: Cost) -> Self {
        Self { value, cost }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Metered<U> {
        Metered {
            value: f(self.value),
            cost: self.cost,
        }
    }
}

/// Per-stage and global budget caps. Currently only metric calls and
/// LLM calls are first-class caps; the spec leaves richer limits to
/// later iterations.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Budget {
    pub metric_calls: Option<u64>,
    pub llm_calls: Option<u64>,
    pub seconds: Option<f64>,
}

impl Budget {
    #[must_use]
    pub fn unlimited() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn metric_calls(n: u64) -> Self {
        Self {
            metric_calls: Some(n),
            ..Self::default()
        }
    }
}

/// Read-only snapshot of a `BudgetLedger`'s remaining caps and totals.
///
/// Returned by [`crate::context::run_context`] surfaces and embedded
/// into [`crate::graph::events::RunEvent::BudgetCharged`] payloads.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    pub spent: Cost,
    pub limit: Budget,
    pub stages: BTreeMap<StageId, Cost>,
}
