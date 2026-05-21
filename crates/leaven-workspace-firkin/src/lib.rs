//! Firkin-backed Leaven workspaces.

mod factory;
mod placement;
mod runtime;

pub use factory::FirkinWorkspaceFactory;
pub use placement::{
    FirkinContainerId, FirkinGuestPath, FirkinImageRef, FirkinProductPodId, FirkinWorkspaceContext,
};
pub use runtime::{
    FirkinCommandRequest, FirkinCommandResult, FirkinRuntimeError, FirkinWorkspaceAllocation,
    FirkinWorkspaceRuntime,
};
