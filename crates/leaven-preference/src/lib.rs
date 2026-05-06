//! leaven-preference crate skeleton.

mod scalar;

pub struct BordaPreference;
pub struct CopelandPreference;
pub struct LexicographicPreference;
pub struct ParetoPreference;
pub use scalar::{HigherScoreIsBetter, LowerScoreIsBetter};
pub mod prelude {
    pub use crate::{
        BordaPreference, CopelandPreference, HigherScoreIsBetter, LexicographicPreference,
        LowerScoreIsBetter, ParetoPreference,
    };
}
