//! Bridge errors over the ACP transport and the public-seam wire validators.

use leaven_acp::AcpTransportError;
use leaven_public_seam::PublicSeamError;

/// Failure raised while dispatching a stage, servicing a host effect, or running
/// the tiny accept loop.
#[derive(Debug, thiserror::Error)]
pub enum StageBridgeError {
    /// The ACP stdio transport failed (spawn, framing, demux, or cancellation).
    #[error(transparent)]
    Transport(#[from] AcpTransportError),
    /// A wire document failed locked public-seam validation.
    #[error(transparent)]
    PublicSeam(#[from] PublicSeamError),
    /// The worker returned a stage output the bridge could not interpret.
    #[error("stage-run output is not interpretable: {message}")]
    Output { message: String },
    /// The optimizer ran with an empty case set or could not evaluate a candidate.
    #[error("optimizer cannot proceed: {message}")]
    Optimizer { message: String },
}

impl StageBridgeError {
    pub(crate) fn output(message: impl Into<String>) -> Self {
        Self::Output {
            message: message.into(),
        }
    }

    pub(crate) fn optimizer(message: impl Into<String>) -> Self {
        Self::Optimizer {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_and_optimizer_errors_render_their_message() {
        assert_eq!(
            StageBridgeError::output("bad output").to_string(),
            "stage-run output is not interpretable: bad output"
        );
        assert_eq!(
            StageBridgeError::optimizer("no cases").to_string(),
            "optimizer cannot proceed: no cases"
        );
    }

    #[test]
    fn public_seam_errors_convert_transparently() {
        let seam = PublicSeamError::InvalidScope {
            message: "scope".to_owned(),
        };
        let bridge: StageBridgeError = seam.into();
        assert!(matches!(bridge, StageBridgeError::PublicSeam(_)));
        assert!(bridge.to_string().contains("scope"));
    }
}
