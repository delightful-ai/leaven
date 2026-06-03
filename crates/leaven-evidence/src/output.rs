//! Reportable output records with visibility and data-class facts.

use std::collections::BTreeSet;

use leaven_kernel::BlobRef;
use serde::{Deserialize, Serialize};

/// Public-seam data class carried by output or evidence.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DataClass(String);

impl DataClass {
    /// Builds a validated data-class label.
    ///
    /// Known labels match the locked public-seam vocabulary. Extension labels
    /// must use the `x.` namespace and contain only stable identifier
    /// characters.
    pub fn new(label: impl Into<String>) -> Result<Self, DataClassError> {
        let label = label.into();
        if is_known_data_class(&label) || is_extension_data_class(&label) {
            Ok(Self(label))
        } else {
            Err(DataClassError { label })
        }
    }

    /// Public data that can appear in ordinary reports.
    #[must_use]
    pub fn public() -> Self {
        Self("public".to_owned())
    }

    /// Candidate output data.
    #[must_use]
    pub fn candidate_output() -> Self {
        Self("candidate.output".to_owned())
    }

    /// Candidate artifact data.
    #[must_use]
    pub fn candidate_artifact() -> Self {
        Self("candidate.artifact".to_owned())
    }

    /// Case target data.
    #[must_use]
    pub fn case_target() -> Self {
        Self("case.target".to_owned())
    }

    /// Raw transcript data.
    #[must_use]
    pub fn transcript_raw() -> Self {
        Self("transcript.raw".to_owned())
    }

    /// Returns the wire label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Invalid public-seam data-class label.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid data class `{label}`")]
pub struct DataClassError {
    label: String,
}

/// Unique set of data classes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DataClassSet(BTreeSet<DataClass>);

impl DataClassSet {
    /// Builds a data-class set from validated labels.
    #[must_use]
    pub fn new(classes: impl IntoIterator<Item = DataClass>) -> Self {
        Self(classes.into_iter().collect())
    }

    /// Public-only data-class set.
    #[must_use]
    pub fn public() -> Self {
        Self::new([DataClass::public()])
    }

    /// Public reportable candidate-output data-class set.
    #[must_use]
    pub fn public_candidate_output() -> Self {
        Self::new([DataClass::candidate_output(), DataClass::public()])
    }

    /// Public reportable candidate-artifact data-class set.
    #[must_use]
    pub fn public_candidate_artifact() -> Self {
        Self::new([DataClass::candidate_artifact(), DataClass::public()])
    }

    /// Returns the classes in stable order.
    pub fn iter(&self) -> impl Iterator<Item = &DataClass> {
        self.0.iter()
    }

    /// Whether this set contains a class.
    #[must_use]
    pub fn contains(&self, class: &DataClass) -> bool {
        self.0.contains(class)
    }
}

impl IntoIterator for DataClassSet {
    type Item = DataClass;
    type IntoIter = std::collections::btree_set::IntoIter<DataClass>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Visibility attached to reportable output.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputVisibility {
    /// Visible in public reports.
    Public,
    /// Visible to optimizer stages.
    OptimizerVisible,
    /// Visible to reflector stages.
    ReflectorVisible,
    /// Visible only to evaluator/scorer code.
    EvaluatorOnly,
    /// Visible only to the operator.
    OperatorOnly,
    /// Private output.
    Private,
    /// Output content was redacted.
    Redacted,
}

/// Visibility and data-class metadata for an output record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OutputMetadata {
    visibility: OutputVisibility,
    data_classes: DataClassSet,
}

impl OutputMetadata {
    /// Public output metadata.
    #[must_use]
    pub fn public() -> Self {
        Self {
            visibility: OutputVisibility::Public,
            data_classes: DataClassSet::public(),
        }
    }

    /// Publicly visible candidate output metadata.
    #[must_use]
    pub fn public_candidate_output() -> Self {
        Self {
            visibility: OutputVisibility::Public,
            data_classes: DataClassSet::public_candidate_output(),
        }
    }

    /// Publicly visible candidate artifact metadata.
    #[must_use]
    pub fn public_candidate_artifact() -> Self {
        Self {
            visibility: OutputVisibility::Public,
            data_classes: DataClassSet::public_candidate_artifact(),
        }
    }

    /// Builds output metadata from visibility and data classes.
    #[must_use]
    pub fn new(visibility: OutputVisibility, data_classes: DataClassSet) -> Self {
        Self {
            visibility,
            data_classes,
        }
    }

    /// Output visibility.
    #[must_use]
    pub const fn visibility(&self) -> OutputVisibility {
        self.visibility
    }

    /// Output data classes.
    #[must_use]
    pub const fn data_classes(&self) -> &DataClassSet {
        &self.data_classes
    }
}

/// Audit metadata for a blob-backed reportable output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OutputBlobAudit {
    sha256: String,
    bytes: u64,
    media_type: Option<String>,
    uri: Option<String>,
}

