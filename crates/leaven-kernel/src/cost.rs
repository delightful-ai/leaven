//! Cost accounting and budget enforcement.
//!
//! Cost in Leaven is *infrastructure*, not proposal metadata. Every
//! side-effectful stage — proposer, evaluator, renderer, agent runtime,
//! cache miss — charges the central ledger so retrospective analysis can
//! answer "where did the run spend?" without reconstructing it from logs.
//!
//! # Vocabulary
//!
//! - [`Cost`] — what was spent. Multi-axis: metric calls, LLM calls, prompt
//!   and completion tokens, seconds, plus user-defined axes.
//! - [`Amount`] — finite, non-negative `f64`. Used for every continuous-cost
//!   axis to make NaN/negative bugs unrepresentable.
//! - [`Budget`] — what *may* be spent. A configured ceiling, possibly per
//!   axis, possibly unlimited.
//! - [`BudgetSnapshot`] — what *has* been spent. Authoritative ledger state,
//!   broken down by stage.
//! - [`Metered<T>`] — a value paired with the cost paid to produce it. Stage
//!   trait return types use this so producers cannot forget to report cost.
//! - [`BudgetExceeded`] — refusal-to-charge error. Carries the exact
//!   dimension, requested amount, and snapshot at the time of refusal so
//!   the caller can react meaningfully.
//!
//! Stage charges go through a `BudgetHandle` (defined in `leaven-engine`)
//! that holds a stage-tagged mutable reference to the ledger. Stages never
//! touch the ledger directly.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

use crate::StageId;

/// Unit attached to a cost axis.
///
/// Every numeric cost axis has a unit so reporting and aggregation know
/// what they're summing. `UsdMicro` (one millionth of a USD) is integer-
/// friendly — small enough to express LLM call costs without losing
/// precision, large enough to avoid `i64` overflow over long runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum CostUnit {
    /// Unitless count.
    Count,
    /// Token count.
    Token,
    /// Wall-clock or CPU seconds.
    Second,
    /// One millionth of a USD.
    UsdMicro,
    /// Caller-defined unit.
    Custom,
}

/// Named axis used for cost accounting.
///
/// Pairs a human-readable label with a [`CostUnit`]. Used when describing
/// cost axes in reports and configuration; charges themselves go through
/// the named fields on [`Cost`] (and the `other` map for caller-defined
/// axes).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CostAxis {
    /// Human-readable axis name.
    pub name: String,
    /// Unit used by this axis.
    pub unit: CostUnit,
}

/// Error returned when constructing an [`Amount`] from an invalid `f64`.
///
/// `Amount`'s validation is the single boundary where stray NaN or negative
/// numbers entering from external sources (configuration, deserialization,
/// stage return values) are caught and turned into a typed error rather
/// than propagated into ledgers and budgets.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum AmountError {
    /// Value was NaN or infinite.
    #[error("amount must be finite, got {value}")]
    NonFinite {
        /// Rejected value.
        value: f64,
    },
    /// Value was finite but negative.
    #[error("amount must be non-negative, got {value}")]
    Negative {
        /// Rejected value.
        value: f64,
    },
}

/// Finite, non-negative amount used by costs and budgets.
///
/// Wraps `f64` and refuses to construct from NaN, infinity, or negative
/// numbers. Once constructed, an `Amount` is safe to compare, sum, and
/// serialize without re-validating.
///
/// Saturating arithmetic is used internally — adding two finite `Amount`s
/// whose sum overflows `f64` clamps to `f64::MAX` rather than producing
/// infinity. This keeps ledger invariants intact under pathological
/// inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Amount(f64);

impl Amount {
    /// The zero amount.
    pub const ZERO: Self = Self(0.0);

    /// Constructs a finite, non-negative amount.
    ///
    /// # Errors
    ///
    /// - [`AmountError::NonFinite`] — `value` is NaN or `±infinity`.
    /// - [`AmountError::Negative`] — `value` is finite but below zero.
    pub fn new(value: f64) -> Result<Self, AmountError> {
        if !value.is_finite() {
            return Err(AmountError::NonFinite { value });
        }
        if value < 0.0 {
            return Err(AmountError::Negative { value });
        }
        Ok(Self(value))
    }

    /// Returns the zero amount.
    #[must_use]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Returns the underlying numeric value.
    #[must_use]
    pub const fn as_f64(self) -> f64 {
        self.0
    }

    /// Returns true when this amount is exactly zero.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    fn saturating_add(self, rhs: Self) -> Self {
        let sum = self.0 + rhs.0;
        if sum.is_finite() {
            Self(sum)
        } else {
            Self(f64::MAX)
        }
    }
}

