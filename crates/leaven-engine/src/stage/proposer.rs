//! Proposer stage trait.

use std::any::{Any, type_name};

use futures::future::LocalBoxFuture;
use leaven_core::{OptimizationProblem, ProposalBatch};
use leaven_kernel::{Metered, ProposerId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
/// Number of candidates a proposer expects to read for one proposal request.
pub enum Arity {
    /// The proposer creates content without candidate input.
    None,
    /// The proposer mutates or reasons from one candidate.
    Single,
    /// The proposer compares or combines two candidates.
    Pair,
    /// The proposer accepts a variable-size candidate set.
    Variadic,
}

/// Static proposer capability.
///
/// Static proposers keep their request type in the type system. Runtime plugin
/// dispatch should go through [`DynProposer`], which checks the erased request
/// type before calling this trait.
#[allow(async_fn_in_trait)]
pub trait Proposer<P: OptimizationProblem>: Send + Sync {
    /// Request shape this proposer accepts.
    type Request: Send + Sync;

    /// Stable proposer identity for events, budget accounting, and errors.
    fn id(&self) -> ProposerId;

    /// Candidate arity this proposer expects.
    fn arity(&self) -> Arity {
        Arity::Single
    }

    /// Produce a metered batch of sibling proposals.
    async fn propose(
        &self,
        request: Self::Request,
        ctx: crate::ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError>;
}

/// Object-safe proposer capability for runtime-loaded or heterogeneous stages.
pub trait DynProposer<P: OptimizationProblem>: Send + Sync {
    /// Stable proposer identity for events, budget accounting, and errors.
    fn id(&self) -> ProposerId;
    /// Candidate arity this proposer expects.
    fn arity(&self) -> Arity;
    /// Downcast and dispatch an erased request into the static proposer.
    fn propose_boxed<'a>(
        &'a self,
        request: Box<dyn std::any::Any + Send>,
        ctx: crate::ProposalContext<'a, P>,
    ) -> LocalBoxFuture<'a, Result<Metered<ProposalBatch<P>>, ProposalError>>;
}

impl<P, T> DynProposer<P> for T
where
    P: OptimizationProblem,
    T: Proposer<P>,
    T::Request: 'static,
{
    fn id(&self) -> ProposerId {
        Proposer::id(self)
    }

    fn arity(&self) -> Arity {
        Proposer::arity(self)
    }

    fn propose_boxed<'a>(
        &'a self,
        request: Box<dyn Any + Send>,
        ctx: crate::ProposalContext<'a, P>,
    ) -> LocalBoxFuture<'a, Result<Metered<ProposalBatch<P>>, ProposalError>> {
        let Ok(request) = request.downcast::<T::Request>() else {
            let error = ProposalError::RequestTypeMismatch {
                proposer: Proposer::id(self),
                expected: type_name::<T::Request>(),
            };
            return Box::pin(async move { Err(error) });
        };
        Box::pin(self.propose(*request, ctx))
    }
}

/// Failures a proposer can report at the capability boundary.
#[derive(Debug, thiserror::Error)]
pub enum ProposalError {
    /// Generic proposer failure when no better public variant applies.
    #[error("proposal failed: {0}")]
    Message(String),
    /// A dynamic proposer call supplied the wrong erased request type.
    #[error("proposal `{proposer}` expected request type `{expected}`")]
    RequestTypeMismatch {
        proposer: ProposerId,
        expected: &'static str,
    },
    /// Proposer failure with a structured source error.
    #[error("proposal failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl ProposalError {
    /// Build a proposer error while preserving a source error chain.
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::WithSource {
            message: message.into(),
            source: Box::new(source),
        }
    }
}
