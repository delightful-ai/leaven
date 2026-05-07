//! Preference result.

/// The four-valued result of comparing two candidates.
///
/// Cold core defines only the result type; the trait that *produces* the
/// result (`PreferenceRelation`) lives in `leaven-engine` because it
/// reads from a `RunGraphView` to do its work.
///
/// # Why four values, not three
///
/// Pareto-style comparisons can be partial: two candidates that win on
/// different axes are neither equivalent (their evidence differs) nor
/// ordered. [`Incomparable`](Preference::Incomparable) is a real,
/// load-bearing answer; collapsing it to "equivalent" makes downstream
/// logic silently wrong. Optimizers that need a total order must reduce
/// `Incomparable` deliberately (lexicographic tiebreak, scalarization,
/// etc.).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Preference {
    /// The left-hand candidate dominates.
    LeftBetter,
    /// The right-hand candidate dominates.
    RightBetter,
    /// Both candidates have indistinguishable evidence under this relation.
    Equivalent,
    /// The relation is partial and gives no answer for this pair.
    Incomparable,
}