impl TryFrom<f64> for Amount {
    type Error = AmountError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Amount> for f64 {
    fn from(amount: Amount) -> Self {
        amount.0
    }
}

impl Serialize for Amount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

/// Multi-axis cost incurred by a stage.
///
/// Common axes — metric calls, LLM calls, prompt and completion tokens,
/// seconds — are first-class fields so charges and aggregates are
/// allocation-free and explicit. The `other` map carries user-defined
/// axes (subprocess invocations, GPU minutes, custom unit-test buckets)
/// keyed by name.
///
/// `Cost` values are additive via [`combine`]: producing a stage's total
/// cost is "fold over your sub-costs." Constructing a fresh `Cost` for
/// each charge keeps stage code obvious — no shared mutable accumulator.
///
/// [`combine`]: Cost::combine
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    /// Number of metric calls.
    pub metric_calls: u64,
    /// Number of LLM calls.
    pub llm_calls: u64,
    /// Prompt/input tokens.
    pub prompt_tokens: u64,
    /// Completion/output tokens.
    pub completion_tokens: u64,
    /// Seconds spent by this stage.
    pub seconds: Amount,
    /// Caller-defined cost axes keyed by name.
    pub other: BTreeMap<String, Amount>,
}

impl Cost {
    /// Returns a cost with every axis at zero.
    #[must_use]
    pub fn zero() -> Self {
        Self::default()
    }

    /// Returns a cost charging only `metric_calls`.
    #[must_use]
    pub fn metric_calls(n: u64) -> Self {
        Self {
            metric_calls: n,
            ..Self::default()
        }
    }

    /// Returns a cost charging only `llm_calls`.
    #[must_use]
    pub fn llm_calls(n: u64) -> Self {
        Self {
            llm_calls: n,
            ..Self::default()
        }
    }

    /// Returns a cost charging only token counts.
    #[must_use]
    pub fn tokens(prompt: u64, completion: u64) -> Self {
        Self {
            prompt_tokens: prompt,
            completion_tokens: completion,
            ..Self::default()
        }
    }

    /// Returns a cost charging only seconds.
    ///
    /// # Errors
    ///
    /// Forwards [`AmountError`] when `seconds` is NaN, infinite, or
    /// negative. Use [`Amount::new`] directly if you want to reject
    /// invalid values up-front.
    pub fn seconds(seconds: f64) -> Result<Self, AmountError> {
        Ok(Self {
            seconds: Amount::new(seconds)?,
            ..Self::default()
        })
    }

    /// Returns a cost charging one caller-defined axis.
    ///
    /// # Errors
    ///
    /// Forwards [`AmountError`] when `amount` is NaN, infinite, or negative.
    pub fn custom(axis: impl Into<String>, amount: f64) -> Result<Self, AmountError> {
        let mut other = BTreeMap::new();
        other.insert(axis.into(), Amount::new(amount)?);
        Ok(Self {
            other,
            ..Self::default()
        })
    }

    /// Sums two costs axis-wise, saturating continuous axes at `f64::MAX`.
    ///
    /// Integer axes (`metric_calls`, `llm_calls`, token counts) wrap on
    /// overflow per Rust's debug/release defaults; this is intentional
    /// because hitting `u64::MAX` of any of those means cost accounting
    /// is already broken in a way saturation wouldn't fix.
    #[must_use]
    pub fn combine(mut self, rhs: &Self) -> Self {
        self.metric_calls += rhs.metric_calls;
        self.llm_calls += rhs.llm_calls;
        self.prompt_tokens += rhs.prompt_tokens;
        self.completion_tokens += rhs.completion_tokens;
        self.seconds = self.seconds.saturating_add(rhs.seconds);
        for (key, value) in &rhs.other {
            let current = self.other.entry(key.clone()).or_insert_with(Amount::zero);
            *current = current.saturating_add(*value);
        }
        self
    }

    /// Returns true when every axis is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.metric_calls == 0
            && self.llm_calls == 0
            && self.prompt_tokens == 0
            && self.completion_tokens == 0
            && self.seconds.is_zero()
            && self.other.values().all(|amount| amount.is_zero())
    }
}

/// A value paired with the cost paid to produce it.
///
/// Stage trait return types use `Metered<T>` so producers cannot forget to
/// report cost. The framework reads `cost` to update the ledger and emits a
/// `BudgetCharged` event before yielding `value` to the caller.
///
/// # Why this exists as a type
///
/// Without `Metered`, every stage signature would be either `(T, Cost)`
/// (forgettable) or `T` plus a separate ledger-charging side channel
/// (untyped). Pairing them at the type level means the cost story is
/// uniform across proposers, evaluators, renderers, and agent runtimes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Metered<T> {
    /// Produced value.
    pub value: T,
    /// Cost incurred while producing the value.
    pub cost: Cost,
}

