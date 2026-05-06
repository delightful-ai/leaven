//! Workspace policies.

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum FilesystemPolicy {
    ReadOnly,
    #[default]
    WritableScratch,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum NetworkPolicy {
    Deny,
    #[default]
    Allow,
}
