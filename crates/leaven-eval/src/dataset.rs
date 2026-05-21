//! Dataset and case records.

use std::collections::BTreeMap;

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
