//! Path-based surface vocabulary.

use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PathPartId(pub PathBuf);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PathAddress(pub PathBuf);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PathSurfaceConfig {
    pub include_hidden: bool,
}
