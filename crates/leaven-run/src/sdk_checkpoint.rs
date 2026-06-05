//! Rust-owned checkpoint materialization for SDK mechanics reports.

use std::path::Path;

use base64::{Engine as _, engine::general_purpose};
use bytes::Bytes;
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, OptimizationProblem, ResolvedRequestKind,
};
use leaven_engine::{
    CachePolicy, CaseSet, Engine, EvaluationContext, EvaluationError, Evaluator, Optimizer,
    OptimizerError, RunContext, StepStatus, StoreRunPersistence,
};
use leaven_eval::Case;
use leaven_evidence::{CaseAssessmentEvidence, CaseDataReadEvidence, OutputRecord, ScalarEvidence};
use leaven_kernel::{
    Budget, CandidateId, CaseId, ContentId, Cost, EvaluationSetId, EvaluatorId, Fingerprint,
    FingerprintBuilder, MetadataBag, MetadataValue, Metered, RunId,
};
use leaven_store::{BlobStore, BlobWrite};
use leaven_store_file::{FileEvidenceStore, FileStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use thiserror::Error;
use uuid::Uuid;

/// Schema name for SDK prompt mechanics materialization input.
pub const SDK_PROMPT_RUN_RECORD_SCHEMA: &str = "leaven.sdk_prompt_run_record.v1";

/// Typed SDK mechanics report consumed by Rust checkpoint materialization.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SdkPromptRunRecord {
    /// Input schema.
    pub schema_version: String,
    /// Completed run id.
    pub run_id: String,
    /// Seed prompt artifact.
    pub seed: SdkPromptArtifact,
    /// Ordered case rows used by the mechanics run.
    pub cases: Vec<SdkPromptCase>,
    /// Per-case assessment rows computed through the SDK seam.
    pub assessments: Vec<SdkPromptAssessment>,
    /// LM tokens reported by the mechanics route.
    #[serde(default)]
    pub total_lm_tokens: u64,
}

/// Prompt artifact shape shared with the Python foundation slice.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SdkPromptArtifact {
    /// Prompt template text.
    pub template: String,
    /// Public candidate id used by the SDK projection.
    pub candidate_id: String,
}

/// Placeholder change type for the seed-only prompt checkpoint slice.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum SdkPromptChange {}

impl Artifact for SdkPromptArtifact {
    type ApplyError = SdkPromptChangeError;
    type Change = SdkPromptChange;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(self.template.as_bytes()))
    }

    fn cache_identity(&self) -> Option<leaven_core::CacheIdentity> {
        Some(leaven_core::CacheIdentity::Content(ContentId::hash_bytes(
            self.template.as_bytes(),
        )))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        match *change {}
    }
}

/// Unreachable prompt change application error for the seed-only slice.
#[derive(Clone, Debug, Error)]
#[error("SDK prompt checkpoint slice has no prompt change variant")]
pub struct SdkPromptChangeError;

/// One ordered SDK case row.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SdkPromptCase {
    /// Public SDK case id.
    pub case_id: String,
    /// Case input JSON.
    pub input: Value,
    /// Optional target JSON.
    pub target: Option<Value>,
    /// Optional split label.
    pub split: Option<String>,
}

/// One SDK assessment row.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SdkPromptAssessment {
    /// Public SDK case id.
    pub case_id: String,
    /// Case target JSON used by the scorer, when present.
    pub target: Option<Value>,
    /// Candidate output value.
    pub output: Value,
    /// Scalar score.
    pub score: f64,
    /// Feedback attached to the score.
    pub feedback: String,
    /// Reward-vector dimensions that contributed to the score.
    #[serde(default)]
    pub rewards: Vec<SdkPromptReward>,
    /// Effect receipts produced while computing this assessment.
    #[serde(default)]
    pub effect_receipts: Vec<SdkPromptEffectReceipt>,
    /// Effect blob contents to materialize into Rust-owned stage journal blobs.
    #[serde(default)]
    pub effect_blob_contents: Vec<SdkPromptEffectBlobContent>,
}

/// One effect receipt and its public blob metadata from the SDK mechanics route.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SdkPromptEffectReceipt {
    /// Opaque receipt id.
    pub receipt_id: String,
    /// Blob refs attached to the receipt, such as agent transcripts.
    #[serde(default)]
    pub blob_refs: Vec<SdkPromptBlobRef>,
}

/// Public blob metadata attached to an effect receipt.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SdkPromptBlobRef {
    /// Opaque blob id.
    pub blob_id: String,
    /// Optional SHA-256 digest.
    #[serde(default)]
    pub sha256: Option<String>,
    /// Optional byte count.
    #[serde(default)]
    pub bytes: Option<u64>,
    /// Public data classes associated with the blob.
    #[serde(default)]
    pub data_classes: Vec<String>,
}

