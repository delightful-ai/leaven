//! Dataset and case records.

use std::collections::BTreeMap;

use std::collections::BTreeSet;

use leaven_kernel::{CaseId, Fingerprint, FingerprintBuilder, MetadataBag, MetadataValue};

use crate::DatasetError;

/// Marker target for unlabeled cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NoTarget {}

/// One unit of evaluation work.
#[derive(Clone, Debug)]
pub struct Case<I = serde_json::Value, T = NoTarget> {
    /// Stable case id.
    pub id: CaseId,
    /// User/domain input.
    pub input: I,
    /// Optional reference target.
    pub target: Option<T>,
    /// Operational metadata.
    pub metadata: MetadataBag,
}

impl<I, T> Case<I, T> {
    /// Builds a case envelope with an optional scorer-visible target.
    #[must_use]
    pub fn new(id: CaseId, input: I, target: Option<T>) -> Self {
        Self {
            id,
            input,
            target,
            metadata: MetadataBag::new(),
        }
    }

    /// Builds a case envelope with a required scorer-visible target.
    #[must_use]
    pub fn targeted(id: CaseId, input: I, target: T) -> Self {
        Self::new(id, input, Some(target))
    }

    /// Builds a case from an upstream ordered source-row manifest.
    #[must_use]
    pub fn from_source_row(
        row_index: usize,
        source_id: impl Into<String>,
        input: I,
        target: Option<T>,
    ) -> Self {
        let mut metadata = MetadataBag::new();
        metadata.insert(
            "source_row_index",
            MetadataValue::U64(u64::try_from(row_index).expect("usize row index fits in u64")),
        );
        metadata.insert("source_id", MetadataValue::String(source_id.into()));
        Self::new(CaseId::from_index(row_index), input, target).with_metadata(metadata)
    }

    /// Replaces the operational metadata bag.
    #[must_use]
    pub fn with_metadata(mut self, metadata: MetadataBag) -> Self {
        self.metadata = metadata;
        self
    }
}

impl<I> Case<I, NoTarget> {
    /// Builds an input-only case envelope.
    #[must_use]
    pub fn input(id: CaseId, input: I) -> Self {
        Self::new(id, input, None)
    }
}

/// Conventional language-model case alias.
pub type LmCase<I = serde_json::Value, T = serde_json::Value> = Case<I, T>;

/// One trusted row from an upstream ordered source manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRow<I, T = NoTarget> {
    source_id: String,
    input: I,
    target: Option<T>,
}

impl<I, T> SourceRow<I, T> {
    /// Builds a source row with an optional scorer-visible target.
    #[must_use]
    pub fn new(source_id: impl Into<String>, input: I, target: Option<T>) -> Self {
        Self {
            source_id: source_id.into(),
            input,
            target,
        }
    }

    /// Builds a source row with a required scorer-visible target.
    #[must_use]
    pub fn targeted(source_id: impl Into<String>, input: I, target: T) -> Self {
        Self::new(source_id, input, Some(target))
    }

    /// Upstream source-row id.
    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    /// User/domain input.
    #[must_use]
    pub const fn input(&self) -> &I {
        &self.input
    }

    /// Optional scorer-visible target.
    #[must_use]
    pub const fn target(&self) -> Option<&T> {
        self.target.as_ref()
    }
}

impl<I> SourceRow<I, NoTarget> {
    /// Builds an input-only source row.
    #[must_use]
    pub fn input_only(source_id: impl Into<String>, input: I) -> Self {
        Self::new(source_id, input, None)
    }
}

/// Ordered upstream row manifest lowered into stable Leaven case ids.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRowManifest<I, T = NoTarget> {
    rows: Vec<SourceRow<I, T>>,
    fingerprint: Fingerprint,
}

impl<I, T> SourceRowManifest<I, T> {
    /// Builds an ordered source-row manifest and refuses duplicate source ids.
    pub fn new(rows: Vec<SourceRow<I, T>>) -> Result<Self, DatasetError> {
        let mut seen = BTreeSet::new();
        for row in &rows {
            if !seen.insert(row.source_id.clone()) {
                return Err(DatasetError::DuplicateSourceRowId(row.source_id.clone()));
            }
        }
        let fingerprint = source_row_manifest_fingerprint(&rows);
        Ok(Self { rows, fingerprint })
    }

