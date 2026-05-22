use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use leaven_core::CaseSetVersion;
use leaven_eval::{SourceRow, SourceRowManifest, SplitRole, StratifiedSplitBuilder};
use leaven_kernel::{CaseId, Fingerprint};
use sha2::{Digest, Sha256};
use smol_str::SmolStr;

pub const DEFAULT_TOLERANCES: [f64; 5] = [0.0, 0.01, 0.025, 0.05, 0.10];
pub const DEFAULT_FAILURE_THRESHOLD: f64 = 0.8;
const YEAR_START: f64 = 1900.0;
const YEAR_END: f64 = 2100.0;
const OFFICEQA_VALIDATION_ROWS: usize = 17;
const OFFICEQA_TRAIN_SIZES: [usize; 3] = [12, 24, 36];

#[derive(Clone, Debug)]
pub struct ManifestBuildInput {
    root: PathBuf,
}

impl ManifestBuildInput {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillReplicaManifest {
    pub schema_version: u32,
    pub paper: PaperTarget,
    pub exactness: ExactnessClass,
    pub source_revisions: Vec<SourceRevision>,
    pub artifacts: Vec<SourceArtifact>,
    pub datasets: Vec<DatasetRequirement>,
    pub source_materializations: Vec<DatasetMaterializationReport>,
    pub scorer: ScorerManifest,
    pub frontier: FrontierManifest,
    pub schedule: ScheduleManifest,
    pub model_pins: Vec<ModelPin>,
    pub blockers: Vec<ReplicationBlocker>,
    pub proxy_rejections: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PaperTarget {
    pub id: String,
    pub arxiv_id: String,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactnessClass {
    BlockedBeforePaperClose,
    PaperCloseCandidate,
    PaperClose,
    PaperExact,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SourceRevision {
    pub id: String,
    pub relative_path: String,
    pub head: Option<String>,
    pub status: SourceRevisionStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRevisionStatus {
    MissingPath,
    NotGitCheckout,
    Present,
    ProbeFailed,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SourceArtifact {
    pub id: String,
    pub role: String,
    pub relative_path: String,
    pub exists: bool,
    pub bytes: Option<u64>,
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DatasetRequirement {
    pub id: String,
    pub paper_rows: Option<u64>,
    pub train_sizes: Vec<u64>,
    pub validation_rows: Option<u64>,
    pub held_out: String,
    pub split_status: SplitManifestStatus,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitManifestStatus {
    ExactPublished,
    PaperCloseSubstituteRequired,
    BlockedMissingCategoryManifest,
    BlockedMissingSplitManifest,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMaterializationStatus {
    MissingArtifact,
    Materialized,
    BlockedMissingRowReader,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterializationExactness {
    PaperExact,
    PaperCloseSubstitute,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StratumMaterializationReport {
    pub name: String,
    pub rows: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SplitMaterializationReport {
    pub id: String,
    pub method: String,
    pub exactness: MaterializationExactness,
    pub train_rows: u64,
    pub validation_rows: Option<u64>,
    pub test_rows: Option<u64>,
    pub split_fingerprint: Option<String>,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DatasetMaterializationReport {
    pub dataset_id: String,
    pub source_artifact_id: String,
    pub source_status: SourceMaterializationStatus,
    pub source_rows: Option<u64>,
    pub case_rows: Option<u64>,
    pub source_row_fingerprint: Option<String>,
    pub source_artifact_sha256: Option<String>,
    pub target_policy: String,
    pub strata: Vec<StratumMaterializationReport>,
    pub split_materializations: Vec<SplitMaterializationReport>,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficeQaInput {
    pub question: String,
    pub source_docs: String,
    pub source_files: String,
    pub difficulty: String,
}

#[derive(Clone, Debug)]
pub struct OfficeQaSourceMaterialization {
    pub rows: SourceRowManifest<OfficeQaInput, String>,
    pub report: DatasetMaterializationReport,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScorerManifest {
    pub id: String,
    pub tolerances: Vec<f64>,
    pub failure_threshold: f64,
    pub implementation_status: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FrontierManifest {
    pub capacity: u64,
    pub parent_selection: String,
    pub admission: String,
    pub eviction: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScheduleManifest {
    pub epochs: f64,
    pub train_batch_policy: String,
    pub feedback_history: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ModelPin {
    pub role: String,
    pub paper_model: String,
    pub leaven_candidate_model: Option<String>,
    pub status: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReplicationBlocker {
    pub id: String,
    pub description: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillScoreReport {
    pub weighted_score: f64,
    pub is_failure: bool,
    pub tolerance_scores: Vec<ToleranceScore>,
}

impl EvoSkillScoreReport {
    #[must_use]
    pub fn tolerances(&self) -> Vec<f64> {
        self.tolerance_scores
            .iter()
            .map(|score| score.tolerance)
            .collect()
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToleranceScore {
    pub tolerance: f64,
    pub weight: f64,
    pub score: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("failed to read `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse CSV `{path}`: {source}")]
    Csv {
        path: PathBuf,
        #[source]
        source: csv::Error,
    },
    #[error("failed to build Leaven source rows: {source}")]
    Dataset {
        #[source]
        source: leaven_eval::DatasetError,
    },
    #[error("failed to build Leaven split materialization: {source}")]
    Split {
        #[source]
        source: leaven_eval::DatasetSplitsError,
    },
}

pub fn build_evoskill_replica_manifest(
    input: &ManifestBuildInput,
) -> Result<EvoSkillReplicaManifest, ManifestError> {
    Ok(EvoSkillReplicaManifest {
        schema_version: 2,
        paper: PaperTarget {
            id: "evoskill".to_owned(),
            arxiv_id: "2603.02766".to_owned(),
            title: "EvoSkill".to_owned(),
        },
        exactness: ExactnessClass::BlockedBeforePaperClose,
        source_revisions: source_revisions(&input.root),
        artifacts: source_artifacts(&input.root)?,
        datasets: dataset_requirements(),
        source_materializations: source_materializations(input)?,
        scorer: scorer_manifest(),
        frontier: frontier_manifest(),
        schedule: schedule_manifest(),
        model_pins: model_pins(),
        blockers: blockers(),
        proxy_rejections: proxy_rejections(),
    })
}

#[derive(Debug, serde::Deserialize)]
struct OfficeQaCsvRecord {
    uid: String,
    question: String,
    answer: String,
    source_docs: String,
    source_files: String,
    difficulty: String,
}

pub fn materialize_officeqa_source(
    input: &ManifestBuildInput,
) -> Result<Option<OfficeQaSourceMaterialization>, ManifestError> {
    let path = input.root.join("tmp/repros/officeqa/officeqa_full.csv");
    if !path.exists() {
        return Ok(None);
    }

    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(&path)
        .map_err(|source| ManifestError::Csv {
            path: path.clone(),
            source,
        })?;
    let mut rows = Vec::new();
    for record in reader.deserialize::<OfficeQaCsvRecord>() {
        let record = record.map_err(|source| ManifestError::Csv {
            path: path.clone(),
            source,
        })?;
        if record.uid.trim().is_empty() {
            continue;
        }
        rows.push(SourceRow::targeted(
            record.uid,
            OfficeQaInput {
                question: record.question,
                source_docs: record.source_docs,
                source_files: record.source_files,
                difficulty: record.difficulty,
            },
            record.answer,
        ));
    }

    let rows = SourceRowManifest::new(rows).map_err(|source| ManifestError::Dataset { source })?;
    let dataset = rows
        .clone()
        .into_dataset()
        .map_err(|source| ManifestError::Dataset { source })?;
    let strata = officeqa_difficulty_strata(&rows);
    let split_materializations = officeqa_difficulty_split_reports(rows.len(), &strata)?;
    let report = DatasetMaterializationReport {
        dataset_id: "officeqa".to_owned(),
        source_artifact_id: "officeqa_full_csv".to_owned(),
        source_status: SourceMaterializationStatus::Materialized,
        source_rows: Some(u64::try_from(rows.len()).expect("row count fits in u64")),
        case_rows: Some(u64::try_from(dataset.cases().len()).expect("case count fits in u64")),
        source_row_fingerprint: Some(fingerprint_hex(rows.fingerprint())),
        source_artifact_sha256: Some(sha256_file(&path)?),
        target_policy: "answers are scorer targets only; runner inputs exclude answers".to_owned(),
        strata: strata_reports(&strata),
        split_materializations,
        blocker_ids: vec![
            "officeqa_category_split_manifest".to_owned(),
            "officeqa_exact_split_membership".to_owned(),
        ],
    };
    Ok(Some(OfficeQaSourceMaterialization { rows, report }))
}

fn source_materializations(
    input: &ManifestBuildInput,
) -> Result<Vec<DatasetMaterializationReport>, ManifestError> {
    let officeqa = materialize_officeqa_source(input)?.map_or_else(
        || {
            missing_materialization_report(
                "officeqa",
                "officeqa_full_csv",
                "officeqa_full_csv_missing",
            )
        },
        |materialization| materialization.report,
    );
    Ok(vec![officeqa, sealqa_materialization_report(&input.root)?])
}

fn officeqa_difficulty_strata(
    rows: &SourceRowManifest<OfficeQaInput, String>,
) -> BTreeMap<SmolStr, Vec<CaseId>> {
    let mut strata = BTreeMap::<SmolStr, Vec<CaseId>>::new();
    for (row_index, row) in rows.rows().iter().enumerate() {
        strata
            .entry(SmolStr::new(row.input().difficulty.as_str()))
            .or_default()
            .push(CaseId::from_index(row_index));
    }
    strata
}

fn officeqa_difficulty_split_reports(
    source_rows: usize,
    strata: &BTreeMap<SmolStr, Vec<CaseId>>,
) -> Result<Vec<SplitMaterializationReport>, ManifestError> {
    let mut reports = Vec::new();
    for train_rows in OFFICEQA_TRAIN_SIZES {
        reports.push(officeqa_difficulty_split_report(
            source_rows,
            strata,
            train_rows,
        )?);
    }
    Ok(reports)
}

fn officeqa_difficulty_split_report(
    source_rows: usize,
    strata: &BTreeMap<SmolStr, Vec<CaseId>>,
    train_rows: usize,
) -> Result<SplitMaterializationReport, ManifestError> {
    let id = format!("officeqa_difficulty_train_{train_rows}_val_{OFFICEQA_VALIDATION_ROWS}");
    if source_rows <= train_rows + OFFICEQA_VALIDATION_ROWS {
        return Ok(SplitMaterializationReport {
            id,
            method: "difficulty_stratified_exact_count".to_owned(),
            exactness: MaterializationExactness::Blocked,
            train_rows: u64::try_from(train_rows).expect("train count fits in u64"),
            validation_rows: Some(
                u64::try_from(OFFICEQA_VALIDATION_ROWS).expect("validation count fits in u64"),
            ),
            test_rows: None,
            split_fingerprint: None,
            blocker_ids: vec!["officeqa_insufficient_rows".to_owned()],
        });
    }

    let test_rows = source_rows - train_rows - OFFICEQA_VALIDATION_ROWS;
    let splits = StratifiedSplitBuilder::new(strata.clone())
        .map_err(|source| ManifestError::Split { source })?
        .role_count(SplitRole::Train, train_rows)
        .role_count(SplitRole::Validation, OFFICEQA_VALIDATION_ROWS)
        .role_count(SplitRole::Test, test_rows)
        .build(CaseSetVersion(format!(
            "officeqa-difficulty-substitute-v1-rows-{source_rows}-train-{train_rows}-val-{OFFICEQA_VALIDATION_ROWS}"
        )))
        .map_err(|source| ManifestError::Split { source })?;

    Ok(SplitMaterializationReport {
        id,
        method: "difficulty_stratified_exact_count".to_owned(),
        exactness: MaterializationExactness::PaperCloseSubstitute,
        train_rows: split_len(&splits, &SplitRole::Train),
        validation_rows: Some(split_len(&splits, &SplitRole::Validation)),
        test_rows: Some(split_len(&splits, &SplitRole::Test)),
        split_fingerprint: Some(fingerprint_hex(splits.fingerprint())),
        blocker_ids: vec![
            "officeqa_category_split_manifest".to_owned(),
            "officeqa_exact_split_membership".to_owned(),
        ],
    })
}

fn split_len(splits: &leaven_eval::DatasetSplits, role: &SplitRole) -> u64 {
    splits.cases(&role.partition_id()).map_or(0, |cases| {
        u64::try_from(cases.len()).expect("split count fits in u64")
    })
}

fn strata_reports(strata: &BTreeMap<SmolStr, Vec<CaseId>>) -> Vec<StratumMaterializationReport> {
    strata
        .iter()
        .map(|(name, rows)| StratumMaterializationReport {
            name: name.to_string(),
            rows: u64::try_from(rows.len()).expect("stratum row count fits in u64"),
        })
        .collect()
}

fn sealqa_materialization_report(
    root: &Path,
) -> Result<DatasetMaterializationReport, ManifestError> {
    let path = root.join("tmp/replication/evoskill/sealqa/seal-0.parquet");
    let source_artifact_sha256 = if path.exists() {
        Some(sha256_file(&path)?)
    } else {
        None
    };
    Ok(DatasetMaterializationReport {
        dataset_id: "sealqa".to_owned(),
        source_artifact_id: "sealqa_parquet".to_owned(),
        source_status: if path.exists() {
            SourceMaterializationStatus::BlockedMissingRowReader
        } else {
            SourceMaterializationStatus::MissingArtifact
        },
        source_rows: None,
        case_rows: None,
        source_row_fingerprint: None,
        source_artifact_sha256,
        target_policy: "blocked: no Leaven-owned seal-0 parquet row reader yet".to_owned(),
        strata: Vec::new(),
        split_materializations: Vec::new(),
        blocker_ids: vec![
            "sealqa_parquet_row_reader".to_owned(),
            "sealqa_split_manifest".to_owned(),
        ],
    })
}

fn missing_materialization_report(
    dataset_id: &str,
    source_artifact_id: &str,
    blocker_id: &str,
) -> DatasetMaterializationReport {
    DatasetMaterializationReport {
        dataset_id: dataset_id.to_owned(),
        source_artifact_id: source_artifact_id.to_owned(),
        source_status: SourceMaterializationStatus::MissingArtifact,
        source_rows: None,
        case_rows: None,
        source_row_fingerprint: None,
        source_artifact_sha256: None,
        target_policy: "blocked: source artifact is not present".to_owned(),
        strata: Vec::new(),
        split_materializations: Vec::new(),
        blocker_ids: vec![blocker_id.to_owned()],
    }
}

#[must_use]
pub fn score_evoskill_answer(ground_truth: &str, prediction: &str) -> EvoSkillScoreReport {
    let tolerance_scores = DEFAULT_TOLERANCES
        .into_iter()
        .map(|tolerance| {
            let weight = tolerance_weight(tolerance);
            ToleranceScore {
                tolerance,
                weight,
                score: score_at_tolerance(ground_truth, prediction, tolerance),
            }
        })
        .collect::<Vec<_>>();
    let weight_total = tolerance_scores
        .iter()
        .map(|score| score.weight)
        .sum::<f64>();
    let weighted_score = tolerance_scores
        .iter()
        .map(|score| score.weight * score.score)
        .sum::<f64>()
        / weight_total;
    EvoSkillScoreReport {
        weighted_score,
        is_failure: weighted_score < DEFAULT_FAILURE_THRESHOLD,
        tolerance_scores,
    }
}

fn score_at_tolerance(ground_truth: &str, prediction: &str, tolerance: f64) -> f64 {
    if prediction.trim().is_empty() {
        return 0.0;
    }
    let ground_numbers = number_mentions(ground_truth);
    let prediction_numbers = number_mentions(prediction);
    if !ground_numbers.is_empty() {
        if prediction_numbers.is_empty() {
            return 0.0;
        }
        let prediction_numbers = filter_prediction_years(ground_truth, prediction_numbers);
        let normalized_prediction = normalized_text(prediction);
        let text_ok = required_text_tokens(ground_truth)
            .iter()
            .all(|token| normalized_prediction.contains(token));
        let numbers_ok = ground_numbers.iter().all(|ground| {
            prediction_numbers.iter().any(|prediction| {
                numeric_match(ground.base_value, prediction.base_value, tolerance)
            })
        });
        return f64::from(numbers_ok && text_ok);
    }
    let truth = normalized_text(ground_truth);
    let predicted = normalized_text(prediction);
    f64::from(!truth.is_empty() && predicted.contains(&truth))
}

fn tolerance_weight(tolerance: f64) -> f64 {
    1.0 / (1.0 + 20.0 * tolerance)
}

#[derive(Clone, Debug)]
struct NumberMention {
    base_value: f64,
}

fn number_mentions(text: &str) -> Vec<NumberMention> {
    let mut mentions = Vec::new();
    let mut start = None;
    for (index, ch) in text.char_indices() {
        if ch.is_ascii_digit() || ch == '-' || ch == '.' || ch == ',' {
            start.get_or_insert(index);
        } else if let Some(token_start) = start.take() {
            push_number_mention(text, token_start, index, &mut mentions);
        }
    }
    if let Some(token_start) = start {
        push_number_mention(text, token_start, text.len(), &mut mentions);
    }
    mentions
}

fn push_number_mention(text: &str, start: usize, end: usize, mentions: &mut Vec<NumberMention>) {
    let raw = &text[start..end];
    let normalized = raw.replace(',', "");
    if normalized.is_empty() || normalized == "-" || normalized == "." {
        return;
    }
    if let Ok(value) = normalized.parse::<f64>() {
        mentions.push(NumberMention {
            base_value: value * unit_multiplier(context_window(text, start, end)).unwrap_or(1.0),
        });
    }
}

fn context_window(text: &str, start: usize, end: usize) -> &str {
    let context_start = text[..start]
        .char_indices()
        .rev()
        .nth(19)
        .map_or(0, |(index, _)| index);
    let context_end = text[end..]
        .char_indices()
        .nth(20)
        .map_or(text.len(), |(index, _)| end + index);
    &text[context_start..context_end]
}

fn unit_multiplier(context: &str) -> Option<f64> {
    let context = context.to_ascii_lowercase();
    if context.contains("trillion") {
        Some(1_000_000_000_000.0)
    } else if context.contains("billion") {
        Some(1_000_000_000.0)
    } else if context.contains("million") {
        Some(1_000_000.0)
    } else {
        None
    }
}

fn filter_prediction_years(
    ground_truth: &str,
    prediction_numbers: Vec<NumberMention>,
) -> Vec<NumberMention> {
    if ground_truth_allows_years(ground_truth) {
        return prediction_numbers;
    }
    prediction_numbers
        .into_iter()
        .filter(|number| !is_year_like(number.base_value))
        .collect()
}

fn ground_truth_allows_years(ground_truth: &str) -> bool {
    number_mentions(ground_truth)
        .iter()
        .any(|number| is_year_like(number.base_value))
        || !required_text_tokens(ground_truth).is_empty()
}

fn is_year_like(value: f64) -> bool {
    value.fract() == 0.0 && (YEAR_START..=YEAR_END).contains(&value)
}

fn numeric_match(ground_truth: f64, prediction: f64, tolerance: f64) -> bool {
    if ground_truth == 0.0 {
        prediction == 0.0
    } else {
        ((ground_truth - prediction).abs() / ground_truth.abs()) <= tolerance
    }
}

fn required_text_tokens(text: &str) -> Vec<String> {
    normalized_text_without_numbers(text)
        .split_whitespace()
        .filter(|token| !is_ignored_text_token(token))
        .map(str::to_owned)
        .collect()
}

fn normalized_text(text: &str) -> String {
    strip_parentheticals(text)
        .trim()
        .to_ascii_lowercase()
        .replace(['"', '\'', ',', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalized_text_without_numbers(text: &str) -> String {
    let stripped = strip_number_runs(text);
    normalized_text(&stripped)
}

fn strip_number_runs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '-' || ch == '.' || ch == ',' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn strip_parentheticals(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0_u32;
    for ch in text.chars() {
        match ch {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

fn is_unit_word(token: &str) -> bool {
    matches!(
        token,
        "million" | "millions" | "billion" | "billions" | "trillion" | "trillions"
    )
}

fn is_ignored_text_token(token: &str) -> bool {
    is_unit_word(token) || matches!(token, "and" | "or" | "the" | "a" | "an" | "of")
}

fn source_revisions(root: &Path) -> Vec<SourceRevision> {
    vec![
        source_revision(root, "evoskill_repo", "tmp/repros/evoskill"),
        source_revision(root, "officeqa_repo", "tmp/repros/officeqa"),
    ]
}

fn source_artifacts(root: &Path) -> Result<Vec<SourceArtifact>, ManifestError> {
    Ok(vec![
        source_artifact(
            root,
            "paper_full_source",
            "paper source text",
            "tmp/skill_opt_sources/arx_2603.02766/full_source.md",
        )?,
        source_artifact(
            root,
            "officeqa_full_csv",
            "OfficeQA full CSV",
            "tmp/repros/officeqa/officeqa_full.csv",
        )?,
        source_artifact(
            root,
            "officeqa_pro_csv",
            "OfficeQA pro CSV",
            "tmp/repros/officeqa/officeqa_pro.csv",
        )?,
        source_artifact(
            root,
            "sealqa_parquet",
            "SealQA seal-0 parquet",
            "tmp/replication/evoskill/sealqa/seal-0.parquet",
        )?,
        source_artifact(
            root,
            "officeqa_validation_sample",
            "OfficeQA inspected sample",
            "tmp/paper_exact_samples/evoskill/officeqa/officeqa_pro_first_case.json",
        )?,
        source_artifact(
            root,
            "sealqa_validation_sample",
            "SealQA inspected sample",
            "tmp/paper_exact_samples/evoskill/sealqa/seal_0_first_case.json",
        )?,
    ])
}

fn dataset_requirements() -> Vec<DatasetRequirement> {
    vec![
        DatasetRequirement {
            id: "officeqa".to_owned(),
            paper_rows: Some(246),
            train_sizes: vec![12, 24, 36],
            validation_rows: Some(17),
            held_out: "paper reports held-out test and skill-merge tables".to_owned(),
            split_status: SplitManifestStatus::BlockedMissingCategoryManifest,
            blocker_ids: vec![
                "officeqa_category_split_manifest".to_owned(),
                "officeqa_exact_split_membership".to_owned(),
                "officeqa_reported_result_target".to_owned(),
            ],
        },
        DatasetRequirement {
            id: "sealqa".to_owned(),
            paper_rows: Some(111),
            train_sizes: vec![11],
            validation_rows: None,
            held_out: "paper uses 10 percent train and held-out remainder".to_owned(),
            split_status: SplitManifestStatus::BlockedMissingSplitManifest,
            blocker_ids: vec![
                "sealqa_parquet_row_reader".to_owned(),
                "sealqa_split_manifest".to_owned(),
            ],
        },
        DatasetRequirement {
            id: "browsecomp_transfer".to_owned(),
            paper_rows: Some(128),
            train_sizes: Vec::new(),
            validation_rows: None,
            held_out: "transfer-only evaluation from SealQA skill".to_owned(),
            split_status: SplitManifestStatus::BlockedMissingSplitManifest,
            blocker_ids: vec!["browsecomp_transfer_sample".to_owned()],
        },
    ]
}

fn scorer_manifest() -> ScorerManifest {
    ScorerManifest {
        id: "evoskill-multi-tolerance-v1".to_owned(),
        tolerances: vec![0.0, 0.01, 0.025, 0.05, 0.10],
        failure_threshold: 0.8,
        implementation_status:
            "Rust scorer law-tested for multi-tolerance weighting, units, years, text, and lists"
                .to_owned(),
    }
}

fn frontier_manifest() -> FrontierManifest {
    FrontierManifest {
        capacity: 3,
        parent_selection: "round-robin".to_owned(),
        admission: "validate child before frontier admission".to_owned(),
        eviction: "evict weakest frontier member when full".to_owned(),
    }
}

fn schedule_manifest() -> ScheduleManifest {
    ScheduleManifest {
        epochs: 1.5,
        train_batch_policy: "category-aware without-replacement train batches".to_owned(),
        feedback_history: "proposer sees prior failures and feedback history".to_owned(),
    }
}

fn model_pins() -> Vec<ModelPin> {
    vec![
        ModelPin {
            role: "paper_agent_runtime".to_owned(),
            paper_model: "Claude Code Opus 4.5 per paper ledger".to_owned(),
            leaven_candidate_model: Some(
                "Codex gpt-5.4-mini low for approved small live runs".to_owned(),
            ),
            status: "paper-close may use a declared model delta; paper-exact cannot".to_owned(),
        },
        ModelPin {
            role: "underlying_task_model".to_owned(),
            paper_model:
                "frozen underlying model; exact provider/version unresolved in local ledger"
                    .to_owned(),
            leaven_candidate_model: None,
            status: "blocked until source manifest resolves paper pin".to_owned(),
        },
    ]
}

fn blockers() -> Vec<ReplicationBlocker> {
    vec![
        blocker(
            "source_pin",
            "choose paper-release source, local checkout, or current upstream revision before comparing behavior",
        ),
        blocker(
            "officeqa_category_split_manifest",
            "OfficeQA paper category/pseudo-label split artifact is not present locally",
        ),
        blocker(
            "officeqa_exact_split_membership",
            "OfficeQA can be difficulty-stratified as a documented substitute, but exact paper split membership is absent",
        ),
        blocker(
            "sealqa_parquet_row_reader",
            "SealQA parquet is pinned locally, but Leaven does not yet have a no-spend Rust row reader for seal-0",
        ),
        blocker(
            "sealqa_split_manifest",
            "SealQA seal-0 parquet is pinned, but Leaven still lacks exact train/held-out split membership",
        ),
        blocker(
            "browsecomp_transfer_sample",
            "BrowseComp 128-example transfer sample/result source is not present locally",
        ),
        blocker(
            "officeqa_reported_result_target",
            "OfficeQA prose and table report disagree between 67.9 and 68.1 percent",
        ),
    ]
}

fn proxy_rejections() -> Vec<String> {
    vec![
        "P5 one-iteration fixture is product wiring evidence, not paper-close completion".to_owned(),
        "Git trust benchmark proves substrate isolation/performance, not EvoSkill paper semantics"
            .to_owned(),
        "Fake runtime child admission is mechanics evidence, not live paper behavior".to_owned(),
        "Single OfficeQA/SealQA sample inspection does not prove split construction or held-out reporting".to_owned(),
        "just check/topology proves repo health only".to_owned(),
    ]
}

fn source_revision(root: &Path, id: &str, relative_path: &str) -> SourceRevision {
    let path = root.join(relative_path);
    let (head, status) = if !path.exists() {
        (None, SourceRevisionStatus::MissingPath)
    } else if !path.join(".git").exists() {
        (None, SourceRevisionStatus::NotGitCheckout)
    } else {
        match Command::new("git")
            .arg("-C")
            .arg(&path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
        {
            Ok(output) if output.status.success() => (
                Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()),
                SourceRevisionStatus::Present,
            ),
            _ => (None, SourceRevisionStatus::ProbeFailed),
        }
    };
    SourceRevision {
        id: id.to_owned(),
        relative_path: relative_path.to_owned(),
        head,
        status,
    }
}

fn source_artifact(
    root: &Path,
    id: &str,
    role: &str,
    relative_path: &str,
) -> Result<SourceArtifact, ManifestError> {
    let path = root.join(relative_path);
    if !path.exists() {
        return Ok(SourceArtifact {
            id: id.to_owned(),
            role: role.to_owned(),
            relative_path: relative_path.to_owned(),
            exists: false,
            bytes: None,
            sha256: None,
        });
    }
    let metadata = path.metadata().map_err(|source| ManifestError::Read {
        path: path.clone(),
        source,
    })?;
    Ok(SourceArtifact {
        id: id.to_owned(),
        role: role.to_owned(),
        relative_path: relative_path.to_owned(),
        exists: true,
        bytes: Some(metadata.len()),
        sha256: Some(sha256_file(&path)?),
    })
}

fn sha256_file(path: &Path) -> Result<String, ManifestError> {
    let mut file = File::open(path).map_err(|source| ManifestError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];
    loop {
        let read = file.read(&mut buf).map_err(|source| ManifestError::Read {
            path: path.to_owned(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn fingerprint_hex(fingerprint: Fingerprint) -> String {
    hex::encode(fingerprint.0)
}

fn blocker(id: &str, description: &str) -> ReplicationBlocker {
    ReplicationBlocker {
        id: id.to_owned(),
        description: description.to_owned(),
    }
}
