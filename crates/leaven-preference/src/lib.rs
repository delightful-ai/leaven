//! Stateless preference relations over reusable evidence values.

mod scalar;

pub use scalar::{HigherScoreIsBetter, LowerScoreIsBetter};

pub mod prelude {
    pub use crate::{HigherScoreIsBetter, LowerScoreIsBetter};
}
