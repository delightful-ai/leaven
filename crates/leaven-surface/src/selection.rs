//! Part selections.

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum PartSelection<Id = crate::PartAddress> {
    All,
    Only(Vec<Id>),
}
