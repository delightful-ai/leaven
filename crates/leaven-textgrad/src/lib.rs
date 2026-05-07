//! leaven-textgrad crate skeleton.

mod feedback;
mod textgrad;
mod updater;

pub use feedback::{FeedbackAggregator, PerPartFeedbackAggregator};
pub use textgrad::{TextGrad, TextGradBuilder, TextGradConfig};
pub use updater::TextGradientUpdater;