/// Private SDK checkpoint blob bytes bound to a public blob ref.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SdkPromptEffectBlobContent {
    /// Receipt that produced this blob.
    pub receipt_id: String,
    /// Public blob metadata reported by the callback receipt.
    pub blob_ref: SdkPromptBlobRef,
    /// Base64-encoded blob bytes.
    pub content_base64: String,
}

/// One reward-vector dimension from the SDK scorer.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SdkPromptReward {
    /// Dimension id.
    pub id: String,
    /// Dimension value.
    pub value: f64,
    /// Dimension weight.
    pub weight: f64,
    /// Dimension feedback.
    pub feedback: String,
}

/// Materialization report returned to the SDK.
#[derive(Clone, Debug, Serialize)]
pub struct SdkPromptCheckpointReport {
    /// Output schema.
    pub schema_version: &'static str,
    /// Run directory that now contains Rust-owned checkpoint state.
    pub run_dir: String,
    /// Number of assessments materialized into the Rust graph.
    pub assessment_count: usize,
}

/// Materialize one SDK prompt mechanics report into a Rust-owned run checkpoint.
pub fn materialize_sdk_prompt_checkpoint(
    record: SdkPromptRunRecord,
    run_dir: impl AsRef<Path>,
) -> Result<SdkPromptCheckpointReport, SdkPromptCheckpointError> {
    record.validate()?;
    let run_dir = run_dir.as_ref();
    let file_store = FileStore::open(run_dir).map_err(SdkPromptCheckpointError::Store)?;
    let persistence = StoreRunPersistence::new(file_store.clone());
    let evidence_store = FileEvidenceStore::<CaseAssessmentEvidence>::open(
        "case-assessment",
        run_dir.join("evidence"),
    )
    .map_err(SdkPromptCheckpointError::Store)?;
    let case_rows = record.case_rows();
    let case_set = CaseSet::new(case_rows.clone());
    let mut engine = Engine::<SdkPromptRunProblem>::builder()
        .run_id(RunId::from_uuid(run_uuid(&record.run_id)))
        .budget(Budget::unlimited())
        .persistence(persistence)
        .build();
    let mut optimizer = SdkPromptCheckpointOptimizer::new(
        record.seed.clone(),
        record.cases.clone(),
        record.assessments.clone(),
        record.total_lm_tokens,
        file_store,
    );
    futures::executor::block_on(engine.run(&mut optimizer, &case_set, &evidence_store))
        .map_err(SdkPromptCheckpointError::Optimizer)?;
    Ok(SdkPromptCheckpointReport {
        schema_version: "leaven.sdk_prompt_checkpoint_report.v1",
        run_dir: run_dir.display().to_string(),
        assessment_count: record.assessments.len(),
    })
}

/// Marker problem for SDK prompt checkpoint materialization.
pub struct SdkPromptRunProblem;

impl OptimizationProblem for SdkPromptRunProblem {
    type Artifact = SdkPromptArtifact;
    type Case = Case<Value, Value>;
    type Evidence = CaseAssessmentEvidence;
    type ProposalAnnotations = ();
}

struct SdkPromptEvaluator {
    candidate: CandidateId,
    cases: Vec<SdkPromptCase>,
    assessments: Vec<SdkPromptAssessment>,
    total_lm_tokens: u64,
}

struct SdkPromptCheckpointOptimizer {
    seed: SdkPromptArtifact,
    cases: Vec<SdkPromptCase>,
    assessments: Vec<SdkPromptAssessment>,
    total_lm_tokens: u64,
    store: FileStore,
    candidate: Option<CandidateId>,
    evaluated: bool,
}

impl SdkPromptCheckpointOptimizer {
    fn new(
        seed: SdkPromptArtifact,
        cases: Vec<SdkPromptCase>,
        assessments: Vec<SdkPromptAssessment>,
        total_lm_tokens: u64,
        store: FileStore,
    ) -> Self {
        Self {
            seed,
            cases,
            assessments,
            total_lm_tokens,
            store,
            candidate: None,
            evaluated: false,
        }
    }

    fn record_effect_blob_contents(
        &self,
        ctx: &mut RunContext<'_, SdkPromptRunProblem>,
    ) -> Result<(), OptimizerError> {
        for assessment in &self.assessments {
            for content in &assessment.effect_blob_contents {
                let bytes = decode_blob_content(content).map_err(|source| {
                    OptimizerError::with_source("decode SDK effect blob", source)
                })?;
                let reference = BlobStore::put(
                    &self.store,
                    BlobWrite {
                        bytes: Bytes::from(bytes),
                        content_type: Some("application/json".to_owned()),
                    },
                )
                .map_err(|source| OptimizerError::with_source("write SDK effect blob", source))?;
                ctx.record_stage_journal_entry(reference)
                    .map_err(|source| {
                        OptimizerError::with_source("record SDK effect blob stage journal", source)
                    })?;
            }
        }
        Ok(())
    }
}

