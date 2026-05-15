use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    path::Path,
    time::Duration,
};

use leaven::core::InfoRef;
use leaven::engine::RunContext;
use leaven::eval::{Case, SplitRole};
use leaven::extend::PartitionId;
use leaven::gepa::{Gepa, ReflectionError, ReflectiveDatasetBuilder, ReflectiveExample};
use leaven::kernel::{AssessmentId, CandidateId, CaseId, FingerprintBuilder, MetadataValue};
use leaven::plumbing::{ContentId, Fingerprint, FiniteF64, MetadataBag};
use leaven::prelude::{
    Artifact, ArtifactIdentity, Budget, EditSurface, Optimized, Part, PartAddress, RunOutput,
    Score, ScoreContext, ScoreError, SurfaceError, SurfaceFingerprint,
};
use leaven::run::{RunCase, RunProblem};
use leaven::{kernel::Metered, stdlib::populations::ParetoFrontier};
use leaven_gepa::LmBackedReflectorConfig;
use leaven_lm::{
    Lm, LmError, LmId, LmRequest, LmResponse, Message, Messages, ReasoningEffort, SamplingOptions,
    TokenUsage,
};
use leaven_lm_cache::{CachedLm, InMemoryLmCache, LmCachePolicy};
use leaven_lm_openai::{OpenAiConfig, OpenAiLm, OpenAiThrottlePolicy};
use serde::{Deserialize, Serialize};

const BASELINE: &str = "Solve the math problem carefully. Break down the steps and provide the final answer as a single number.";
const OPTIMIZED: &str = "Solve with modular arithmetic when useful. Verify arithmetic before the final answer. Provide only the final integer.";
const GEPA_AIME_METRIC_CALLS: u64 = 500;
const GEPA_AIME_MAX_WORKERS: usize = 32;
const GEPA_AIME_MAX_OUTPUT_TOKENS: u32 = 32_000;
// GEPA AIME is controlled by max_metric_calls, not max_iterations. This is a
// Leaven-local safety ceiling; the public metric-call budget is the stop control.
const GEPA_AIME_INTERNAL_ITERATION_CEILING: usize = 500;
const GEPA_AIME_SOLVER_MODEL: &str = "gpt-4.1-mini";
const GEPA_AIME_REFLECTION_MODEL: &str = "gpt-5.4-mini";
const LEAVEN_AIME_SOLVER_CACHE_POLICY: &str = "LEAVEN_AIME_SOLVER_CACHE_POLICY";
const LEAVEN_AIME_REFLECTION_CACHE_POLICY: &str = "LEAVEN_AIME_REFLECTION_CACHE_POLICY";
const LEAVEN_AIME_LM_CACHE_BACKEND: &str = "LEAVEN_AIME_LM_CACHE_BACKEND";
const LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS: &str = "LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS";
const DETERMINISTIC_SMOKE_METRIC_CALLS: u64 = 512;
const DETERMINISTIC_SMOKE_ITERATIONS: usize = 1;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config = AimeRunConfig::configured();
    let result = run_configured_aime(config.clone()).await;
    for line in report_lines(&config, &result) {
        println!("{line}");
    }
}

fn report_lines(config: &AimeRunConfig, run: &AimeRunResult) -> Vec<String> {
    let result = &run.optimized;
    let mut lines = vec![
        format!("run_profile={}", config.profile.label()),
        format!(
            "baseline_train_score={}",
            report_score(result.summary.baseline_train_score)
        ),
        format!(
            "optimized_train_score={}",
            report_score(result.summary.optimized_train_score)
        ),
        format!(
            "validation_score={}",
            report_score(result.summary.validation_score)
        ),
        format!(
            "baseline_heldout_test_score={}",
            report_score(result.summary.baseline_test_score)
        ),
        format!(
            "heldout_test_score={}",
            report_score(result.summary.test_score)
        ),
        "test_score_use=final_report_only".to_owned(),
        format!(
            "report_splits={}",
            result.summary.evaluation.splits_reported.len()
        ),
        format!("solver_model={}", config.solver.model),
        format!("reflection_model={}", config.reflection.model),
        format!(
            "solver_cache_policy={}",
            report_lm_cache_policy(config.solver.cache_policy)
        ),
        format!(
            "reflection_cache_policy={}",
            report_lm_cache_policy(config.reflection.cache_policy)
        ),
        format!(
            "lm_cache_backend={}",
            report_lm_cache_backend(config.solver.runtime.cache_backend)
        ),
        format!(
            "lm_cache_durable={}",
            config.solver.runtime.cache_backend.is_durable()
        ),
        format!(
            "openai_max_concurrent_requests={}",
            config.solver.runtime.max_concurrent_requests
        ),
        "reflection_output=text".to_owned(),
        "reflection_parser=plain-text-fenced".to_owned(),
        format!("stop_reason={}", report_stop_reason(result.stop)),
        format!(
            "optimization_metric_calls={}",
            result.summary.optimization_cost.metric_calls
        ),
        format!(
            "final_report_metric_calls={}",
            result.summary.final_report_cost.metric_calls
        ),
        format!("budget_metric_calls={}", result.budget.spent.metric_calls),
        format!("budget_llm_calls={}", result.budget.spent.llm_calls),
        format!(
            "best_system_prompt={}",
            result.best().expect("AIME run has best prompt").system
        ),
        format!(
            "events={}",
            result
                .events
                .iter()
                .map(|event| event.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ),
    ];
    for split in &result.summary.evaluation.splits_reported {
        for candidate in &split.candidates {
            for case in &candidate.cases {
                let source_id = run
                    .report_metadata
                    .get(&case.case_id)
                    .map(AimeReportMetadata::source_id)
                    .unwrap_or_else(|| "missing-source-id".to_owned());
                lines.push(format!(
                    "report_case={} source_id={} split={:?} score={:.3} output_chars={} feedback_chars={}",
                    case.case_id,
                    source_id,
                    split.role,
                    case.score,
                    case.output.len(),
                    case.feedback.len()
                ));
            }
        }
    }
    lines
}

fn report_score(score: Option<f64>) -> String {
    score.map_or_else(|| "absent".to_owned(), |value| format!("{value:.3}"))
}

fn report_lm_cache_policy(policy: LmCachePolicy) -> &'static str {
    match policy {
        LmCachePolicy::Never => "never",
        LmCachePolicy::ReadWrite => "read-write",
        LmCachePolicy::ReadOnly => "read-only",
        LmCachePolicy::Refresh => "refresh",
    }
}

fn report_lm_cache_backend(backend: AimeLmCacheBackend) -> &'static str {
    match backend {
        AimeLmCacheBackend::InMemory => "in-memory",
    }
}