impl<T> Metered<T> {
    /// Pairs a value with its cost.
    #[must_use]
    pub fn new(value: T, cost: Cost) -> Self {
        Self { value, cost }
    }

    /// Maps the inner value while preserving the cost.
    ///
    /// Use this to adapt between stage shapes without losing the cost
    /// breadcrumb — the cost stays attached to the work that produced
    /// the value, regardless of how the value is reshaped downstream.
    #[must_use]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Metered<U> {
        Metered {
            value: f(self.value),
            cost: self.cost,
        }
    }
}

/// Configured upper bounds on what the run is allowed to spend.
///
/// Each axis is independent and `None` means "no ceiling on this axis."
/// `Budget::unlimited()` is the all-`None` default — useful for
/// development and tests; production runs should set at least one cap to
/// avoid runaway spending in error paths.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    /// Optional metric-call limit.
    pub metric_calls: Option<u64>,
    /// Optional LLM-call limit.
    pub llm_calls: Option<u64>,
    /// Optional seconds limit.
    pub seconds: Option<Amount>,
    /// Optional concurrent-call reservation limit.
    pub concurrent_calls: Option<u64>,
    /// Optional caller-defined axis limits keyed by cost axis name.
    pub other: BTreeMap<String, Amount>,
}

impl Budget {
    /// Returns a budget with no axis limits.
    #[must_use]
    pub fn unlimited() -> Self {
        Self::default()
    }

    /// Returns a budget capped at `n` metric calls and unlimited otherwise.
    #[must_use]
    pub fn metric_calls(n: u64) -> Self {
        Self {
            metric_calls: Some(n),
            ..Self::default()
        }
    }

    /// Returns a budget capped at `seconds` of stage time and unlimited
    /// otherwise.
    ///
    /// # Errors
    ///
    /// Forwards [`AmountError`] when `seconds` is NaN, infinite, or
    /// negative.
    pub fn seconds(seconds: f64) -> Result<Self, AmountError> {
        Ok(Self {
            seconds: Some(Amount::new(seconds)?),
            ..Self::default()
        })
    }

    /// Sets a caller-defined axis limit.
    ///
    /// # Errors
    ///
    /// Forwards [`AmountError`] when `amount` is NaN, infinite, or negative.
    pub fn with_axis_limit(
        mut self,
        axis: impl Into<String>,
        amount: f64,
    ) -> Result<Self, AmountError> {
        self.other.insert(axis.into(), Amount::new(amount)?);
        Ok(self)
    }
}

/// Authoritative point-in-time view of run spending.
///
/// `spent` is the aggregate ledger; `stages` breaks the same total down by
/// [`StageId`] so retrospective analysis can answer "where did this run
/// burn its budget?" without reconstructing it from event logs. `limit`
/// preserves the configured ceiling for context.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    /// Total cost spent across all stages.
    pub spent: Cost,
    /// Configured budget limit.
    pub limit: Budget,
    /// Cost spent by stage.
    pub stages: BTreeMap<StageId, Cost>,
    /// In-flight calls currently reserving concurrency capacity.
    pub in_flight_calls: u64,
}

/// Which axis tripped a [`BudgetExceeded`] refusal.
///
/// `BudgetDimension` is a deliberately small enum because [`Budget`]
/// itself only caps three axes today. Extending the budget to cap
/// caller-defined axes will require extending this enum.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum BudgetDimension {
    /// Metric call limit.
    MetricCalls,
    /// LLM call limit.
    LlmCalls,
    /// Seconds limit.
    Seconds,
    /// Concurrent-call reservation limit.
    ConcurrentCalls,
    /// Caller-defined cost axis.
    Other(String),
}

/// Refusal returned when charging a cost would exceed the budget.
///
/// The error carries enough context to react meaningfully without
/// re-querying the ledger:
///
/// - `dimension` — which axis blew the cap.
/// - `requested` — the cost the stage tried to charge.
/// - `snapshot` — ledger state *before* the refused charge, so
///   `requested + snapshot.spent` reproduces the would-be post-charge
///   total.
/// - `stage` — which [`StageId`] was charging when the refusal happened.
///
/// Both `requested` and `snapshot` are boxed because the error path is
/// cold and clones happen often (it lands in run events, error records,
/// and stop reasons).
#[derive(Clone, Debug, Error)]
#[error("budget exceeded for {dimension:?} at stage {stage}")]
pub struct BudgetExceeded {
    /// Stage attempting to spend.
    pub stage: StageId,
    /// Cost the stage requested to charge.
    pub requested: Box<Cost>,
    /// Ledger state before applying the refused charge.
    pub snapshot: Box<BudgetSnapshot>,
    /// Dimension that would exceed the limit.
    pub dimension: BudgetDimension,
}
