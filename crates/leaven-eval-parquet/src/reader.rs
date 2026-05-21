//! Parquet source-row reader.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use arrow_array::{Array, LargeListArray, LargeStringArray, ListArray, RecordBatch, StringArray};
use leaven_eval::{DatasetError, SourceRow, SourceRowManifest};
use leaven_kernel::{Fingerprint, FingerprintBuilder};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use thiserror::Error;

/// Reads a Parquet file as ordered source rows.
pub fn read_parquet_source_rows<I, T, F>(
    path: impl AsRef<Path>,
    mut map: F,
) -> Result<ParquetSourceRows<I, T>, ParquetSourceRowError>
where
    F: FnMut(&ParquetRow<'_>) -> Result<SourceRow<I, T>, ParquetSourceRowError>,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| ParquetSourceRowError::ReadFile {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })?;
    let file_fingerprint = file_fingerprint(&bytes);
    let file = File::open(path).map_err(|source| ParquetSourceRowError::ReadFile {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|source| ParquetSourceRowError::Parquet {
            path: path.to_path_buf(),
            reason: source.to_string(),
        })?
        .build()
        .map_err(|source| ParquetSourceRowError::Parquet {
            path: path.to_path_buf(),
            reason: source.to_string(),
        })?;

    let mut source_rows = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|source| ParquetSourceRowError::Parquet {
            path: path.to_path_buf(),
            reason: source.to_string(),
        })?;
        for row_in_batch in 0..batch.num_rows() {
            let row = ParquetRow {
                batch: &batch,
                row_index: source_rows.len(),
                row_in_batch,
            };
            source_rows.push(map(&row)?);
        }
    }
    let row_count = source_rows.len();
    let manifest = SourceRowManifest::new(source_rows)?;
    Ok(ParquetSourceRows {
        manifest,
        row_count,
        file_fingerprint,
    })
}

/// Source rows read from one Parquet file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParquetSourceRows<I, T> {
    manifest: SourceRowManifest<I, T>,
    row_count: usize,
    file_fingerprint: Fingerprint,
}

impl<I, T> ParquetSourceRows<I, T> {
    /// Number of physical rows read from the Parquet file.
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Fingerprint over the exact Parquet file bytes.
    pub const fn file_fingerprint(&self) -> Fingerprint {
        self.file_fingerprint
    }

    /// Ordered source-row manifest.
    pub const fn manifest(&self) -> &SourceRowManifest<I, T> {
        &self.manifest
    }

    /// Consumes the wrapper into the ordered source-row manifest.
    pub fn into_manifest(self) -> SourceRowManifest<I, T> {
        self.manifest
    }
}

/// Cursor over one Parquet row.
#[derive(Debug)]
pub struct ParquetRow<'a> {
    batch: &'a RecordBatch,
    row_index: usize,
    row_in_batch: usize,
}

impl ParquetRow<'_> {
    /// Zero-based row index across the whole file.
    pub const fn row_index(&self) -> usize {
        self.row_index
    }

    /// Reads a non-null UTF-8 string column.
    pub fn required_string(&self, column: &str) -> Result<String, ParquetSourceRowError> {
        self.optional_string(column)?
            .ok_or_else(|| ParquetSourceRowError::NullRequired {
                column: column.to_owned(),
                row: self.row_index,
            })
    }

    /// Reads a nullable UTF-8 string column.
    pub fn optional_string(&self, column: &str) -> Result<Option<String>, ParquetSourceRowError> {
        let array = self.column(column)?;
        if array.is_null(self.row_in_batch) {
            return Ok(None);
        }
        string_value(column, array, self.row_in_batch).map(Some)
    }

    /// Reads a nullable list-of-UTF-8-strings column.
    pub fn optional_string_list(&self, column: &str) -> Result<Vec<String>, ParquetSourceRowError> {
        let array = self.column(column)?;
        if array.is_null(self.row_in_batch) {
            return Ok(Vec::new());
        }
        if let Some(list) = array.as_any().downcast_ref::<ListArray>() {
            let values = list.value(self.row_in_batch);
            return string_values(column, self.row_index, values.as_ref());
        }
        if let Some(list) = array.as_any().downcast_ref::<LargeListArray>() {
            let values = list.value(self.row_in_batch);
            return string_values(column, self.row_index, values.as_ref());
        }
        Err(ParquetSourceRowError::UnsupportedColumn {
            column: column.to_owned(),
            expected: "list<utf8>".to_owned(),
            actual: format!("{:?}", array.data_type()),
        })
    }

    fn column(&self, column: &str) -> Result<&dyn Array, ParquetSourceRowError> {
        self.batch
            .column_by_name(column)
            .map(std::convert::AsRef::as_ref)
            .ok_or_else(|| ParquetSourceRowError::MissingColumn {
                column: column.to_owned(),
            })
    }
}

