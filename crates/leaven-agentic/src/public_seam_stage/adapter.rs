use serde_json::Value;

use super::identity::PublicStagePayloadError;
use super::util::schema_bound_payload;

/// Public-seam callback payload with a schema-bound event body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallbackRequestPayload {
    value: Value,
}

impl CallbackRequestPayload {
    /// Builds a callback request payload.
    pub fn new(
        run: impl Into<String>,
        stage_call_id: impl Into<String>,
        event: Value,
        payload_schema: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicStagePayloadError> {
        Ok(Self {
            value: schema_bound_payload(
                "callback",
                run,
                stage_call_id,
                "event",
                event,
                payload_schema,
                capability_fingerprint,
            )?,
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// Public-seam adapter role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterPayloadRole {
    /// Artifact adapter payload.
    Artifact,
    /// Dataset adapter payload.
    Dataset,
}

impl AdapterPayloadRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Artifact => "artifact_adapter",
            Self::Dataset => "dataset_adapter",
        }
    }
}

/// Public-seam artifact or dataset adapter payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterRequestPayload {
    value: Value,
}

impl AdapterRequestPayload {
    /// Builds an artifact adapter request payload.
    pub fn artifact(
        run: impl Into<String>,
        stage_call_id: impl Into<String>,
        payload: Value,
        payload_schema: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicStagePayloadError> {
        Self::new(
            AdapterPayloadRole::Artifact,
            run,
            stage_call_id,
            payload,
            payload_schema,
            capability_fingerprint,
        )
    }

    /// Builds a dataset adapter request payload.
    pub fn dataset(
        run: impl Into<String>,
        stage_call_id: impl Into<String>,
        payload: Value,
        payload_schema: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicStagePayloadError> {
        Self::new(
            AdapterPayloadRole::Dataset,
            run,
            stage_call_id,
            payload,
            payload_schema,
            capability_fingerprint,
        )
    }

    fn new(
        role: AdapterPayloadRole,
        run: impl Into<String>,
        stage_call_id: impl Into<String>,
        payload: Value,
        payload_schema: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicStagePayloadError> {
        Ok(Self {
            value: schema_bound_payload(
                role.as_str(),
                run,
                stage_call_id,
                "payload",
                payload,
                payload_schema,
                capability_fingerprint,
            )?,
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}