fn report_stop_reason(reason: leaven::run::OptimizationStopReason) -> &'static str {
    match reason {
        leaven::run::OptimizationStopReason::OptimizerDone => "optimizer_done",
        leaven::run::OptimizationStopReason::BudgetReached => "budget_reached",
        leaven::run::OptimizationStopReason::BudgetExceeded => "budget_exceeded",
        leaven::run::OptimizationStopReason::StopperTriggered => "stopper_triggered",
        leaven::run::OptimizationStopReason::External => "external",
        leaven::run::OptimizationStopReason::Error => "error",
    }
}

#[cfg(test)]
async fn run_deterministic_aime() -> AimeRunResult {
    let config = AimeRunConfig::deterministic_smoke();
    let dataset = deterministic_dataset();
    run_aime(config, dataset).await
}

async fn run_configured_aime(config: AimeRunConfig) -> AimeRunResult {
    let dataset = configured_dataset();
    run_aime(config, dataset).await
}

async fn run_aime(config: AimeRunConfig, dataset: AimeDataset) -> AimeRunResult {
    let solver = aime_solver_lm(&config.solver);
    let runner_fingerprint = aime_runner_fingerprint(&config.solver);
    let scorer_fingerprint = aime_scorer_fingerprint();
    let solver_config = config.solver.clone();
    let report_metadata = dataset.report_metadata.clone();
    let reflective_dataset = dataset.reflective_dataset();
    let optimized = leaven::prelude::optimize(AimePrompt::new(config.seed_prompt))
        .train(dataset.train)
        .validation(dataset.validation)
        .test(dataset.test)
        .runner(move |prompt, case| {
            let solver = solver.clone();
            let solver_config = solver_config.clone();
            async move { run_solver(prompt, case, solver, solver_config).await }
        })
        .score(score_answer)
        .runner_fingerprint(runner_fingerprint)
        .scorer_fingerprint(scorer_fingerprint)
        .evaluation_parallelism(config.evaluation_parallelism)
        .using(
            Gepa::reflect_with_lm(
                aime_reflection_lm(&config.reflection),
                config.reflection.model.clone(),
            )
            .with_reflector_config(aime_reflector_config(&config.reflection))
            .surface(AimePromptSurface)
            .population(
                ParetoFrontier::by_case()
                    .partition_filter(std::collections::BTreeSet::from([PartitionId::from(
                        "TRAIN",
                    )]))
                    .build(),
            )
            .reflective_dataset(reflective_dataset)
            .max_iterations(config.max_iterations),
        )
        .budget(config.budget)
        .run()
        .await
        .expect("AIME GEPA run succeeds");
    AimeRunResult {
        optimized,
        report_metadata,
    }
}

#[derive(Clone, Debug)]
struct AimeRunConfig {
    profile: AimeRunProfile,
    seed_prompt: &'static str,
    budget: Budget,
    evaluation_parallelism: NonZeroUsize,
    max_iterations: usize,
    solver: AimeSolverConfig,
    reflection: AimeReflectionConfig,
}

impl AimeRunConfig {
    fn configured() -> Self {
        if std::env::var_os("LEAVEN_AIME_LIVE_OPENAI").is_some() {
            Self::gepa_aime()
        } else {
            Self::deterministic_smoke()
        }
    }

    fn gepa_aime() -> Self {
        let cache_policies = AimeLmCachePolicies::from_env();
        let runtime = AimeOpenAiRuntimeConfig::from_env();
        Self {
            profile: AimeRunProfile::GepaAime,
            seed_prompt: BASELINE,
            budget: Budget::metric_calls(GEPA_AIME_METRIC_CALLS),
            evaluation_parallelism: NonZeroUsize::new(GEPA_AIME_MAX_WORKERS)
                .expect("GEPA AIME worker count is non-zero"),
            max_iterations: GEPA_AIME_INTERNAL_ITERATION_CEILING,
            solver: AimeSolverConfig {
                live: true,
                model: openai_model_name(),
                sampling: gepa_aime_sampling(),
                cache_policy: cache_policies.solver,
                runtime,
            },
            reflection: AimeReflectionConfig {
                live: std::env::var_os("LEAVEN_AIME_LIVE_OPENAI_REFLECTION").is_some(),
                model: aime_reflection_model_name(),
                sampling: SamplingOptions::default().with_reasoning_effort(ReasoningEffort::Medium),
                cache_policy: cache_policies.reflection,
                runtime,
            },
        }
    }