/// Parquet source-row materialization failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ParquetSourceRowError {
    /// The Parquet file could not be read from disk.
    #[error("could not read parquet file {path}: {reason}")]
    ReadFile {
        /// File path.
        path: PathBuf,
        /// Read failure reason.
        reason: String,
    },
    /// Parquet/Arrow decoding failed.
    #[error("could not decode parquet file {path}: {reason}")]
    Parquet {
        /// File path.
        path: PathBuf,
        /// Decode failure reason.
        reason: String,
    },
    /// A requested column does not exist.
    #[error("missing parquet column: {column}")]
    MissingColumn {
        /// Column name.
        column: String,
    },
    /// A requested required value is null in the physical file.
    #[error("required parquet column {column} is null at row {row}")]
    NullRequired {
        /// Column name.
        column: String,
        /// Physical source-row index.
        row: usize,
    },
    /// A requested column has an unsupported physical type.
    #[error("unsupported parquet column {column}: expected {expected}, got {actual}")]
    UnsupportedColumn {
        /// Column name.
        column: String,
        /// Expected logical type.
        expected: String,
        /// Actual Arrow type.
        actual: String,
    },
    /// A list-of-strings column contains a null element.
    #[error("parquet column {column} has a null list element at row {row}, element {element}")]
    NullListElement {
        /// Column name.
        column: String,
        /// Physical source-row index.
        row: usize,
        /// Element index inside the list value.
        element: usize,
    },
    /// The lowered source-row manifest was invalid.
    #[error(transparent)]
    Dataset(#[from] DatasetError),
}

fn file_fingerprint(bytes: &[u8]) -> Fingerprint {
    let mut fingerprint = FingerprintBuilder::new();
    fingerprint.update(b"leaven-eval-parquet:file:v1");
    fingerprint.update(bytes);
    fingerprint.finish()
}

fn string_value(
    column: &str,
    array: &dyn Array,
    index: usize,
) -> Result<String, ParquetSourceRowError> {
    if let Some(strings) = array.as_any().downcast_ref::<StringArray>() {
        return Ok(strings.value(index).to_owned());
    }
    if let Some(strings) = array.as_any().downcast_ref::<LargeStringArray>() {
        return Ok(strings.value(index).to_owned());
    }
    Err(ParquetSourceRowError::UnsupportedColumn {
        column: column.to_owned(),
        expected: "utf8".to_owned(),
        actual: format!("{:?}", array.data_type()),
    })
}

fn string_values(
    column: &str,
    row: usize,
    array: &dyn Array,
) -> Result<Vec<String>, ParquetSourceRowError> {
    if let Some(strings) = array.as_any().downcast_ref::<StringArray>() {
        return collect_string_values(column, row, strings);
    }
    if let Some(strings) = array.as_any().downcast_ref::<LargeStringArray>() {
        return collect_string_values(column, row, strings);
    }
    Err(ParquetSourceRowError::UnsupportedColumn {
        column: column.to_owned(),
        expected: "list<utf8>".to_owned(),
        actual: format!("{:?}", array.data_type()),
    })
}

fn collect_string_values<A>(
    column: &str,
    row: usize,
    strings: &A,
) -> Result<Vec<String>, ParquetSourceRowError>
where
    A: StringValueArray,
{
    (0..strings.len())
        .map(|index| {
            if strings.is_null(index) {
                Err(ParquetSourceRowError::NullListElement {
                    column: column.to_owned(),
                    row,
                    element: index,
                })
            } else {
                Ok(strings.value(index).to_owned())
            }
        })
        .collect()
}

trait StringValueArray {
    fn len(&self) -> usize;
    fn is_null(&self, index: usize) -> bool;
    fn value(&self, index: usize) -> &str;
}

impl StringValueArray for StringArray {
    fn len(&self) -> usize {
        Array::len(self)
    }

    fn is_null(&self, index: usize) -> bool {
        Array::is_null(self, index)
    }

    fn value(&self, index: usize) -> &str {
        Self::value(self, index)
    }
}

impl StringValueArray for LargeStringArray {
    fn len(&self) -> usize {
        Array::len(self)
    }

    fn is_null(&self, index: usize) -> bool {
        Array::is_null(self, index)
    }

    fn value(&self, index: usize) -> &str {
        Self::value(self, index)
    }
}
