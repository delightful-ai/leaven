use leaven_core::Evidence;
use leaven_kernel::FiniteF64;

/// Evidence that attributes behavior to arbitrary keys.
///
/// Keys may be surface part IDs, paths, agents, changesets, tools,
/// modules, conflict regions, or any user-defined key.
pub trait AttributableEvidence<K>: Evidence {
    /// Returns all attribution records carried by this evidence.
    fn attributions(&self) -> Vec<Attribution<K>>;

    /// Returns human-readable evidence for one key, when available.
    fn evidence_for(&self, key: &K) -> Option<String>;
}

/// One attribution from an evidence item to a caller-defined key.
#[derive(Clone, Debug)]
pub struct Attribution<K> {
    /// Key this evidence refers to.
    pub key: K,
    /// Optional signed finite weight. Normalization is domain-specific.
    pub weight: Option<FiniteF64>,
    /// Optional human-readable note about the attribution.
    pub note: Option<String>,
}

/// Marker bound for values usable as attribution keys.
pub trait AttributionKey: Eq + std::hash::Hash + Clone + Send + Sync + 'static {}

impl<T> AttributionKey for T where T: Eq + std::hash::Hash + Clone + Send + Sync + 'static {}
