use crate::OptimizeError;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct BuilderOrder {
    violation: Option<Violation>,
}

#[derive(Clone, Copy, Debug)]
struct Violation {
    operation: &'static str,
    after: &'static str,
}

impl BuilderOrder {
    pub(super) fn runner_after_score(self, scored: bool) -> Self {
        if scored {
            self.with_violation("runner", "score")
        } else {
            self
        }
    }

    pub(super) fn check(self) -> Result<(), OptimizeError> {
        match self.violation {
            Some(Violation { operation, after }) => {
                Err(OptimizeError::InvalidBuilderOrder { operation, after })
            }
            None => Ok(()),
        }
    }

    fn with_violation(self, operation: &'static str, after: &'static str) -> Self {
        Self {
            violation: self.violation.or(Some(Violation { operation, after })),
        }
    }
}
