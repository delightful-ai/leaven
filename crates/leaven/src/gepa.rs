//! GEPA optimizer facade.
//!
//! This module keeps GEPA-specific imports under `leaven::gepa` while the
//! ordinary [`crate::prelude`] remains optimizer-agnostic.

pub use leaven_gepa::*;

/// Extension methods for reading GEPA-specific reports from an optimized run.
///
/// Import this trait when a run was executed with [`Gepa`] and you want the
/// typed GEPA detail report:
///
/// ```text
/// use leaven::gepa::GepaOptimizedExt as _;
///
/// let report = optimized.gepa_report();
/// ```
pub trait GepaOptimizedExt {
    /// Return the typed GEPA report when the configured optimizer produced one.
    #[must_use]
    fn gepa_report(&self) -> Option<&GepaReport>;
}

impl<A> GepaOptimizedExt for leaven_run::Optimized<A> {
    fn gepa_report(&self) -> Option<&GepaReport> {
        self.optimizer_report::<GepaReport>()
    }
}
