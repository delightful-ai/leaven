//! Finite floating-point primitive.
//!
//! Use this when a public API needs an arbitrary signed floating-point value
//! but must still reject `NaN` and infinity. Domain-specific non-negative
//! quantities should use [`Amount`](crate::Amount) instead.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

/// Error returned when constructing a [`FiniteF64`] from a non-finite value.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum FiniteF64Error {
    /// Value was NaN or infinite.
    #[error("value must be finite, got {value}")]
    NonFinite {
        /// Rejected value.
        value: f64,
    },
}

/// Signed `f64` that cannot be NaN or infinite.
///
/// `FiniteF64` is deliberately weaker than [`Amount`](crate::Amount): it
/// preserves negative values because some domains use signed weights, deltas, or
/// scores. It only enforces the invariant that downstream comparisons,
/// serialization, and arithmetic are not poisoned by `NaN` or infinity.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct FiniteF64(f64);

impl FiniteF64 {
    /// Zero.
    pub const ZERO: Self = Self(0.0);

    /// Constructs a finite value.
    ///
    /// # Errors
    ///
    /// Returns [`FiniteF64Error::NonFinite`] when `value` is NaN or infinite.
    pub fn new(value: f64) -> Result<Self, FiniteF64Error> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(FiniteF64Error::NonFinite { value })
        }
    }

    /// Returns the wrapped value.
    #[must_use]
    pub const fn as_f64(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FiniteF64 {
    type Error = FiniteF64Error;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FiniteF64> for f64 {
    fn from(value: FiniteF64) -> Self {
        value.0
    }
}

impl Serialize for FiniteF64 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for FiniteF64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}
