//! Firkin-backed Leaven workspaces.

mod factory;
mod placement;
mod runtime;

#[cfg(feature = "firkin-facade")]
mod adapter;

#[cfg(feature = "firkin-facade")]
pub use adapter::FirkinRuntimeAdapterRuntime;
pub use factory::FirkinWorkspaceFactory;
pub use placement::{
    FirkinContainerId, FirkinGuestPath, FirkinImageRef, FirkinProductPodId, FirkinWorkspaceContext,
};
pub use runtime::{
    FirkinCommandRequest, FirkinCommandResult, FirkinRuntimeError, FirkinWorkspaceAllocation,
    FirkinWorkspaceRuntime,
};
