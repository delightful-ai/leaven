use serde::Deserialize;

/// Parsed public-seam conformance matrix.
#[derive(Clone, Debug, Deserialize)]
pub struct ConformanceMatrix {
    /// Matrix name from the YAML package.
    pub matrix_name: String,
    /// Matrix status.
    pub status: String,
    /// Conformance rows.
    pub rows: Vec<ConformanceRow>,
}

impl ConformanceMatrix {
    /// Rows whose status is proven.
    pub fn proven_rows(&self) -> Vec<&ConformanceRow> {
        self.rows
            .iter()
            .filter(|row| row.status == MatrixRowStatus::Proven)
            .collect()
    }
}

/// One public-seam conformance obligation.
#[derive(Clone, Debug, Deserialize)]
pub struct ConformanceRow {
    /// Stable row id.
    pub id: String,
    /// Row area.
    pub area: String,
    /// Human-readable requirement text.
    pub requirement: String,
    /// Spec paths referenced by this row.
    pub spec_refs: Vec<String>,
    /// Active conformance-test denominator ids assigned to this row.
    #[serde(default)]
    pub conformance_tests: Vec<String>,
    /// Positive executable test evidence for proven rows.
    #[serde(default)]
    pub positive_test_evidence: Vec<String>,
    /// Negative executable test evidence for proven semantic-denial rows.
    #[serde(default)]
    pub negative_test_evidence: Vec<String>,
    /// Implementation evidence paths for proven rows.
    #[serde(default)]
    pub implementation_evidence: Vec<String>,
    /// Review evidence paths for proven rows.
    #[serde(default)]
    pub review_evidence: Vec<String>,
    /// Minimum closeout level required by the row.
    pub minimum_closeout_level: MinimumCloseoutLevel,
    /// The fake pass this row explicitly rejects.
    pub fake_pass_rejected: String,
    /// Current row status.
    pub status: MatrixRowStatus,
}

/// Minimum proof level required to close a row.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MinimumCloseoutLevel {
    /// Schemas, examples, or generated types are sufficient only for explicitly shape-only rows.
    ShapeOnly,
    /// Public shape and round-trip vocabulary exist.
    StructuralContract,
    /// Forbidden behavior is rejected with typed evidence.
    SemanticDenial,
    /// The intended user-facing flow exercises the seam through the owning route.
    IntegratedSurface,
}

impl MinimumCloseoutLevel {
    /// Returns true when a proven row must carry positive and negative executable proof.
    pub fn requires_denial_evidence(self) -> bool {
        matches!(self, Self::SemanticDenial | Self::IntegratedSurface)
    }
}

/// Allowed conformance row statuses.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MatrixRowStatus {
    /// No implementation proof yet.
    Pending,
    /// Implementation or verification is underway.
    InProgress,
    /// Evidence exists and is linked from the row.
    Proven,
    /// Cannot be proven without a missing decision or dependency.
    Blocked,
    /// Removed from V1 scope by spec or manifest revision.
    Dropped,
}