impl OutputBlobAudit {
    /// Builds validated blob audit metadata.
    pub fn new(sha256: impl Into<String>, bytes: u64) -> Result<Self, OutputBlobAuditError> {
        let sha256 = sha256.into();
        if !is_lower_hex_sha256(&sha256) {
            return Err(OutputBlobAuditError::InvalidSha256 { sha256 });
        }
        Ok(Self {
            sha256,
            bytes,
            media_type: None,
            uri: None,
        })
    }

    /// Adds media-type metadata.
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

    /// Content SHA-256 in lower hex.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Blob byte length.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Optional media type.
    #[must_use]
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    /// Optional public URI.
    #[must_use]
    pub fn uri(&self) -> Option<&str> {
        self.uri.as_deref()
    }
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Invalid blob audit metadata.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OutputBlobAuditError {
    /// SHA-256 was not lower-hex encoded.
    #[error("blob audit sha256 must be 64 lower-hex characters")]
    InvalidSha256 {
        /// Invalid value.
        sha256: String,
    },
}

/// Command or report output carried inline or by external blob reference.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OutputRecord {
    /// Bounded inline output text.
    Inline {
        /// Captured output snippet.
        text: String,
        /// Whether the full output was truncated to this snippet.
        truncated: bool,
        /// Visibility and data-class facts.
        metadata: OutputMetadata,
    },
    /// Output stored outside the graph.
    BlobRef {
        /// Blob reference.
        reference: BlobRef,
        /// Optional audit metadata for public-seam projections.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        audit: Option<OutputBlobAudit>,
        /// Visibility and data-class facts.
        metadata: OutputMetadata,
    },
}

impl OutputRecord {
    /// Builds an untruncated public inline output record.
    #[must_use]
    pub fn inline(text: impl Into<String>) -> Self {
        Self::Inline {
            text: text.into(),
            truncated: false,
            metadata: OutputMetadata::public(),
        }
    }

    /// Builds an untruncated public candidate-output inline record.
    #[must_use]
    pub fn candidate_inline(text: impl Into<String>) -> Self {
        Self::Inline {
            text: text.into(),
            truncated: false,
            metadata: OutputMetadata::public_candidate_output(),
        }
    }

    /// Builds an untruncated public candidate-artifact inline record.
    #[must_use]
    pub fn candidate_artifact_inline(text: impl Into<String>) -> Self {
        Self::Inline {
            text: text.into(),
            truncated: false,
            metadata: OutputMetadata::public_candidate_artifact(),
        }
    }

    /// Builds a truncated public inline output record.
    #[must_use]
    pub fn truncated(text: impl Into<String>) -> Self {
        Self::Inline {
            text: text.into(),
            truncated: true,
            metadata: OutputMetadata::public(),
        }
    }

    /// Builds a public blob-backed output record.
    #[must_use]
    pub fn blob(reference: BlobRef) -> Self {
        Self::BlobRef {
            reference,
            audit: None,
            metadata: OutputMetadata::public(),
        }
    }

    /// Builds a public blob-backed output record with audit metadata.
    #[must_use]
    pub fn audited_blob(reference: BlobRef, audit: OutputBlobAudit) -> Self {
        Self::BlobRef {
            reference,
            audit: Some(audit),
            metadata: OutputMetadata::public(),
        }
    }

    /// Replaces the output metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: OutputMetadata) -> Self {
        match &mut self {
            Self::Inline {
                metadata: current, ..
            }
            | Self::BlobRef {
                metadata: current, ..
            } => *current = metadata,
        }
        self
    }

    /// Output metadata.
    #[must_use]
    pub const fn metadata(&self) -> &OutputMetadata {
        match self {
            Self::Inline { metadata, .. } | Self::BlobRef { metadata, .. } => metadata,
        }
    }

    /// Blob audit metadata, when the output is blob-backed and auditable.
    #[must_use]
    pub const fn blob_audit(&self) -> Option<&OutputBlobAudit> {
        match self {
            Self::BlobRef { audit, .. } => audit.as_ref(),
            Self::Inline { .. } => None,
        }
    }

    /// Compact report text for inline output or a stable blob reference label.
    #[must_use]
    pub fn report_text(&self) -> String {
        match self {
            Self::Inline { text, .. } => text.clone(),
            Self::BlobRef { reference, .. } => {
                format!("blob:{}:{}", reference.store, reference.key)
            }
        }
    }

    /// Output visibility.
    #[must_use]
    pub const fn visibility(&self) -> OutputVisibility {
        self.metadata().visibility()
    }

    /// Output data classes.
    #[must_use]
    pub const fn data_classes(&self) -> &DataClassSet {
        self.metadata().data_classes()
    }
}

fn is_known_data_class(label: &str) -> bool {
    matches!(
        label,
        "public"
            | "case.input"
            | "case.target"
            | "case.metadata"
            | "candidate.output"
            | "candidate.artifact"
            | "workspace.file"
            | "workspace.secret"
            | "scorer.private"
            | "evaluator.private"
            | "optimizer.visible"
            | "prompt.raw"
            | "completion.raw"
            | "transcript.raw"
            | "external.secret"
    )
}

fn is_extension_data_class(label: &str) -> bool {
    let Some(rest) = label.strip_prefix("x.") else {
        return false;
    };
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
}
