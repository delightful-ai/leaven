//! leaven-preference crate skeleton.

mod pareto;
mod ranking;
mod scalar;

pub use pareto::ParetoPreference;
pub use ranking::{BordaPreference, CopelandPreference, LexicographicPreference};
pub use scalar::{HigherScoreIsBetter, LowerScoreIsBetter};

pub mod prelude {
    pub use crate::{
        BordaPreference, CopelandPreference, HigherScoreIsBetter, LexicographicPreference,
        LowerScoreIsBetter, ParetoPreference,
    };
}
