use std::io::{BufRead, Write};
use std::path::Path;

use leaven_core::OptimizationProblem;
use leaven_public_seam::{PublicSeamError, PublicSeamPackage};
use leaven_seam_runtime::{SeamRuntime, SeamRuntimeError, SeamService};
use leaven_seam_service::RunBoundGraphEffectService;
use leaven_seam_stdio::{SeamStdioError, StdioServeReport, serve_reader_writer};

/// Launchable run-bound route over the locked public seam runtime.
///
/// The route owns runtime/package composition only. The supplied service owns
/// method execution and must route graph writes through a live `RunContext`.
pub struct RunBoundSdkRoute<S> {
    runtime: SeamRuntime<S>,
}

impl<S> RunBoundSdkRoute<S> {
    /// Builds a route from an already loaded locked public-seam package.
    pub fn from_package(
        package: PublicSeamPackage,
        service: S,
    ) -> Result<Self, RunBoundRouteError> {
        Ok(Self {
            runtime: SeamRuntime::from_package(package, service)?,
        })
    }

    /// Loads the locked public-seam package from a repository root.
    pub fn from_repo(root: impl AsRef<Path>, service: S) -> Result<Self, RunBoundRouteError> {
        let package = PublicSeamPackage::active_from_repo(root)?;
        Self::from_package(package, service)
    }

    /// Returns the locked method names exposed by this route.
    pub fn methods(&self) -> impl Iterator<Item = &str> {
        self.runtime.methods()
    }
}

impl<S> RunBoundSdkRoute<S>
where
    S: SeamService,
{
    /// Serves one line-delimited JSON-RPC stream against the bound run service.
    pub fn serve_reader_writer<R, W>(
        &self,
        reader: R,
        writer: W,
    ) -> Result<StdioServeReport, RunBoundRouteError>
    where
        R: BufRead,
        W: Write,
    {
        Ok(serve_reader_writer(&self.runtime, reader, writer)?)
    }
}

impl<'service, 'run, P> RunBoundSdkRoute<RunBoundGraphEffectService<'service, 'run, P>>
where
    P: OptimizationProblem,
{
    /// Builds a route around a service bound to a live run context.
    pub fn bind_run_bound_service(
        root: impl AsRef<Path>,
        service: RunBoundGraphEffectService<'service, 'run, P>,
    ) -> Result<Self, RunBoundRouteError> {
        Self::from_repo(root, service)
    }
}

/// Failure while constructing or serving the run-bound SDK route.
#[derive(Debug, thiserror::Error)]
pub enum RunBoundRouteError {
    /// The locked public-seam package failed to load or validate.
    #[error(transparent)]
    PublicSeam(#[from] PublicSeamError),
    /// The transport-neutral runtime failed to construct.
    #[error(transparent)]
    Runtime(#[from] SeamRuntimeError),
    /// The stdio adapter failed while serving the route.
    #[error(transparent)]
    Stdio(#[from] SeamStdioError),
}
