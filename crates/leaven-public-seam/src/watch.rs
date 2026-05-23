use crate::{PlanDocument, PublicSeamError};

/// Validated V1 watch replacement route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeferredWatchReplacement {
    plan: PlanDocument,
}

impl DeferredWatchReplacement {
    pub(crate) fn from_plan(plan: PlanDocument) -> Result<Self, PublicSeamError> {
        if !plan.is_since_revision_event_diff() {
            return Err(PublicSeamError::InvalidWatch {
                message: "deferred watch replacement must use a since_revision event diff plan"
                    .to_owned(),
            });
        }
        Ok(Self { plan })
    }

    /// Plan IR document that replaces V1 watch runtime behavior.
    pub fn plan(&self) -> &PlanDocument {
        &self.plan
    }
}