    fn deterministic_smoke() -> Self {
        Self {
            profile: AimeRunProfile::DeterministicSmoke,
            seed_prompt: BASELINE,
            budget: Budget::metric_calls(DETERMINISTIC_SMOKE_METRIC_CALLS),
            evaluation_parallelism: NonZeroUsize::new(1).expect("smoke worker count is non-zero"),
            max_iterations: DETERMINISTIC_SMOKE_ITERATIONS,
            solver: AimeSolverConfig {
                live: false,
                model: openai_model_name(),
                sampling: SamplingOptions::default(),
                cache_policy: LmCachePolicy::Never,
                runtime: AimeOpenAiRuntimeConfig::default_for_p8(),
            },
            reflection: AimeReflectionConfig {
                live: false,
                model: "deterministic-aime-reflector".to_owned(),
                sampling: SamplingOptions::default().with_reasoning_effort(ReasoningEffort::Medium),
                cache_policy: LmCachePolicy::Never,
                runtime: AimeOpenAiRuntimeConfig::default_for_p8(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AimeRunProfile {
    DeterministicSmoke,
    GepaAime,
}

impl AimeRunProfile {
    const fn label(self) -> &'static str {
        match self {
            Self::DeterministicSmoke => "deterministic-smoke",
            Self::GepaAime => "gepa-aime",
        }
    }
}

#[derive(Clone, Debug)]
struct AimeSolverConfig {
    live: bool,
    model: String,
    sampling: SamplingOptions,
    cache_policy: LmCachePolicy,
    runtime: AimeOpenAiRuntimeConfig,
}

#[derive(Clone, Debug)]
struct AimeReflectionConfig {
    live: bool,
    model: String,
    sampling: SamplingOptions,
    cache_policy: LmCachePolicy,
    runtime: AimeOpenAiRuntimeConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AimeLmCachePolicies {
    solver: LmCachePolicy,
    reflection: LmCachePolicy,
}

impl AimeLmCachePolicies {
    fn from_env() -> Self {
        let solver = std::env::var(LEAVEN_AIME_SOLVER_CACHE_POLICY).ok();
        let reflection = std::env::var(LEAVEN_AIME_REFLECTION_CACHE_POLICY).ok();
        Self::from_values(solver.as_deref(), reflection.as_deref())
    }

    fn from_values(solver: Option<&str>, reflection: Option<&str>) -> Self {
        Self {
            solver: parse_lm_cache_policy(LEAVEN_AIME_SOLVER_CACHE_POLICY, solver),
            reflection: parse_lm_cache_policy(LEAVEN_AIME_REFLECTION_CACHE_POLICY, reflection),
        }
    }
}

fn parse_lm_cache_policy(env_name: &str, value: Option<&str>) -> LmCachePolicy {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return LmCachePolicy::Never;
    };
    match raw.to_ascii_lowercase().as_str() {
        "never" | "none" | "off" => LmCachePolicy::Never,
        "read-write" | "read_write" | "readwrite" => LmCachePolicy::ReadWrite,
        "read-only" | "read_only" | "readonly" => LmCachePolicy::ReadOnly,
        "refresh" => LmCachePolicy::Refresh,
        _ => panic!(
            "unsupported {env_name}={raw:?}; expected never, read-write, read-only, or refresh"
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AimeOpenAiRuntimeConfig {
    max_concurrent_requests: NonZeroUsize,
    cache_backend: AimeLmCacheBackend,
}

impl AimeOpenAiRuntimeConfig {
    fn from_env() -> Self {
        let max_concurrent = std::env::var(LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS).ok();
        let cache_backend = std::env::var(LEAVEN_AIME_LM_CACHE_BACKEND).ok();
        Self::from_values(max_concurrent.as_deref(), cache_backend.as_deref())
    }

    fn from_values(max_concurrent: Option<&str>, cache_backend: Option<&str>) -> Self {
        Self {
            max_concurrent_requests: parse_max_concurrent_requests(max_concurrent),
            cache_backend: parse_lm_cache_backend(cache_backend),
        }
    }

    fn default_for_p8() -> Self {
        Self {
            max_concurrent_requests: NonZeroUsize::new(GEPA_AIME_MAX_WORKERS)
                .expect("GEPA AIME worker count is non-zero"),
            cache_backend: AimeLmCacheBackend::InMemory,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AimeLmCacheBackend {
    InMemory,
}

impl AimeLmCacheBackend {
    const fn is_durable(self) -> bool {
        match self {
            Self::InMemory => false,
        }
    }
}

fn parse_max_concurrent_requests(value: Option<&str>) -> NonZeroUsize {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return NonZeroUsize::new(GEPA_AIME_MAX_WORKERS)
            .expect("GEPA AIME worker count is non-zero");
    };
    let parsed = raw.parse::<usize>().unwrap_or_else(|source| {
        panic!("unsupported {LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS}={raw:?}: {source}")
    });
    NonZeroUsize::new(parsed).unwrap_or_else(|| {
        panic!("unsupported {LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS}=0; expected a positive integer")
    })
}

fn parse_lm_cache_backend(value: Option<&str>) -> AimeLmCacheBackend {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return AimeLmCacheBackend::InMemory;
    };
    match raw.to_ascii_lowercase().as_str() {
        "in-memory" | "in_memory" | "memory" => AimeLmCacheBackend::InMemory,
        _ => panic!(
            "unsupported {LEAVEN_AIME_LM_CACHE_BACKEND}={raw:?}; only in-memory is implemented in this branch"
        ),
    }
}

fn gepa_aime_sampling() -> SamplingOptions {
    SamplingOptions {
        temperature: Some(FiniteF64::new(1.0).expect("temperature is finite")),
        max_output_tokens: Some(GEPA_AIME_MAX_OUTPUT_TOKENS),
        ..SamplingOptions::default()
    }
}

fn aime_reflection_lm(config: &AimeReflectionConfig) -> AimeReflectionLm {
    if config.live {
        AimeReflectionLm::OpenAi(cached_openai_lm(
            config.cache_policy,
            config.runtime,
            "live reflection",
        ))
    } else {
        AimeReflectionLm::Deterministic(DeterministicReflectionLm)
    }
}

fn aime_reflector_config(config: &AimeReflectionConfig) -> LmBackedReflectorConfig {
    LmBackedReflectorConfig {
        sampling: config.sampling.clone(),
        output: leaven_lm::OutputMode::Text,
        prompt_template: None,
    }
}

fn aime_reflection_model_name() -> String {
    std::env::var("LEAVEN_AIME_REFLECTION_MODEL")
        .unwrap_or_else(|_| GEPA_AIME_REFLECTION_MODEL.to_owned())
}

#[derive(Clone)]
enum AimeReflectionLm {
    Deterministic(DeterministicReflectionLm),
    OpenAi(AimeOpenAiLm),
}

impl std::fmt::Debug for AimeReflectionLm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deterministic(inner) => f.debug_tuple("Deterministic").field(inner).finish(),
            Self::OpenAi(_) => f.write_str("OpenAi"),
        }
    }
}

impl Lm for AimeReflectionLm {
    fn id(&self) -> LmId {
        match self {
            Self::Deterministic(inner) => inner.id(),
            Self::OpenAi(inner) => inner.id(),
        }
    }

    fn fingerprint(&self) -> Fingerprint {
        match self {
            Self::Deterministic(inner) => inner.fingerprint(),
            Self::OpenAi(inner) => inner.fingerprint(),
        }
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        match self {
            Self::Deterministic(inner) => inner.complete(request).await,
            Self::OpenAi(inner) => inner.complete(request).await,
        }
    }
}

#[derive(Clone, Debug)]
struct DeterministicReflectionLm;

impl Lm for DeterministicReflectionLm {
    fn id(&self) -> LmId {
        LmId::from("deterministic-aime-reflection")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([10; 32])
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        let prompt = request
            .messages
            .iter()
            .map(Message::content)
            .collect::<Vec<_>>()
            .join("\n");
        let system = if prompt.contains("incorrect") && prompt.contains("mod") {
            OPTIMIZED
        } else {
            BASELINE
        };
        let content = format!("```\n{system}\n```");
        let usage = TokenUsage {
            input_tokens: 37,
            cached_input_tokens: 0,
            output_tokens: 11,
            reasoning_tokens: 0,
        };
        let response =
            LmResponse::new(Message::assistant(content), usage.clone()).map_err(|source| {
                LmError::invalid_response("deterministic-aime-reflection", source.to_string())
            })?;
        Ok(Metered::new(response, usage.to_cost()))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AimePrompt {
    system: String,
}

impl AimePrompt {
    fn new(system: impl Into<String>) -> Self {
        Self {
            system: system.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AimePromptChange {
    system: String,
}

#[derive(Debug)]
struct AimePromptError;

impl std::fmt::Display for AimePromptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AIME prompt change was invalid")
    }
}

impl std::error::Error for AimePromptError {}

impl Artifact for AimePrompt {
    type Change = AimePromptChange;
    type ApplyError = AimePromptError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(content_id(self.system.as_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self {
            system: change.system.clone(),
        })
    }
}

#[derive(Clone, Debug)]
struct AimePromptSurface;

impl EditSurface<AimePrompt> for AimePromptSurface {
    type PartId = &'static str;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = String;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(Fingerprint::from_bytes([8; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a AimePrompt,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(vec![Part {
            id: "system",
            address: PartAddress("system".to_owned()),
            view: artifact.system.as_str(),
        }])
    }

    fn change_part(
        &self,
        _artifact: &AimePrompt,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<AimePromptChange, SurfaceError> {
        if id != "system" {
            return Err(SurfaceError::UnknownPart);
        }
        Ok(AimePromptChange { system: edit })
    }
}

type AimeRunCase = Case<AimeInput, AimeTarget>;

#[derive(Clone, Debug)]
struct AimeRunResult {
    optimized: Optimized<AimePrompt>,
    report_metadata: BTreeMap<CaseId, AimeReportMetadata>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AimeImportRecord {
    source_id: String,
    problem: String,
    answer: i64,
    solution: String,
    needs_modular: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AimeInput {
    problem: String,
}

impl std::fmt::Display for AimeInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.problem)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AimeTarget {
    answer: AimeAnswer,
    solution: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AimeAnswer {
    integer: i64,
    raw: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AimeSource {
    dataset: String,
    config: String,
    split: String,
    row_id: String,
    revision: Option<String>,
}

impl AimeSource {
    fn parse(source_id: &str) -> Result<Self, AimeDatasetError> {
        let (body, revision) = source_id
            .rsplit_once('@')
            .map_or((source_id, None), |(body, revision)| {
                (body, Some(revision.to_owned()))
            });
        let parts = body.split(':').collect::<Vec<_>>();
        if parts.len() != 4 || parts.iter().any(|part| part.is_empty()) {
            return Err(AimeDatasetError::InvalidSourceId {
                source_id: source_id.to_owned(),
            });
        }
        Ok(Self {
            dataset: parts[0].to_owned(),
            config: parts[1].to_owned(),
            split: parts[2].to_owned(),
            row_id: parts[3].to_owned(),
            revision,
        })
    }

    fn canonical_id(&self) -> String {
        let mut source_id = format!(
            "{}:{}:{}:{}",
            self.dataset, self.config, self.split, self.row_id
        );
        if let Some(revision) = &self.revision {
            source_id.push('@');
            source_id.push_str(revision);
        }
        source_id
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AimeTags {
    needs_modular: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeReportMetadata {
    source: AimeSource,
    split: SplitRole,
    tags: AimeTags,
}

impl AimeReportMetadata {
    fn source_id(&self) -> String {
        self.source.canonical_id()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AimeDatasetCache {
    train: Vec<AimeImportRecord>,
    validation: Vec<AimeImportRecord>,
    test: Vec<AimeImportRecord>,
}

#[derive(Clone, Debug)]
struct AimeDataset {
    train: Vec<AimeRunCase>,
    validation: Vec<AimeRunCase>,
    test: Vec<AimeRunCase>,
    report_metadata: BTreeMap<CaseId, AimeReportMetadata>,
}

impl AimeDataset {
    fn from_cache(cache: AimeDatasetCache) -> Result<Self, AimeDatasetError> {
        let mut lowerer = AimeDatasetLowerer::default();
        let train = lowerer.lower_split(SplitRole::Train, cache.train)?;
        let validation = lowerer.lower_split(SplitRole::Validation, cache.validation)?;
        let test = lowerer.lower_split(SplitRole::Test, cache.test)?;
        Ok(Self {
            train,
            validation,
            test,
            report_metadata: lowerer.report_metadata,
        })
    }

    fn reflective_dataset(&self) -> AimeReflectiveDataset {
        AimeReflectiveDataset {
            inputs_by_case: self
                .train
                .iter()
                .chain(&self.validation)
                .chain(&self.test)
                .map(|case| (case.id, case.input.problem.clone()))
                .collect(),
        }
    }
}

#[derive(Default)]
struct AimeDatasetLowerer {
    seen_sources: BTreeSet<String>,
    seen_case_ids: BTreeMap<CaseId, String>,
    report_metadata: BTreeMap<CaseId, AimeReportMetadata>,
}

impl AimeDatasetLowerer {
    fn lower_split(
        &mut self,
        split: SplitRole,
        records: Vec<AimeImportRecord>,
    ) -> Result<Vec<AimeRunCase>, AimeDatasetError> {
        records
            .into_iter()
            .map(|record| self.lower_record(split.clone(), record))
            .collect()
    }

    fn lower_record(
        &mut self,
        split: SplitRole,
        record: AimeImportRecord,
    ) -> Result<AimeRunCase, AimeDatasetError> {
        let source = AimeSource::parse(&record.source_id)?;
        let source_id = source.canonical_id();
        if !self.seen_sources.insert(source_id.clone()) {
            return Err(AimeDatasetError::DuplicateSourceId { source_id });
        }
        let case_id = case_id_from_source_id(&source_id);
        if let Some(existing_source_id) = self.seen_case_ids.insert(case_id, source_id.clone()) {
            return Err(AimeDatasetError::CaseIdCollision {
                case_id,
                existing_source_id,
                incoming_source_id: source_id,
            });
        }

        let tags = AimeTags {
            needs_modular: record.needs_modular,
        };
        let metadata = AimeReportMetadata {
            source,
            split,
            tags,
        };
        self.report_metadata.insert(case_id, metadata.clone());

        Ok(Case::targeted(
            case_id,
            AimeInput {
                problem: record.problem,
            },
            AimeTarget {
                answer: AimeAnswer {
                    integer: record.answer,
                    raw: record.answer.to_string(),
                },
                solution: record.solution,
            },
        )
        .with_metadata(aime_metadata(&metadata)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AimeDatasetError {
    InvalidSourceId {
        source_id: String,
    },
    DuplicateSourceId {
        source_id: String,
    },
    CaseIdCollision {
        case_id: CaseId,
        existing_source_id: String,
        incoming_source_id: String,
    },
}

impl std::fmt::Display for AimeDatasetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSourceId { source_id } => {
                write!(
                    f,
                    "invalid AIME source_id {source_id:?}; expected dataset:config:split:row_id[@revision]"
                )
            }
            Self::DuplicateSourceId { source_id } => {
                write!(f, "duplicate AIME source_id {source_id:?}")
            }
            Self::CaseIdCollision {
                case_id,
                existing_source_id,
                incoming_source_id,
            } => write!(
                f,
                "AIME source ids {existing_source_id:?} and {incoming_source_id:?} collide at {case_id}"
            ),
        }
    }
}

impl std::error::Error for AimeDatasetError {}

fn configured_dataset() -> AimeDataset {
    match std::env::var("LEAVEN_AIME_CACHE") {
        Ok(path) => dataset_from_cache(Path::new(&path)),
        Err(_) => deterministic_dataset(),
    }
}

fn dataset_from_cache(path: &Path) -> AimeDataset {
    let bytes = std::fs::read(path).unwrap_or_else(|source| {
        panic!(
            "failed to read LEAVEN_AIME_CACHE={}: {source}",
            path.display()
        )
    });
    let cache: AimeDatasetCache = serde_json::from_slice(&bytes).unwrap_or_else(|source| {
        panic!(
            "failed to parse LEAVEN_AIME_CACHE={}: {source}",
            path.display()
        )
    });
    AimeDataset::from_cache(cache).unwrap_or_else(|source| {
        panic!(
            "failed to lower LEAVEN_AIME_CACHE={}: {source}",
            path.display()
        )
    })
}

fn deterministic_dataset() -> AimeDataset {
    let train = vec![
        AimeImportRecord {
            source_id: "deterministic:default:train:0".to_owned(),
            problem: "Find the remainder when 2^10 is divided by 7.".to_owned(),
            answer: 2,
            solution: "2^3 = 8 == 1 mod 7, so 2^10 == 2 mod 7.".to_owned(),
            needs_modular: true,
        },
        AimeImportRecord {
            source_id: "deterministic:default:train:1".to_owned(),
            problem: "What is 19 + 23?".to_owned(),
            answer: 42,
            solution: "19 + 23 = 42.".to_owned(),
            needs_modular: false,
        },
        AimeImportRecord {
            source_id: "deterministic:default:train:2".to_owned(),
            problem: "Find the remainder when 5^4 is divided by 13.".to_owned(),
            answer: 1,
            solution: "5^2 = 25 == -1 mod 13, so 5^4 == 1.".to_owned(),
            needs_modular: true,
        },
    ];
    let validation = vec![AimeImportRecord {
        source_id: "deterministic:default:validation:0".to_owned(),
        problem: "Find the remainder when 3^6 is divided by 7.".to_owned(),
        answer: 1,
        solution: "3^6 = 729 == 1 mod 7.".to_owned(),
        needs_modular: true,
    }];
    let test = vec![
        AimeImportRecord {
            source_id: "deterministic:default:test:0".to_owned(),
            problem: "Find the remainder when 4^5 is divided by 9.".to_owned(),
            answer: 7,
            solution: "4^3 == 1 mod 9, so 4^5 == 4^2 == 7.".to_owned(),
            needs_modular: true,
        },
        AimeImportRecord {
            source_id: "deterministic:default:test:1".to_owned(),
            problem: "What is 31 - 8?".to_owned(),
            answer: 23,
            solution: "31 - 8 = 23.".to_owned(),
            needs_modular: false,
        },
    ];
    AimeDataset::from_cache(AimeDatasetCache {
        train,
        validation,
        test,
    })
    .expect("deterministic AIME fixture lowers")
}

fn aime_metadata(metadata: &AimeReportMetadata) -> MetadataBag {
    let mut bag = MetadataBag::new();
    bag.insert("source_id", MetadataValue::String(metadata.source_id()));
    bag.insert(
        "source",
        MetadataValue::Json(
            serde_json::to_value(&metadata.source).expect("AIME source metadata serializes"),
        ),
    );
    bag.insert(
        "split_role",
        MetadataValue::String(split_role_label(&metadata.split).to_owned()),
    );
    bag.insert(
        "needs_modular",
        MetadataValue::Bool(metadata.tags.needs_modular),
    );
    bag
}

fn split_role_label(role: &SplitRole) -> &str {
    match role {
        SplitRole::Train => "train",
        SplitRole::Validation => "validation",
        SplitRole::Test => "test",
        SplitRole::Search => "search",
        SplitRole::Probe => "probe",
        SplitRole::ReportOnly => "report-only",
        SplitRole::Custom(_) => "custom",
    }
}

fn case_id_from_source_id(source_id: &str) -> CaseId {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"leaven:aime-case:v1");
    builder.update(source_id.as_bytes());
    let fingerprint = builder.finish();
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&fingerprint.0[..8]);
    CaseId::new(u64::from_le_bytes(bytes))
}

#[derive(Clone, Debug)]
struct AimeReflectiveDataset {
    inputs_by_case: BTreeMap<CaseId, String>,
}

impl ReflectiveDatasetBuilder<RunProblem<AimePrompt, AimeInput, AimeTarget>, AimePromptSurface>
    for AimeReflectiveDataset
{
    async fn build(
        &self,
        ctx: &mut RunContext<'_, RunProblem<AimePrompt, AimeInput, AimeTarget>>,
        _parent: CandidateId,
        parent_assessment: AssessmentId,
        _part: &&'static str,
    ) -> Result<Vec<ReflectiveExample>, ReflectionError> {
        let evidence = ctx.assessment_evidence(parent_assessment)?;
        Ok(evidence
            .outcomes()
            .iter()
            .map(|outcome| {
                let case = outcome.case();
                ReflectiveExample {
                    case: Some(case),
                    input: self.inputs_by_case.get(&case).cloned().unwrap_or_default(),
                    output: Some(format!("{:?}", outcome.evidence().output())),
                    score: Some(outcome.evidence().score().score()),
                    feedback: outcome.evidence().feedback().to_owned(),
                    source_refs: vec![InfoRef::Assessment(parent_assessment)],
                }
            })
            .collect())
    }
}

#[derive(Clone)]
struct AimeOpenAiLm {
    inner: CachedLm<OpenAiLm, InMemoryLmCache>,
}

impl std::fmt::Debug for AimeOpenAiLm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AimeOpenAiLm")
    }
}

impl Lm for AimeOpenAiLm {
    fn id(&self) -> LmId {
        self.inner.id()
    }

    fn fingerprint(&self) -> Fingerprint {
        self.inner.fingerprint()
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        self.inner.complete(request).await
    }
}

fn aime_solver_lm(config: &AimeSolverConfig) -> Option<AimeOpenAiLm> {
    if config.live {
        Some(cached_openai_lm(
            config.cache_policy,
            config.runtime,
            "live solver",
        ))
    } else {
        None
    }
}

fn cached_openai_lm(
    cache_policy: LmCachePolicy,
    runtime: AimeOpenAiRuntimeConfig,
    role: &str,
) -> AimeOpenAiLm {
    let config = OpenAiConfig::from_env()
        .unwrap_or_else(|source| panic!("OPENAI_API_KEY is required for {role}: {source}"))
        .with_throttle_policy(OpenAiThrottlePolicy::new(
            runtime.max_concurrent_requests,
            Duration::ZERO,
        ));
    let inner = OpenAiLm::new(config);
    match runtime.cache_backend {
        AimeLmCacheBackend::InMemory => AimeOpenAiLm {
            inner: CachedLm::new(inner, InMemoryLmCache::default(), cache_policy),
        },
    }
}

async fn run_solver(
    prompt: AimePrompt,
    case: RunCase<AimeInput>,
    solver: Option<AimeOpenAiLm>,
    solver_config: AimeSolverConfig,
) -> RunOutput {
    if let Some(solver) = solver {
        return run_openai_solver(solver, &prompt, &case, &solver_config).await;
    }
    let has_modular = prompt.system.contains("modular arithmetic");
    let verifies = prompt.system.contains("Verify arithmetic");
    let correct = (!input_needs_modular(case.input()) || has_modular) && verifies;
    let target_answer = deterministic_fixture_answer(case.input());
    let answer = if correct {
        target_answer
    } else {
        target_answer + 1
    };
    RunOutput::new(answer.to_string())
}

async fn run_openai_solver(
    solver: AimeOpenAiLm,
    prompt: &AimePrompt,
    case: &RunCase<AimeInput>,
    solver_config: &AimeSolverConfig,
) -> RunOutput {
    let request = LmRequest::new(
        solver_config.model.clone(),
        Messages::new()
            .with_system(prompt.system.clone())
            .with_user(format!(
                "Problem:\n{}\n\nReturn only the final numerical answer.",
                case.input().problem
            )),
    )
    .with_sampling(solver_config.sampling.clone());
    match solver.complete(request).await {
        Ok(metered) => {
            let answer = metered.value.assistant.content().trim().to_owned();
            RunOutput::new(answer).with_cost(metered.cost)
        }
        Err(_) => RunOutput::new(String::new()),
    }
}

fn openai_model_name() -> String {
    std::env::var("LEAVEN_OPENAI_MODEL").unwrap_or_else(|_| GEPA_AIME_SOLVER_MODEL.to_owned())
}

fn aime_runner_fingerprint(config: &AimeSolverConfig) -> Fingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"p8-aime-runner.v1");
    builder.update([u8::from(config.live)]);
    builder.update(config.model.as_bytes());
    builder.update(
        serde_json::to_vec(&config.sampling).expect("AIME solver sampling config serializes"),
    );
    builder.update(report_lm_cache_policy(config.cache_policy).as_bytes());
    builder.update(report_lm_cache_backend(config.runtime.cache_backend).as_bytes());
    builder.update(config.runtime.max_concurrent_requests.get().to_le_bytes());
    builder.finish()
}

fn aime_scorer_fingerprint() -> Fingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"p8-aime-scorer.exact-integer.v1");
    builder.update(b"target-answer-integer");
    builder.update(b"solution-feedback-visible-to-scorer");
    builder.finish()
}

async fn score_answer(
    ctx: ScoreContext<AimePrompt, AimeInput, AimeTarget>,
) -> Result<Score, ScoreError> {
    let target = ctx
        .case
        .target()
        .ok_or_else(|| ScoreError::new("AIME scorer requires a target answer"))?;
    let parsed = ctx.output.output.parse::<i64>();
    let score = match parsed {
        Ok(answer) if answer == target.answer.integer => {
            Score::new(1.0, format!("correct.{}", solution_feedback(target)))
        }
        Ok(answer) => Score::new(
            0.0,
            format!(
                "incorrect; got {answer}, expected {}.{}",
                target.answer.integer,
                solution_feedback(target)
            ),
        ),
        Err(_) => Score::new(
            0.0,
            format!(
                "final answer must parse as an integer; expected {}.{}",
                target.answer.integer,
                solution_feedback(target)
            ),
        ),
    };
    Ok(score)
}

fn input_needs_modular(input: &AimeInput) -> bool {
    input.problem.contains("remainder")
}

fn deterministic_fixture_answer(input: &AimeInput) -> i64 {
    match input.problem.as_str() {
        "Find the remainder when 2^10 is divided by 7." => 2,
        "What is 19 + 23?" => 42,
        "Find the remainder when 5^4 is divided by 13." => 1,
        "Find the remainder when 3^6 is divided by 7." => 1,
        "Find the remainder when 4^5 is divided by 9." => 7,
        "What is 31 - 8?" => 23,
        _ => 0,
    }
}

fn solution_feedback(target: &AimeTarget) -> String {
    format!(
        " Here's the full step-by-step solution:\n{}\n\nThink about what takeaways you can learn from this solution to improve your future answers and approach to similar problems.",
        target.solution
    )
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut material = BTreeMap::new();
    material.insert("prompt", bytes);
    let mut id = [0; ContentId::BYTES];
    for (index, byte) in material
        .values()
        .flat_map(|value| value.iter().copied())
        .enumerate()
    {
        id[index % ContentId::BYTES] ^= byte;
    }
    ContentId::from_bytes(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use leaven::kernel::CaseId;
    use leaven::prelude::RunEventSummary;

    fn assert_score(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < f64::EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_optional_score(actual: Option<f64>, expected: f64) {
        assert_score(actual.expect("score is present"), expected);
    }

    #[test]
    fn deterministic_fixture_lowers_to_target_safe_cases() {
        let dataset = deterministic_dataset();
        let first = &dataset.train[0];
        let first_target = first.target.as_ref().expect("AIME train case has target");
        let first_source = "deterministic:default:train:0";
        let first_id = case_id_from_source_id(first_source);

        assert_eq!(first.id, first_id);
        assert_eq!(
            first.input.problem,
            "Find the remainder when 2^10 is divided by 7."
        );
        assert_eq!(first_target.answer.integer, 2);
        assert_eq!(dataset.report_metadata[&first_id].source_id(), first_source);
        assert_eq!(dataset.report_metadata[&first_id].split, SplitRole::Train);
        assert!(dataset.report_metadata[&first_id].tags.needs_modular);
        assert_ne!(first.id, CaseId::from_index(0));
    }

    #[test]
    fn runner_case_type_does_not_name_target_or_metadata() {
        let runner_case_type = std::any::type_name::<RunCase<AimeInput>>();

        assert!(runner_case_type.contains("AimeInput"));
        assert!(!runner_case_type.contains("AimeTarget"));
        assert!(!runner_case_type.contains("AimeReportMetadata"));
    }

    #[test]
    fn duplicate_source_ids_refuse_before_running() {
        let duplicate = AimeDatasetCache {
            train: vec![AimeImportRecord {
                source_id: "deterministic:default:train:0".to_owned(),
                problem: "first".to_owned(),
                answer: 1,
                solution: "first solution".to_owned(),
                needs_modular: false,
            }],
            validation: vec![AimeImportRecord {
                source_id: "deterministic:default:train:0".to_owned(),
                problem: "second".to_owned(),
                answer: 2,
                solution: "second solution".to_owned(),
                needs_modular: true,
            }],
            test: Vec::new(),
        };

        assert_eq!(
            AimeDataset::from_cache(duplicate).unwrap_err(),
            AimeDatasetError::DuplicateSourceId {
                source_id: "deterministic:default:train:0".to_owned()
            }
        );
    }

    #[test]
    fn deterministic_aime_acceptance_shows_public_gepa_improvement() {
        let run = block_on(run_deterministic_aime());
        let result = &run.optimized;

        assert_optional_score(result.summary.baseline_train_score, 0.0);
        assert_optional_score(result.summary.optimized_train_score, 1.0);
        assert_optional_score(result.summary.baseline_validation_score, 0.0);
        assert_optional_score(result.summary.validation_score, 1.0);
        assert_optional_score(result.summary.baseline_test_score, 0.0);
        assert_optional_score(result.summary.test_score, 1.0);
        assert_eq!(result.summary.evaluation.splits_reported.len(), 3);
        assert!(result.budget.spent.metric_calls > 0);
        assert_eq!(result.budget.spent.llm_calls, 1);
        assert_eq!(result.budget.spent.prompt_tokens, 37);
        assert_eq!(result.budget.spent.completion_tokens, 11);
        assert!(
            result
                .summary
                .evaluation
                .splits_reported
                .iter()
                .flat_map(|split| &split.candidates)
                .flat_map(|candidate| &candidate.cases)
                .any(|case| !case.feedback.is_empty() && !case.output.is_empty())
        );
        assert!(
            result
                .summary
                .evaluation
                .splits_reported
                .iter()
                .flat_map(|split| &split.candidates)
                .flat_map(|candidate| &candidate.cases)
                .any(|case| case
                    .feedback
                    .contains("Here's the full step-by-step solution")),
            "scorer feedback should prove target/reference solution visibility"
        );
        assert!(
            result
                .best()
                .expect("AIME run has best prompt")
                .system
                .contains("modular arithmetic")
        );
        assert!(result.events.contains(&RunEventSummary::ProposalRecorded));
        assert!(
            result
                .events
                .contains(&RunEventSummary::EvaluationCompleted)
        );
        assert!(result.events.contains(&RunEventSummary::OptimizationEnded));
    }

    #[test]
    fn train_only_run_reports_absent_validation_and_test_scores() {
        let config = AimeRunConfig::deterministic_smoke();
        let mut dataset = deterministic_dataset();
        dataset.validation.clear();
        dataset.test.clear();
        let run = block_on(run_aime(config, dataset));
        let result = &run.optimized;

        assert_optional_score(result.summary.baseline_train_score, 0.0);
        assert_optional_score(result.summary.optimized_train_score, 1.0);
        assert_eq!(result.summary.baseline_validation_score, None);
        assert_eq!(result.summary.validation_score, None);
        assert_eq!(result.summary.baseline_test_score, None);
        assert_eq!(result.summary.test_score, None);
        assert!(!result.events.is_empty());
        assert_eq!(
            result.best(),
            Some(
                &result
                    .best
                    .as_ref()
                    .expect("train-only run has best")
                    .artifact
            )
        );
    }

    #[test]
    fn run_builder_requires_score_function() {
        let config = AimeRunConfig::deterministic_smoke();
        let solver_config = config.solver.clone();
        let dataset = deterministic_dataset();
        let reflective_dataset = dataset.reflective_dataset();
        let error = block_on(async {
            leaven::prelude::optimize(AimePrompt::new(config.seed_prompt))
                .train(dataset.train)
                .runner(move |prompt, case| {
                    let solver_config = solver_config.clone();
                    async move { run_solver(prompt, case, None, solver_config).await }
                })
                .using(
                    Gepa::reflect_with_lm(
                        aime_reflection_lm(&config.reflection),
                        config.reflection.model.clone(),
                    )
                    .with_reflector_config(aime_reflector_config(&config.reflection))
                    .surface(AimePromptSurface)
                    .build()
                    .reflective_dataset(reflective_dataset),
                )
                .budget(Budget::metric_calls(8))
                .run()
                .await
        })
        .unwrap_err();

        assert!(error.to_string().contains("score function is required"));
    }

    #[test]
    fn configured_gepa_aime_profile_matches_reference_knobs() {
        let config = AimeRunConfig::gepa_aime();

        assert_eq!(config.profile, AimeRunProfile::GepaAime);
        assert_eq!(config.seed_prompt, BASELINE);
        assert_eq!(config.budget.metric_calls, Some(GEPA_AIME_METRIC_CALLS));
        assert_eq!(config.evaluation_parallelism.get(), GEPA_AIME_MAX_WORKERS);
        assert_eq!(config.max_iterations, GEPA_AIME_INTERNAL_ITERATION_CEILING);
        assert!(config.solver.live);
        assert_eq!(config.solver.model, openai_model_name());
        assert_eq!(config.solver.cache_policy, LmCachePolicy::Never);
        assert_eq!(
            config.solver.sampling.temperature.map(FiniteF64::as_f64),
            Some(1.0)
        );
        assert_eq!(
            config.solver.sampling.max_output_tokens,
            Some(GEPA_AIME_MAX_OUTPUT_TOKENS)
        );
        assert_eq!(config.reflection.model, aime_reflection_model_name());
        assert_eq!(config.reflection.cache_policy, LmCachePolicy::Never);
        assert_eq!(
            config.reflection.sampling.reasoning_effort,
            Some(ReasoningEffort::Medium)
        );
    }

    #[test]
    fn report_lines_include_split_budget_and_case_identity() {
        let config = AimeRunConfig::deterministic_smoke();
        let result = block_on(run_deterministic_aime());
        let lines = report_lines(&config, &result);
        let validation_id = case_id_from_source_id("deterministic:default:validation:0");
        let test_id = case_id_from_source_id("deterministic:default:test:0");

        assert!(
            lines
                .iter()
                .any(|line| line == "optimization_metric_calls=6")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "final_report_metric_calls=12")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "test_score_use=final_report_only")
        );
        assert!(lines.iter().any(|line| {
            line.contains(&format!("report_case={validation_id}"))
                && line.contains("source_id=deterministic:default:validation:0")
                && line.contains("output_chars=")
                && line.contains("feedback_chars=")
        }));
        assert!(lines.iter().any(|line| {
            line.contains(&format!("report_case={test_id}"))
                && line.contains("source_id=deterministic:default:test:0")
                && line.contains("output_chars=")
                && line.contains("feedback_chars=")
        }));
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("step-by-step solution"))
        );
    }

    #[test]
    fn deterministic_metric_call_budget_stops_gepa_cleanly_before_second_step() {
        let mut config = AimeRunConfig::deterministic_smoke();
        config.budget = Budget::metric_calls(6);
        config.max_iterations = 2;
        let run = block_on(run_aime(config.clone(), deterministic_dataset()));
        let result = &run.optimized;

        assert_eq!(
            result.stop,
            leaven::run::OptimizationStopReason::BudgetReached
        );
        assert_eq!(result.summary.optimization_cost.metric_calls, 6);
        assert!(
            result
                .events
                .contains(&RunEventSummary::OptimizationStopping)
        );
        assert!(
            !result.events.contains(&RunEventSummary::Error),
            "metric-call stop should be a clean public stop, not an optimizer error"
        );
        let lines = report_lines(&config, &run);
        assert!(
            lines
                .iter()
                .any(|line| line == "stop_reason=budget_reached")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "optimization_metric_calls=6")
        );
    }

    #[test]
    fn legacy_cache_policy_parser_keeps_solver_and_reflection_role_knobs_scaffolded() {
        let policies = AimeLmCachePolicies::from_values(Some("read-write"), Some("refresh"));

        assert_eq!(policies.solver, LmCachePolicy::ReadWrite);
        assert_eq!(policies.reflection, LmCachePolicy::Refresh);
        assert_eq!(
            AimeLmCachePolicies::from_values(None, None).solver,
            LmCachePolicy::Never
        );
    }

    #[test]
    fn live_openai_runtime_config_names_in_memory_cache_and_provider_throttle() {
        let runtime = AimeOpenAiRuntimeConfig::from_values(Some("8"), Some("in-memory"));

        assert_eq!(runtime.max_concurrent_requests.get(), 8);
        assert_eq!(runtime.cache_backend, AimeLmCacheBackend::InMemory);
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, None)
                .max_concurrent_requests
                .get(),
            GEPA_AIME_MAX_WORKERS
        );
    }

    #[test]
    fn report_lines_disclose_live_lm_role_cache_and_runtime_truth() {
        let mut config = AimeRunConfig::deterministic_smoke();
        config.profile = AimeRunProfile::GepaAime;
        config.solver.live = true;
        config.solver.model = "solver-model".to_owned();
        config.solver.cache_policy = LmCachePolicy::ReadWrite;
        config.solver.runtime = AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("in-memory"));
        config.reflection.live = true;
        config.reflection.model = "reflection-model".to_owned();
        config.reflection.cache_policy = LmCachePolicy::Refresh;
        config.reflection.runtime = config.solver.runtime;
        let result = block_on(run_deterministic_aime());

        let lines = report_lines(&config, &result);

        assert!(lines.iter().any(|line| line == "solver_model=solver-model"));
        assert!(
            lines
                .iter()
                .any(|line| line == "reflection_model=reflection-model")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "solver_cache_policy=read-write")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "reflection_cache_policy=refresh")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "lm_cache_backend=in-memory")
        );
        assert!(lines.iter().any(|line| line == "lm_cache_durable=false"));
        assert!(
            lines
                .iter()
                .any(|line| line == "openai_max_concurrent_requests=7")
        );
        assert!(lines.iter().any(|line| line == "reflection_output=text"));
        assert!(
            lines
                .iter()
                .any(|line| line == "reflection_parser=plain-text-fenced")
        );
    }

    #[test]
    fn public_report_preserves_case_ids_and_generated_outputs() {
        let run = block_on(run_deterministic_aime());
        let result = &run.optimized;
        let validation_id = case_id_from_source_id("deterministic:default:validation:0");
        let test_id = case_id_from_source_id("deterministic:default:test:0");

        let cases = result
            .summary
            .evaluation
            .splits_reported
            .iter()
            .flat_map(|split| &split.candidates)
            .flat_map(|candidate| &candidate.cases)
            .collect::<Vec<_>>();

        assert!(
            cases
                .iter()
                .any(|case| { case.case_id == validation_id && !case.output.is_empty() }),
            "expected deterministic validation case id and generated output in public report"
        );
        assert!(
            cases
                .iter()
                .any(|case| { case.case_id == test_id && !case.output.is_empty() }),
            "expected deterministic test case id and generated output in public report"
        );
    }

    #[test]
    fn aime_cache_loading_preserves_train_validation_test_roles() {
        let path =
            std::env::temp_dir().join(format!("leaven-aime-cache-{}.json", std::process::id()));
        let cache = AimeDatasetCache {
            train: vec![AimeImportRecord {
                source_id: "AI-MO/aimo-validation-aime:default:train:17".to_owned(),
                problem: "train".to_owned(),
                answer: 1,
                solution: "train solution".to_owned(),
                needs_modular: true,
            }],
            validation: vec![AimeImportRecord {
                source_id: "AI-MO/aimo-validation-aime:default:train:42".to_owned(),
                problem: "validation".to_owned(),
                answer: 2,
                solution: "validation solution".to_owned(),
                needs_modular: true,
            }],
            test: vec![AimeImportRecord {
                source_id: "MathArena/aime_2025:default:train:3".to_owned(),
                problem: "test".to_owned(),
                answer: 3,
                solution: "test solution".to_owned(),
                needs_modular: true,
            }],
        };
        std::fs::write(&path, serde_json::to_vec(&cache).unwrap()).unwrap();

        let dataset = dataset_from_cache(&path);
        let train_id = case_id_from_source_id("AI-MO/aimo-validation-aime:default:train:17");
        let validation_id = case_id_from_source_id("AI-MO/aimo-validation-aime:default:train:42");
        let test_id = case_id_from_source_id("MathArena/aime_2025:default:train:3");

        assert_eq!(dataset.train[0].id, train_id);
        assert_eq!(dataset.train[0].input.problem, "train");
        assert_eq!(
            dataset.report_metadata[&train_id].source_id(),
            "AI-MO/aimo-validation-aime:default:train:17"
        );
        assert_eq!(dataset.report_metadata[&train_id].split, SplitRole::Train);
        assert_eq!(dataset.validation[0].id, validation_id);
        assert_eq!(
            dataset.report_metadata[&validation_id].source_id(),
            "AI-MO/aimo-validation-aime:default:train:42"
        );
        assert_eq!(
            dataset.report_metadata[&validation_id].split,
            SplitRole::Validation
        );
        assert_eq!(dataset.test[0].id, test_id);
        assert_eq!(dataset.test[0].input.problem, "test");
        assert_eq!(dataset.report_metadata[&test_id].split, SplitRole::Test);
        std::fs::remove_file(path).unwrap();
    }
}
