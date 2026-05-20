//! `Trace2Skill` `SpreadsheetBench` manifest lowering.

use std::{fs, path::Path};

use leaven_eval::{Case, Dataset, DatasetSplits, RowOrderSplitBuilder, SplitRole};
use serde::Deserialize;

const VERIFIED_400_VERSION: &str = "trace2skill-spreadsheetbench-verified-400-v1";
const VERIFIED_400_ROWS: usize = 400;
const VERIFIED_400_EVOLVING_ROWS: usize = 200;

/// Lowered `Trace2Skill` `SpreadsheetBench` manifest.
#[derive(Clone, Debug)]
pub struct Trace2SkillSpreadsheetBenchManifest {
    /// `SpreadsheetBench` rows as Leaven evaluation cases.
    pub dataset: Dataset<Case<SpreadsheetBenchTask, SpreadsheetBenchAnswerSpec>>,
    /// Paper split: first 200 rows train/evolving, last 200 rows held-out test.
    pub splits: DatasetSplits,
}

/// `SpreadsheetBench` task input fields used to run the upstream agent.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpreadsheetBenchTask {
    /// Natural-language spreadsheet instruction.
    pub instruction: String,
    /// Upstream spreadsheet directory path, relative to the dataset root.
    pub spreadsheet_path: String,
    /// Upstream instruction taxonomy.
    pub instruction_type: String,
    /// Optional source data range supplied by the upstream manifest.
    pub data_position: Option<String>,
}

/// `SpreadsheetBench` expected answer location.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpreadsheetBenchAnswerSpec {
    /// Expected output range.
    pub answer_position: String,
    /// Optional sheet for the expected output range.
    pub answer_sheet: Option<String>,
}

/// Loads the official 400-row `Trace2Skill` `SpreadsheetBench`-Verified manifest.
pub fn load_verified_400_manifest(
    path: &Path,
) -> Result<Trace2SkillSpreadsheetBenchManifest, Trace2SkillManifestError> {
    let bytes = fs::read(path)?;
    let rows: Vec<SpreadsheetBenchRow> = serde_json::from_slice(&bytes)?;
    if rows.len() != VERIFIED_400_ROWS {
        return Err(Trace2SkillManifestError::UnexpectedRowCount {
            expected: VERIFIED_400_ROWS,
            actual: rows.len(),
        });
    }

    let cases = rows
        .into_iter()
        .enumerate()
        .map(|(row_index, row)| {
            let source_id = row.id.to_source_id();
            Case::from_source_row(
                row_index,
                source_id,
                SpreadsheetBenchTask {
                    instruction: row.instruction,
                    spreadsheet_path: row.spreadsheet_path,
                    instruction_type: row.instruction_type,
                    data_position: row.data_position,
                },
                Some(SpreadsheetBenchAnswerSpec {
                    answer_position: row.answer_position,
                    answer_sheet: row.answer_sheet,
                }),
            )
        })
        .collect::<Vec<_>>();

    let ordered_cases = cases.iter().map(|case| case.id).collect::<Vec<_>>();
    let dataset = Dataset::from_cases(cases)?;
    let splits = RowOrderSplitBuilder::new(ordered_cases)
        .role_range(SplitRole::Train, 0..VERIFIED_400_EVOLVING_ROWS)
        .role_range(
            SplitRole::Test,
            VERIFIED_400_EVOLVING_ROWS..VERIFIED_400_ROWS,
        )
        .build(leaven_core::CaseSetVersion(VERIFIED_400_VERSION.to_owned()))?;

    Ok(Trace2SkillSpreadsheetBenchManifest { dataset, splits })
}

#[derive(Debug, thiserror::Error)]
pub enum Trace2SkillManifestError {
    /// Manifest file could not be read.
    #[error("failed to read Trace2Skill manifest: {0}")]
    Read(#[from] std::io::Error),
    /// Manifest JSON could not be parsed.
    #[error("failed to parse Trace2Skill manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Manifest row count does not match the official 400-row release.
    #[error("expected {expected} Trace2Skill rows, found {actual}")]
    UnexpectedRowCount {
        /// Expected row count.
        expected: usize,
        /// Actual row count.
        actual: usize,
    },
    /// Leaven dataset construction failed.
    #[error(transparent)]
    Dataset(#[from] leaven_eval::DatasetError),
    /// Leaven split construction failed.
    #[error(transparent)]
    Splits(#[from] leaven_eval::DatasetSplitsError),
}

#[derive(Debug, Deserialize)]
struct SpreadsheetBenchRow {
    id: SpreadsheetBenchSourceId,
    instruction: String,
    spreadsheet_path: String,
    instruction_type: String,
    answer_position: String,
    answer_sheet: Option<String>,
    data_position: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SpreadsheetBenchSourceId {
    String(String),
    Number(u64),
}

impl SpreadsheetBenchSourceId {
    fn to_source_id(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(value) => value.to_string(),
        }
    }
}
