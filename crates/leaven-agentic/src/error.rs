//! Agentic adapter errors.

use leaven_agent::AgentRuntimeError;
use leaven_engine::{MaterializeError, RenderError};
use leaven_kernel::{Amount, Cost};
use leaven_workspace::{FactoryError, WithWorkspaceError, WorkspaceError};

#[derive(Debug, thiserror::Error)]
pub enum AgenticAdapterError {
    #[error(transparent)]
    WorkspaceAllocate(#[from] FactoryError),
    #[error(transparent)]
    WorkspaceCleanup(WorkspaceError),
    #[error("agentic stage failed and workspace cleanup also failed")]
    StageAndCleanup {
        #[source]
        stage: Box<Self>,
        cleanup: WorkspaceError,
    },
    #[error(transparent)]
    Materialize(#[from] MaterializeError),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error(transparent)]
    Runtime(#[from] AgentRuntimeError),
    #[error(transparent)]
    Parse(#[from] AgenticParseError),
    #[error("agentic input build failed: {0}")]
    Input(String),
    #[error("agentic cost overflow while adding {axis}")]
    CostOverflow { axis: &'static str },
}

#[derive(Debug, thiserror::Error)]
pub enum AgenticParseError {
    #[error("agentic parse failed: {0}")]
    Message(String),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error("agentic parse failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl AgenticParseError {
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

pub fn map_workspace_error(error: WithWorkspaceError<AgenticAdapterError>) -> AgenticAdapterError {
    match error {
        WithWorkspaceError::Allocate(error) => AgenticAdapterError::WorkspaceAllocate(error),
        WithWorkspaceError::Stage(error) => error,
        WithWorkspaceError::Cleanup(error) => AgenticAdapterError::WorkspaceCleanup(error),
        WithWorkspaceError::StageAndCleanup { stage, cleanup } => {
            AgenticAdapterError::StageAndCleanup {
                stage: Box::new(stage),
                cleanup,
            }
        }
    }
}

pub fn checked_add_cost(mut total: Cost, next: &Cost) -> Result<Cost, AgenticAdapterError> {
    total.metric_calls = total.metric_calls.checked_add(next.metric_calls).ok_or(
        AgenticAdapterError::CostOverflow {
            axis: "metric_calls",
        },
    )?;
    total.llm_calls = total
        .llm_calls
        .checked_add(next.llm_calls)
        .ok_or(AgenticAdapterError::CostOverflow { axis: "llm_calls" })?;
    total.prompt_tokens = total.prompt_tokens.checked_add(next.prompt_tokens).ok_or(
        AgenticAdapterError::CostOverflow {
            axis: "prompt_tokens",
        },
    )?;
    total.completion_tokens = total
        .completion_tokens
        .checked_add(next.completion_tokens)
        .ok_or(AgenticAdapterError::CostOverflow {
            axis: "completion_tokens",
        })?;
    total.seconds = Amount::new(total.seconds.as_f64() + next.seconds.as_f64())
        .map_err(|_err| AgenticAdapterError::CostOverflow { axis: "seconds" })?;
    for (axis, amount) in &next.other {
        let current = total.other.entry(axis.clone()).or_insert_with(Amount::zero);
        *current = Amount::new(current.as_f64() + amount.as_f64()).map_err(|_err| {
            AgenticAdapterError::CostOverflow {
                axis: "custom axis",
            }
        })?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use leaven_kernel::Amount;

    use super::*;

    #[test]
    fn workspace_error_mapping_preserves_all_variants() {
        assert!(matches!(
            map_workspace_error(WithWorkspaceError::Allocate(FactoryError::Allocate(
                "no slot".to_owned()
            ))),
            AgenticAdapterError::WorkspaceAllocate(_)
        ));
        assert!(matches!(
            map_workspace_error(WithWorkspaceError::Stage(AgenticAdapterError::Input(
                "bad request".to_owned()
            ))),
            AgenticAdapterError::Input(message) if message == "bad request"
        ));
    }

    #[test]
    fn checked_cost_adds_custom_axes_and_rejects_each_overflow_family() {
        let mut total = Cost::metric_calls(u64::MAX);
        let mut next = Cost::metric_calls(1);
        assert!(matches!(
            checked_add_cost(total.clone(), &next),
            Err(AgenticAdapterError::CostOverflow {
                axis: "metric_calls"
            })
        ));

        total = Cost {
            prompt_tokens: u64::MAX,
            ..Cost::zero()
        };
        next = Cost::tokens(1, 0);
        assert!(matches!(
            checked_add_cost(total.clone(), &next),
            Err(AgenticAdapterError::CostOverflow {
                axis: "prompt_tokens"
            })
        ));

        total = Cost {
            completion_tokens: u64::MAX,
            ..Cost::zero()
        };
        next = Cost::tokens(0, 1);
        assert!(matches!(
            checked_add_cost(total.clone(), &next),
            Err(AgenticAdapterError::CostOverflow {
                axis: "completion_tokens"
            })
        ));

        total = Cost {
            seconds: Amount::new(f64::MAX).unwrap(),
            ..Cost::zero()
        };
        next = Cost {
            seconds: Amount::new(f64::MAX).unwrap(),
            ..Cost::zero()
        };
        assert!(matches!(
            checked_add_cost(total.clone(), &next),
            Err(AgenticAdapterError::CostOverflow { axis: "seconds" })
        ));

        total = Cost::zero();
        total
            .other
            .insert("gpu_seconds".to_owned(), Amount::new(1.5).unwrap());
        next = Cost::zero();
        next.other
            .insert("gpu_seconds".to_owned(), Amount::new(2.5).unwrap());
        let combined = checked_add_cost(total.clone(), &next).unwrap();
        assert_eq!(combined.other["gpu_seconds"], Amount::new(4.0).unwrap());

        total
            .other
            .insert("gpu_seconds".to_owned(), Amount::new(f64::MAX).unwrap());
        next.other
            .insert("gpu_seconds".to_owned(), Amount::new(f64::MAX).unwrap());
        assert!(matches!(
            checked_add_cost(total, &next),
            Err(AgenticAdapterError::CostOverflow {
                axis: "custom axis"
            })
        ));
    }
}