impl Optimizer<SdkPromptRunProblem> for SdkPromptCheckpointOptimizer {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, SdkPromptRunProblem>,
    ) -> Result<(), OptimizerError> {
        let candidate = ctx
            .insert_seed(self.seed.clone(), 0)
            .map_err(|source| OptimizerError::with_source("insert SDK prompt seed", source))?;
        self.candidate = Some(candidate);
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, SdkPromptRunProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        if self.evaluated {
            return Ok(StepStatus::Done);
        }
        self.record_effect_blob_contents(ctx)?;
        let candidate = self.candidate.ok_or_else(|| {
            OptimizerError::Message("SDK prompt seed was not initialized".to_owned())
        })?;
        let evaluator = SdkPromptEvaluator::new(
            candidate,
            self.cases.clone(),
            self.assessments.clone(),
            self.total_lm_tokens,
        );
        ctx.evaluate_with(
            &evaluator,
            EvaluationRequest::Independent {
                candidates: vec![candidate],
                set: EvaluationSet::All,
                granularity: AssessmentGranularity::PerCase,
                purpose: EvaluationPurpose::FinalTest,
            },
        )
        .await
        .map_err(|source| OptimizerError::with_source("evaluate SDK prompt rows", source))?;
        self.evaluated = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, SdkPromptRunProblem>,
    ) -> Option<CandidateId> {
        self.candidate
    }
}

impl SdkPromptEvaluator {
    fn new(
        candidate: CandidateId,
        cases: Vec<SdkPromptCase>,
        assessments: Vec<SdkPromptAssessment>,
        total_lm_tokens: u64,
    ) -> Self {
        Self {
            candidate,
            cases,
            assessments,
            total_lm_tokens,
        }
    }
}

impl Evaluator<SdkPromptRunProblem> for SdkPromptEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::from("sdk_prompt_reward_vector")
    }

    fn fingerprint(&self) -> Fingerprint {
        let mut builder = FingerprintBuilder::new();
        builder.update("sdk_prompt_reward_vector");
        builder.finish()
    }

    fn cache_policy(&self, _request: &leaven_core::ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: leaven_core::ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, SdkPromptRunProblem>,
    ) -> Result<Metered<Vec<Assessment<SdkPromptRunProblem>>>, EvaluationError> {
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "SDK prompt checkpoint only supports independent assessment".to_owned(),
            ));
        };
        if candidates.as_slice() != [self.candidate] {
            return Err(EvaluationError::Message(
                "SDK prompt checkpoint evaluator received an unexpected candidate".to_owned(),
            ));
        }
        let set = EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let rows = request
            .set
            .case_ids
            .iter()
            .zip(self.cases.iter())
            .zip(self.assessments.iter())
            .map(|((case, row_case), row)| self.assessment(set, *case, row_case, row))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Metered::new(rows, self.cost()))
    }
}

impl SdkPromptEvaluator {
    fn cost(&self) -> Cost {
        if self.total_lm_tokens == 0 {
            return Cost::zero();
        }
        Cost {
            llm_calls: 1,
            prompt_tokens: self.total_lm_tokens,
            ..Cost::zero()
        }
    }

    fn assessment(
        &self,
        set: EvaluationSetId,
        case: CaseId,
        row_case: &SdkPromptCase,
        row: &SdkPromptAssessment,
    ) -> Result<Assessment<SdkPromptRunProblem>, EvaluationError> {
        let score = ScalarEvidence::new(row.score)
            .map_err(|source| EvaluationError::with_source("invalid SDK prompt score", source))?;
        let output = OutputRecord::candidate_inline(output_text(&row.output));
        let mut read = CaseDataReadEvidence::new(
            "sdk_prompt_case",
            format!("qrec_{}", row.case_id),
            case,
            ["input", "target"],
            ["case.input", "case.target"],
        )
        .with_value("case_id", Value::String(row.case_id.clone()));
        if let Some(target) = row.target.clone() {
            read = read.with_value("target", target);
        }
        if !row.rewards.is_empty() {
            read = read.with_value(
                "rewards",
                serde_json::to_value(&row.rewards).map_err(|source| {
                    EvaluationError::with_source("serialize SDK prompt rewards", source)
                })?,
            );
        }
        if !row.effect_receipts.is_empty() {
            read = read.with_value(
                "effect_receipts",
                serde_json::to_value(&row.effect_receipts).map_err(|source| {
                    EvaluationError::with_source("serialize SDK prompt effect receipts", source)
                })?,
            );
        }
        let evidence = CaseAssessmentEvidence::new(score, output, row.feedback.clone())
            .with_case_data_reads([read]);
        let mut metadata = MetadataBag::new();
        metadata.insert("sdk_case_id", MetadataValue::String(row.case_id.clone()));
        if let Some(split) = row_case.split.clone() {
            metadata.insert("split", MetadataValue::String(split));
        }
        Ok(Assessment::Independent {
            candidate: self.candidate,
            target: AssessmentTarget::Case { set, case },
            evidence,
            cost: Cost::zero(),
            metadata,
        })
    }
}

