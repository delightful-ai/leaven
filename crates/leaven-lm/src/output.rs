use serde::{Deserialize, Serialize};

/// Requested output shape.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputMode {
    /// Plain text output.
    #[default]
    Text,
    /// JSON object mode.
    JsonObject,
    /// JSON schema constrained output.
    JsonSchema(JsonSchemaOutput),
}

/// Provider-neutral JSON schema output request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct JsonSchemaOutput {
    /// Schema name.
    pub name: String,
    /// JSON schema body.
    pub schema: serde_json::Value,
    /// Whether provider-side strict schema enforcement is requested.
    pub strict: bool,
}
