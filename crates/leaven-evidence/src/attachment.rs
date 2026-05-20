use leaven_kernel::TraceRef;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// Namespaced attachment name, such as `session/main` or `skill_events`.
    pub name: String,
    pub kind: AttachmentKind,
    pub media_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AttachmentKind {
    /// Durable transcript reference. The workspace runner resolves it when
    /// materializing reflection context; the in-memory value stays compact.
    Transcript(TraceRef),
    /// Structured evidence with an artifact-defined schema.
    Json(serde_json::Value),
    /// Plaintext evidence.
    Text(String),
    /// Durable file reference in the trace store.
    File { ref_: TraceRef },
}