    /// Number of source rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// True when the manifest contains no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Source-row identity fingerprint over row order and upstream ids.
    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// Stable Leaven case ids corresponding to this row order.
    #[must_use]
    pub fn ordered_case_ids(&self) -> Vec<CaseId> {
        (0..self.rows.len()).map(CaseId::from_index).collect()
    }

    /// Source rows in upstream order.
    #[must_use]
    pub fn rows(&self) -> &[SourceRow<I, T>] {
        &self.rows
    }

    /// Consumes the manifest into a Leaven dataset with row-stable case ids.
    pub fn into_dataset(self) -> Result<Dataset<Case<I, T>>, DatasetError> {
        let cases = self
            .rows
            .into_iter()
            .enumerate()
            .map(|(row_index, row)| {
                Case::from_source_row(row_index, row.source_id, row.input, row.target)
            })
            .collect();
        Dataset::from_cases(cases)
    }
}

/// Durable case collection with a membership fingerprint.
#[derive(Clone, Debug)]
pub struct Dataset<C = Case> {
    cases: BTreeMap<CaseId, C>,
    fingerprint: Fingerprint,
    metadata: MetadataBag,
}

impl<C> Dataset<C> {
    /// Starts a dataset builder.
    #[must_use]
    pub fn builder() -> DatasetBuilder<C> {
        DatasetBuilder::default()
    }

    /// Builds a dataset from cases using dense ordered ids.
    pub fn from_ordered(cases: Vec<C>) -> Self {
        Self::builder().ordered_cases(cases).build()
    }

    /// Cases keyed by id.
    #[must_use]
    pub const fn cases(&self) -> &BTreeMap<CaseId, C> {
        &self.cases
    }

    /// Dataset membership fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// Operational metadata.
    #[must_use]
    pub const fn metadata(&self) -> &MetadataBag {
        &self.metadata
    }
}

fn source_row_manifest_fingerprint<I, T>(rows: &[SourceRow<I, T>]) -> Fingerprint {
    let mut fingerprint = FingerprintBuilder::new();
    fingerprint.update(b"leaven-eval:source-row-manifest:v1");
    for (index, row) in rows.iter().enumerate() {
        fingerprint.update(
            u64::try_from(index)
                .expect("usize row index fits in u64")
                .to_le_bytes(),
        );
        fingerprint.update(row.source_id.as_bytes());
        fingerprint.update(b"\0");
    }
    fingerprint.finish()
}

impl<I, T> Dataset<Case<I, T>> {
    /// Builds a dataset from case envelopes while preserving their stable IDs.
    pub fn from_cases(cases: Vec<Case<I, T>>) -> Result<Self, DatasetError> {
        let mut builder = Self::builder();
        for case in cases {
            builder = builder.case(case.id, case)?;
        }
        Ok(builder.build())
    }
}

/// Dataset builder.
pub struct DatasetBuilder<C> {
    cases: BTreeMap<CaseId, C>,
    metadata: MetadataBag,
}

impl<C> Default for DatasetBuilder<C> {
    fn default() -> Self {
        Self {
            cases: BTreeMap::new(),
            metadata: MetadataBag::new(),
        }
    }
}

impl<C> DatasetBuilder<C> {
    /// Replaces the builder cases with dense ordered ids.
    #[must_use]
    pub fn ordered_cases(mut self, cases: Vec<C>) -> Self {
        self.cases = cases
            .into_iter()
            .enumerate()
            .map(|(index, case)| (CaseId::from_index(index), case))
            .collect();
        self
    }

    /// Adds one explicitly id'd case.
    pub fn case(mut self, id: CaseId, case: C) -> Result<Self, DatasetError> {
        if self.cases.insert(id, case).is_some() {
            return Err(DatasetError::DuplicateCase(id));
        }
        Ok(self)
    }

    /// Builds the dataset.
    #[must_use]
    pub fn build(self) -> Dataset<C> {
        let mut fingerprint = FingerprintBuilder::new();
        for id in self.cases.keys() {
            fingerprint.update(id.0.to_le_bytes());
        }
        Dataset {
            cases: self.cases,
            fingerprint: fingerprint.finish(),
            metadata: self.metadata,
        }
    }
}
