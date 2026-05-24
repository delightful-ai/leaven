//! Public-seam wire lowering for reusable output records.

use leaven_evidence::{DataClassSet, OutputBlobAudit, OutputRecord, OutputVisibility};
use leaven_kernel::BlobRef;
use serde_json::{Value, json};

use crate::PublicSeamError;

/// Public-seam blob identity and audit metadata for blob-backed outputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicBlobRef {
    id: String,
    sha256: String,
    bytes: u64,
    media_type: Option<String>,
    uri: Option<String>,
    data_classes: DataClassSet,
}

impl PublicBlobRef {
    /// Builds public-seam blob metadata.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        sha256: impl Into<String>,
        bytes: u64,
        data_classes: DataClassSet,
    ) -> Self {
        Self {
            id: id.into(),
            sha256: sha256.into(),
            bytes,
            media_type: None,
            uri: None,
            data_classes,
        }
    }

    /// Adds media type metadata.
    #[must_use]
    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    /// Adds a public URI for this blob.
    #[must_use]
    pub fn with_uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    fn as_value(&self) -> Value {
        let mut value = json!({
            "kind": "blob_ref",
            "id": self.id,
            "sha256": self.sha256,
            "bytes": self.bytes,
            "data_classes": data_classes_wire(&self.data_classes)
        });
        let object = value.as_object_mut().expect("blob ref JSON is object");
        if let Some(media_type) = &self.media_type {
            object.insert("media_type".to_owned(), json!(media_type));
        }
        if let Some(uri) = &self.uri {
            object.insert("uri".to_owned(), json!(uri));
        }
        value
    }

    fn from_evidence_blob(
        reference: &BlobRef,
        audit: &OutputBlobAudit,
        data_classes: &DataClassSet,
    ) -> Self {
        let mut blob = Self::new(
            public_blob_id(reference),
            audit.sha256(),
            audit.bytes(),
            data_classes.clone(),
        );
        if let Some(media_type) = audit.media_type() {
            blob = blob.with_media_type(media_type);
        }
        if let Some(uri) = audit.uri() {
            blob = blob.with_uri(uri);
        }
        blob
    }
}

fn public_blob_id(reference: &BlobRef) -> String {
    let digest = jcs_canonicalize::sha256_jcs_hex(&json!({
        "store": reference.store,
        "key": reference.key
    }))
    .expect("blob reference JSON is canonicalizable");
    format!("blob_{digest}")
}

/// Schema-valid public-seam `OutputRecord` wire projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicOutputRecord {
    value: Value,
    kind: String,
    visibility: String,
    data_classes: Vec<String>,
}

/// Document-oriented name for validated public-seam output records.
pub type OutputRecordDocument = PublicOutputRecord;

impl PublicOutputRecord {
    pub(crate) fn from_schema_valid_value(value: Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| PublicSeamError::InvalidOutputRecord {
                message: "output record must be an object".to_owned(),
            })?;
        Ok(Self {
            kind: required_string(object.get("kind"), "kind")?.to_owned(),
            visibility: required_string(object.get("visibility"), "visibility")?.to_owned(),
            data_classes: required_string_array(object.get("data_classes"), "data_classes")?,
            value,
        })
    }

    /// Original public-seam wire value.
    pub const fn as_value(&self) -> &Value {
        &self.value
    }

    /// Output kind.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Output visibility class.
    pub fn visibility(&self) -> &str {
        &self.visibility
    }

    /// Output data classes.
    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
    }
}

pub fn output_record_wire_value(
    record: &OutputRecord,
    blob: Option<&PublicBlobRef>,
) -> Result<Value, PublicSeamError> {
    let metadata = record.metadata();
    match record {
        OutputRecord::Inline { text, .. } => {
            if text.trim().is_empty() {
                return Err(PublicSeamError::InvalidOutputRecord {
                    message: "inline output record must not be an empty placeholder".to_owned(),
                });
            }
            Ok(json!({
                "kind": "text",
                "summary": text,
                "value": text,
                "visibility": visibility_wire(metadata.visibility()),
                "data_classes": data_classes_wire(metadata.data_classes())
            }))
        }
        OutputRecord::BlobRef {
            reference, audit, ..
        } => {
            let projected_blob;
            let blob = if let Some(blob) = blob {
                blob
            } else if let Some(audit) = audit {
                projected_blob =
                    PublicBlobRef::from_evidence_blob(reference, audit, metadata.data_classes());
                &projected_blob
            } else {
                return Err(PublicSeamError::InvalidOutputRecord {
                    message: "blob output record requires public blob metadata".to_owned(),
                });
            };
            Ok(json!({
                "kind": "blob_ref",
                "blob_ref": blob.as_value(),
                "visibility": visibility_wire(metadata.visibility()),
                "data_classes": data_classes_wire(metadata.data_classes())
            }))
        }
    }
}

fn visibility_wire(visibility: OutputVisibility) -> &'static str {
    match visibility {
        OutputVisibility::Public => "public",
        OutputVisibility::OptimizerVisible => "optimizer_visible",
        OutputVisibility::ReflectorVisible => "reflector_visible",
        OutputVisibility::EvaluatorOnly => "evaluator_only",
        OutputVisibility::OperatorOnly => "operator_only",
        OutputVisibility::Private => "private",
        OutputVisibility::Redacted => "redacted",
    }
}

fn data_classes_wire(data_classes: &DataClassSet) -> Vec<&str> {
    data_classes
        .iter()
        .map(leaven_evidence::DataClass::as_str)
        .collect()
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| PublicSeamError::InvalidOutputRecord {
            message: format!("output record {field} must be a string"),
        })
}

fn required_string_array(
    value: Option<&Value>,
    field: &str,
) -> Result<Vec<String>, PublicSeamError> {
    let values =
        value
            .and_then(Value::as_array)
            .ok_or_else(|| PublicSeamError::InvalidOutputRecord {
                message: format!("output record {field} must be an array"),
            })?;
    values
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                PublicSeamError::InvalidOutputRecord {
                    message: format!("output record {field} entries must be strings"),
                }
            })
        })
        .collect()
}
