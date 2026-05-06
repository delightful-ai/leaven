//! Preference result.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Preference {
    LeftBetter,
    RightBetter,
    Equivalent,
    Incomparable,
}
