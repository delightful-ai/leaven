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
    /// Spec paths referenced by this row.
    pub spec_refs: Vec<String>,
    /// Current row status.
    pub status: MatrixRowStatus,
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