fn decode_blob_content(content: &SdkPromptEffectBlobContent) -> Result<Vec<u8>, EvaluationError> {
    let bytes = general_purpose::STANDARD
        .decode(&content.content_base64)
        .map_err(|source| EvaluationError::with_source("decode SDK effect blob", source))?;
    if let Some(declared) = content.blob_ref.bytes {
        let actual = u64::try_from(bytes.len()).map_err(|source| {
            EvaluationError::with_source("measure SDK effect blob bytes", source)
        })?;
        if declared != actual {
            return Err(EvaluationError::Message(format!(
                "SDK effect blob `{}` byte count {declared} does not match decoded bytes {actual}",
                content.blob_ref.blob_id
            )));
        }
    }
    if let Some(expected_sha) = &content.blob_ref.sha256 {
        let actual_sha = format!("{:x}", sha2::Sha256::digest(&bytes));
        if expected_sha != &actual_sha {
            return Err(EvaluationError::Message(format!(
                "SDK effect blob `{}` sha256 does not match decoded bytes",
                content.blob_ref.blob_id
            )));
        }
    }
    Ok(bytes)
}

fn output_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

impl SdkPromptRunRecord {
    fn validate(&self) -> Result<(), SdkPromptCheckpointError> {
        if self.schema_version != SDK_PROMPT_RUN_RECORD_SCHEMA {
            return Err(SdkPromptCheckpointError::Schema {
                expected: SDK_PROMPT_RUN_RECORD_SCHEMA,
                actual: self.schema_version.clone(),
            });
        }
        if self.cases.len() != self.assessments.len() {
            return Err(SdkPromptCheckpointError::CaseAssessmentCount {
                cases: self.cases.len(),
                assessments: self.assessments.len(),
            });
        }
        for (case, assessment) in self.cases.iter().zip(self.assessments.iter()) {
            if case.case_id != assessment.case_id {
                return Err(SdkPromptCheckpointError::CaseAssessmentMismatch {
                    case_id: case.case_id.clone(),
                    assessment_case_id: assessment.case_id.clone(),
                });
            }
        }
        Ok(())
    }

    fn case_rows(&self) -> Vec<Case<Value, Value>> {
        self.cases
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let mut metadata = MetadataBag::new();
                metadata.insert("sdk_case_id", MetadataValue::String(row.case_id.clone()));
                if let Some(split) = row.split.clone() {
                    metadata.insert("split", MetadataValue::String(split));
                }
                Case::new(
                    CaseId::from_index(index),
                    row.input.clone(),
                    row.target.clone(),
                )
                .with_metadata(metadata)
            })
            .collect()
    }
}

fn run_uuid(run_id: &str) -> Uuid {
    let digest = ContentId::hash_bytes(run_id.as_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    Uuid::from_bytes(bytes)
}

/// Errors from SDK prompt checkpoint materialization.
#[derive(Debug, Error)]
pub enum SdkPromptCheckpointError {
    /// Input schema mismatch.
    #[error("expected {expected}, got {actual}")]
    Schema {
        /// Expected schema name.
        expected: &'static str,
        /// Actual schema name.
        actual: String,
    },
    /// Case and assessment row counts differ.
    #[error("case count {cases} does not match assessment count {assessments}")]
    CaseAssessmentCount {
        /// Case row count.
        cases: usize,
        /// Assessment row count.
        assessments: usize,
    },
    /// Case and assessment row order differs.
    #[error("case row {case_id} is paired with assessment row {assessment_case_id}")]
    CaseAssessmentMismatch {
        /// Case row id.
        case_id: String,
        /// Assessment row id.
        assessment_case_id: String,
    },
    /// RunContext rejected the materialization.
    #[error(transparent)]
    RunContext(#[from] leaven_engine::RunContextError),
    /// Engine optimizer lifecycle rejected the materialization.
    #[error(transparent)]
    Optimizer(#[from] leaven_engine::OptimizerError),
    /// Persistence rejected optimizer state.
    #[error(transparent)]
    Persistence(#[from] leaven_engine::RunPersistenceError),
    /// Store rejected the materialization.
    #[error(transparent)]
    Store(#[from] leaven_store::StoreError),
}
