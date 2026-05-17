use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use leaven::core::{AssessmentTarget, CacheIdentity, InfoRef};
use leaven::engine::{
    CacheBypassReason, CacheStatus, Callback, ErrorPolicy, RunContext, RunEvent, RunGraphView,
};
use leaven::eval::{Case, SplitRole};
use leaven::gepa::{Gepa, ReflectionError, ReflectiveDatasetBuilder, ReflectiveExample};
use leaven::kernel::Metered;
use leaven::kernel::{
    AssessmentId, CandidateId, CaseId, Cost, ErrorKind, FingerprintBuilder, MetadataValue, RunId,
};
use leaven::plumbing::{ContentId, Fingerprint, FiniteF64, MetadataBag};
use leaven::prelude::{
    Artifact, ArtifactIdentity, Budget, EditSurface, Optimized, Part, PartAddress, RunOutput,
    Score, ScoreContext, ScoreError, SurfaceError, SurfaceFingerprint,
};
use leaven::run::{CachePolicy, RunCase, RunProblem, RunResumability, RunStorage};
use leaven_gepa::LmBackedReflectorConfig;
use leaven_lm::{
    Lm, LmError, LmId, LmRequest, LmResponse, Message, Messages, ReasoningEffort, SamplingOptions,
    TokenUsage,
};
use leaven_lm_cache::{CachedLm, InMemoryLmCache, LmCachePolicy, SqliteLmCache};
use leaven_lm_openai::{OpenAiConfig, OpenAiLm, OpenAiThrottlePolicy};
use serde::{Deserialize, Serialize};

const BASELINE: &str = "Solve the math problem carefully. Break down the steps and provide the final answer as a single number.";
const OPTIMIZED: &str = "Solve with modular arithmetic when useful. Verify arithmetic before the final answer. Provide only the final integer.";
const GEPA_AIME_METRIC_CALLS: u64 = 500;
const DSPY_QUICKSTART_METRIC_CALLS: u64 = 150;
const DSPY_QUICKSTART_TEST_SCORE_TARGET: f64 = 0.566;
const GEPA_AIME_MAX_WORKERS: usize = 32;
const GEPA_AIME_MAX_OUTPUT_TOKENS: u32 = 32_000;
// GEPA AIME is controlled by max_metric_calls, not max_iterations. This is a
// Leaven-local safety ceiling; the public metric-call budget is the stop control.
const GEPA_AIME_INTERNAL_ITERATION_CEILING: usize = 500;
const GEPA_AIME_SOLVER_MODEL: &str = "gpt-4.1-mini";
const GEPA_AIME_REFLECTION_MODEL: &str = "gpt-5.4-mini";
const DETERMINISTIC_SOLVER_MODEL: &str = "deterministic-aime-solver";
const LEAVEN_AIME_SOLVER_CACHE_POLICY: &str = "LEAVEN_AIME_SOLVER_CACHE_POLICY";
const LEAVEN_AIME_REFLECTION_CACHE_POLICY: &str = "LEAVEN_AIME_REFLECTION_CACHE_POLICY";
const LEAVEN_AIME_LM_CACHE_BACKEND: &str = "LEAVEN_AIME_LM_CACHE_BACKEND";
const LEAVEN_AIME_PROFILE: &str = "LEAVEN_AIME_PROFILE";
const LEAVEN_AIME_RUN_DIR: &str = "LEAVEN_AIME_RUN_DIR";
const LEAVEN_AIME_DETERMINISTIC_REFLECTION: &str = "LEAVEN_AIME_DETERMINISTIC_REFLECTION";
const LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS: &str = "LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS";
const DETERMINISTIC_SMOKE_METRIC_CALLS: u64 = 512;
const DETERMINISTIC_SMOKE_ITERATIONS: usize = 1;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config = AimeRunConfig::configured();
    match Box::pin(try_run_configured_aime(config.clone())).await {
        Ok(result) => {
            for line in report_lines(&config, &result) {
                println!("{line}");
            }
        }
        Err(error) => {
            eprintln!("p8_aime_gepa_failed={error}");
            std::process::exit(1);
        }
    }
}

fn report_lines(config: &AimeRunConfig, run: &AimeRunResult) -> Vec<String> {
    let result = &run.optimized;
    let mut lines = report_run_header_lines(config, result);
    lines.extend(report_score_lines(result));
    lines.extend(report_runtime_lines(config, result));
    lines.extend(report_budget_and_cache_lines(config, result));
    lines.extend(report_best_and_event_lines(result));
    for role in run.role_reports.iter() {
        lines.extend(report_lm_role_lines(role));
    }
    lines.extend(report_case_lines(run));
    lines
}

fn report_run_header_lines(config: &AimeRunConfig, result: &Optimized<AimePrompt>) -> Vec<String> {
    let mut lines = vec![
        format!("run_profile={}", config.profile.label()),
        format!("proof_classification={}", config.proof_classification()),
        format!("comparison_target={}", config.profile.comparison_target()),
        format!(
            "comparison_published_test_score={}",
            report_score(config.profile.published_test_score())
        ),
        format!(
            "comparison_reflection_prompt={}",
            config.profile.reflection_prompt_claim()
        ),
        format!("data_source={}", config.data_source.label()),
        format!("run_id={}", result.run_id),
        format!(
            "run_storage={}",
            report_run_storage(&result.summary.storage)
        ),
        format!("run_resumable={}", result.summary.storage.is_resumable()),
        format!(
            "run_resumability={}",
            report_resumability(&result.summary.storage)
        ),
        format!("run_dir={}", report_run_dir(&result.summary.storage)),
        format!(
            "latest_checkpoint={}",
            report_latest_checkpoint(&result.summary.storage)
        ),
        format!(
            "summary_json={}",
            result
                .summary
                .reports
                .summary_json
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_owned())
        ),
        format!("compatibility={}", report_compatibility(result)),
    ];
    lines.extend(
        config
            .profile
            .comparison_notes()
            .into_iter()
            .map(|note| format!("comparison_note={note}")),
    );
    lines
}

fn report_compatibility(result: &Optimized<AimePrompt>) -> String {
    result
        .summary
        .compatibility
        .as_ref()
        .map(|summary| {
            format!(
                "schema={} run_kind={} dataset={} splits={} cache={} budget={} lm_roles={}",
                summary.schema,
                summary.run_kind,
                summary.dataset,
                summary.splits,
                summary.cache,
                summary.budget,
                summary.lm_role_count
            )
        })
        .unwrap_or_else(|| "none".to_owned())
}

fn report_score_lines(result: &Optimized<AimePrompt>) -> Vec<String> {
    vec![
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
    ]
}

fn report_runtime_lines(config: &AimeRunConfig, result: &Optimized<AimePrompt>) -> Vec<String> {
    vec![
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
            "lm_cache_path={}",
            report_lm_cache_path(config.solver.runtime.cache_backend, &result.summary.storage)
        ),
        format!(
            "openai_max_concurrent_requests={}",
            config.solver.runtime.max_concurrent_requests
        ),
        "reflection_output=text".to_owned(),
        "reflection_parser=plain-text-fenced".to_owned(),
    ]
}

fn report_budget_and_cache_lines(
    config: &AimeRunConfig,
    result: &Optimized<AimePrompt>,
) -> Vec<String> {
    vec![
        format!("stop_reason={}", report_stop_reason(result.stop)),
        format!(
            "search_metric_call_cap={}",
            report_optional_u64(config.budget.metric_calls)
        ),
        format!(
            "search_metric_calls_spent={}",
            result.summary.optimization_cost.metric_calls
        ),
        "final_report_metric_call_cap=unlimited".to_owned(),
        format!(
            "final_report_metric_calls_spent={}",
            result.summary.final_report_cost.metric_calls
        ),
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
            "eval_cache_policy={}",
            report_evaluation_cache_policy(&config.evaluation_cache_policy)
        ),
        format!(
            "eval_cache=backend={} durable={} hits={} misses={} bypasses={} write_errors={} hit_cost_zero={}",
            result.summary.cache.evaluation.backend.as_str(),
            result.summary.cache.evaluation.durable,
            result.summary.cache.evaluation.hits,
            result.summary.cache.evaluation.misses,
            evaluation_cache_bypass_count(&result.summary.cache.evaluation),
            result.summary.cache.evaluation.write_errors,
            result.summary.cache.evaluation.hit_cost_zero
        ),
        format!(
            "eval_cache_bypass_reasons={}",
            report_evaluation_cache_bypasses(&result.summary.cache.evaluation)
        ),
    ]
}

fn report_best_and_event_lines(result: &Optimized<AimePrompt>) -> Vec<String> {
    vec![
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
    ]
}

fn report_case_lines(run: &AimeRunResult) -> Vec<String> {
    let mut lines = Vec::new();
    for split in &run.optimized.summary.evaluation.splits_reported {
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

fn report_run_storage(storage: &RunStorage) -> &'static str {
    match storage {
        RunStorage::Ephemeral { .. } => "ephemeral",
        RunStorage::Stored { .. } => "stored",
    }
}

fn report_resumability(storage: &RunStorage) -> String {
    match storage {
        RunStorage::Ephemeral { .. } => "ephemeral".to_owned(),
        RunStorage::Stored { resumability, .. } => match resumability {
            RunResumability::Resumable => "resumable".to_owned(),
            RunResumability::NotResumable { reason } => reason.as_str().to_owned(),
        },
    }
}

fn report_run_dir(storage: &RunStorage) -> String {
    match storage {
        RunStorage::Stored {
            run_dir: Some(run_dir),
            ..
        } => run_dir.display().to_string(),
        RunStorage::Stored { .. } | RunStorage::Ephemeral { .. } => "none".to_owned(),
    }
}

fn report_latest_checkpoint(storage: &RunStorage) -> String {
    match storage {
        RunStorage::Stored {
            latest_checkpoint: Some(checkpoint),
            ..
        } => checkpoint.to_string(),
        RunStorage::Stored { .. } | RunStorage::Ephemeral { .. } => "none".to_owned(),
    }
}

fn report_optional_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "unlimited".to_owned(), |value| value.to_string())
}

fn evaluation_cache_bypass_count(summary: &leaven::run::EvaluationCacheSummary) -> u64 {
    summary.bypasses.iter().map(|bypass| bypass.count).sum()
}

fn report_evaluation_cache_bypasses(summary: &leaven::run::EvaluationCacheSummary) -> String {
    if summary.bypasses.is_empty() {
        return "none".to_owned();
    }
    summary
        .bypasses
        .iter()
        .map(|bypass| format!("{}:{}", bypass.reason.as_str(), bypass.count))
        .collect::<Vec<_>>()
        .join(",")
}

fn report_lm_role_lines(role: &AimeLmRoleReport) -> Vec<String> {
    vec![
        format!(
            "lm_role={} provider={} live={} model={} runtime_fingerprint={}",
            role.role.label(),
            role.provider.label(),
            role.live,
            role.model,
            report_fingerprint(role.runtime_fingerprint)
        ),
        format!(
            "lm_role_runtime={} cache_policy={} cache_backend={} cache_durable={} max_concurrent_requests={} output={} parser={}",
            role.role.label(),
            report_lm_cache_policy(role.cache_policy),
            report_lm_cache_backend(role.cache_backend),
            role.cache_durable,
            role.max_concurrent_requests,
            role.output,
            role.parser
        ),
        format!(
            "lm_role_cost={} calls={} prompt_tokens={} cached_input_tokens={} completion_tokens={} reasoning_tokens={} cost_llm_calls={} cost_prompt_tokens={} cost_completion_tokens={}",
            role.role.label(),
            role.metrics.calls,
            role.metrics.usage.input_tokens,
            role.metrics.usage.cached_input_tokens,
            role.metrics.usage.output_tokens,
            role.metrics.usage.reasoning_tokens,
            role.metrics.cost.llm_calls,
            role.metrics.cost.prompt_tokens,
            role.metrics.cost.completion_tokens
        ),
        format!(
            "lm_role_cache={} hits={} misses={} bypasses={} bypass_policy_never={} bypass_refresh={} write_errors={} hit_cost_zero={}",
            role.role.label(),
            role.metrics.cache.hits,
            role.metrics.cache.misses,
            role.metrics.cache.bypasses(),
            role.metrics.cache.bypass_policy_never,
            role.metrics.cache.bypass_refresh,
            role.metrics.cache.write_errors,
            role.metrics.cache.hit_cost_zero
        ),
        format!(
            "lm_role_failures={} count={} missing_credentials={} authentication={} rate_limit={} retry_exhausted={} malformed_provider_response={} answer_parse={} scorer_parse={} budget_refusal={} cache={} transport={} provider={} unknown={}",
            role.role.label(),
            role.metrics.failures.total(),
            role.metrics.failures.missing_credentials,
            role.metrics.failures.authentication,
            role.metrics.failures.rate_limit,
            role.metrics.failures.retry_exhausted,
            role.metrics.failures.malformed_provider_response,
            role.metrics.failures.answer_parse,
            role.metrics.failures.scorer_parse,
            role.metrics.failures.budget_refusal,
            role.metrics.failures.cache,
            role.metrics.failures.transport,
            role.metrics.failures.provider,
            role.metrics.failures.unknown
        ),
    ]
}

fn report_fingerprint(fingerprint: Fingerprint) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(16);
    for byte in &fingerprint.0[..8] {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

fn report_lm_cache_policy(policy: LmCachePolicy) -> &'static str {
    match policy {
        LmCachePolicy::Never => "never",
        LmCachePolicy::ReadWrite => "read-write",
        LmCachePolicy::ReadOnly => "read-only",
        LmCachePolicy::Refresh => "refresh",
    }
}

fn report_evaluation_cache_policy(policy: &CachePolicy) -> &'static str {
    match policy {
        CachePolicy::Never => "never",
        CachePolicy::Deterministic => "deterministic",
        CachePolicy::DeterministicWithSeed(_) => "deterministic-with-seed",
        CachePolicy::UserKey(_) => "user-key",
    }
}

fn report_lm_cache_backend(backend: AimeLmCacheBackend) -> &'static str {
    match backend {
        AimeLmCacheBackend::InMemory => "in-memory",
        AimeLmCacheBackend::Sqlite => "sqlite",
        AimeLmCacheBackend::EagerSqlite => "eager-sqlite",
    }
}

fn report_lm_cache_path(backend: AimeLmCacheBackend, storage: &RunStorage) -> String {
    match backend {
        AimeLmCacheBackend::InMemory => "none".to_owned(),
        AimeLmCacheBackend::Sqlite => match storage {
            RunStorage::Stored {
                run_dir: Some(run_dir),
                ..
            } => SqliteLmCache::path_in_run_dir(run_dir)
                .display()
                .to_string(),
            RunStorage::Stored { .. } | RunStorage::Ephemeral { .. } => "none".to_owned(),
        },
        AimeLmCacheBackend::EagerSqlite => {
            SqliteLmCache::path_in_workspace(".").display().to_string()
        }
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
    Box::pin(run_aime(config, dataset)).await
}

async fn try_run_configured_aime(
    config: AimeRunConfig,
) -> Result<AimeRunResult, leaven::run::OptimizeError> {
    let dataset = configured_dataset();
    Box::pin(try_run_aime(config, dataset)).await
}

#[cfg(test)]
async fn run_aime(config: AimeRunConfig, dataset: AimeDataset) -> AimeRunResult {
    Box::pin(try_run_aime(config, dataset))
        .await
        .expect("AIME GEPA run succeeds")
}

async fn try_run_aime(
    config: AimeRunConfig,
    dataset: AimeDataset,
) -> Result<AimeRunResult, leaven::run::OptimizeError> {
    let run_id = RunId::new();
    let run_dir = config
        .run_dir
        .clone()
        .unwrap_or_else(|| leaven::run::default_local_run_dir(run_id));
    let solver_telemetry = AimeLmTelemetry::new(config.solver.cache_policy);
    let reflection_telemetry = AimeLmTelemetry::new(config.reflection.cache_policy);
    let solver = aime_solver_lm(&config.solver, solver_telemetry.clone(), &run_dir);
    let runner_fingerprint = aime_runner_fingerprint(&config.solver);
    let scorer_fingerprint = aime_scorer_fingerprint();
    let solver_config = config.solver.clone();
    let report_metadata = dataset.report_metadata.clone();
    let reflective_dataset = dataset.reflective_dataset();
    let optimized = Box::pin(
        leaven::prelude::optimize(AimePrompt::new(config.seed_prompt))
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
            .evaluation_cache_policy(config.evaluation_cache_policy.clone())
            .evaluation_parallelism(config.evaluation_parallelism)
            .on_event(AimeProgressCallback::default())
            .using(
                Gepa::reflect_with_lm(
                    aime_reflection_lm(&config.reflection, reflection_telemetry.clone(), &run_dir),
                    config.reflection.model.clone(),
                )
                .with_reflector_config(aime_reflector_config(&config.reflection))
                .surface(AimePromptSurface)
                .build()
                .reflective_dataset(reflective_dataset)
                .max_iterations(config.max_iterations),
            )
            .budget(config.budget.clone())
            .run_id(run_id)
            .run_dir(run_dir)
            .run(),
    )
    .await?;
    let role_reports = AimeRoleReports::from_config(
        &config,
        solver_telemetry.snapshot(),
        reflection_telemetry.snapshot(),
    );
    Ok(AimeRunResult {
        optimized,
        report_metadata,
        role_reports,
    })
}

#[derive(Clone, Debug)]
struct AimeRunConfig {
    profile: AimeRunProfile,
    data_source: AimeDataSource,
    seed_prompt: &'static str,
    budget: Budget,
    evaluation_parallelism: NonZeroUsize,
    max_iterations: usize,
    evaluation_cache_policy: CachePolicy,
    run_dir: Option<PathBuf>,
    solver: AimeSolverConfig,
    reflection: AimeReflectionConfig,
}

impl AimeRunConfig {
    fn configured() -> Self {
        let data_source = AimeDataSource::configured();
        match std::env::var(LEAVEN_AIME_PROFILE).as_deref() {
            Ok("dspy-quickstart") => Self::dspy_quickstart_with_data_source(data_source),
            Ok("gepa-aime") => Self::gepa_aime_with_data_source(data_source),
            Ok("deterministic-smoke") => Self::deterministic_smoke_with_data_source(data_source),
            Ok(raw) => panic!(
                "unsupported {LEAVEN_AIME_PROFILE}={raw:?}; expected deterministic-smoke, dspy-quickstart, or gepa-aime"
            ),
            Err(_) if std::env::var_os("LEAVEN_AIME_LIVE_OPENAI").is_some() => {
                Self::gepa_aime_with_data_source(data_source)
            }
            Err(_) => Self::deterministic_smoke_with_data_source(data_source),
        }
    }

    #[cfg(test)]
    fn gepa_aime() -> Self {
        Self::gepa_aime_with_data_source(AimeDataSource::configured())
    }

    #[cfg(test)]
    fn dspy_quickstart() -> Self {
        Self::dspy_quickstart_with_data_source(AimeDataSource::configured())
    }

    fn gepa_aime_with_data_source(data_source: AimeDataSource) -> Self {
        Self::live_openai_with_data_source(
            AimeRunProfile::GepaAime,
            data_source,
            GEPA_AIME_METRIC_CALLS,
        )
    }

    fn dspy_quickstart_with_data_source(data_source: AimeDataSource) -> Self {
        Self::live_openai_with_data_source(
            AimeRunProfile::DspyQuickstart,
            data_source,
            DSPY_QUICKSTART_METRIC_CALLS,
        )
    }

    fn live_openai_with_data_source(
        profile: AimeRunProfile,
        data_source: AimeDataSource,
        metric_calls: u64,
    ) -> Self {
        let cache_policies = AimeLmCachePolicies::from_env();
        let runtime = AimeOpenAiRuntimeConfig::from_env();
        Self {
            profile,
            data_source,
            seed_prompt: BASELINE,
            budget: Budget::metric_calls(metric_calls),
            evaluation_parallelism: NonZeroUsize::new(GEPA_AIME_MAX_WORKERS)
                .expect("GEPA AIME worker count is non-zero"),
            max_iterations: GEPA_AIME_INTERNAL_ITERATION_CEILING,
            evaluation_cache_policy: CachePolicy::Deterministic,
            run_dir: aime_run_dir_from_env(),
            solver: AimeSolverConfig {
                live: true,
                model: openai_model_name(),
                sampling: gepa_aime_sampling(),
                cache_policy: cache_policies.solver,
                runtime,
            },
            reflection: AimeReflectionConfig {
                live: std::env::var_os(LEAVEN_AIME_DETERMINISTIC_REFLECTION).is_none(),
                model: aime_reflection_model_name(),
                sampling: SamplingOptions::default().with_reasoning_effort(ReasoningEffort::Medium),
                cache_policy: cache_policies.reflection,
                runtime,
            },
        }
    }

    #[cfg(test)]
    fn deterministic_smoke() -> Self {
        Self::deterministic_smoke_with_data_source(AimeDataSource::DeterministicFixture)
    }

    fn deterministic_smoke_with_data_source(data_source: AimeDataSource) -> Self {
        Self {
            profile: AimeRunProfile::DeterministicSmoke,
            data_source,
            seed_prompt: BASELINE,
            budget: Budget::metric_calls(DETERMINISTIC_SMOKE_METRIC_CALLS),
            evaluation_parallelism: NonZeroUsize::new(1).expect("smoke worker count is non-zero"),
            max_iterations: DETERMINISTIC_SMOKE_ITERATIONS,
            evaluation_cache_policy: CachePolicy::Never,
            run_dir: aime_run_dir_from_env(),
            solver: AimeSolverConfig {
                live: false,
                model: DETERMINISTIC_SOLVER_MODEL.to_owned(),
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

    fn proof_classification(&self) -> &'static str {
        match (self.solver.live, self.reflection.live, self.data_source) {
            (false, false, AimeDataSource::DeterministicFixture) => {
                "deterministic_mechanics_product_surface_proof"
            }
            (false, false, AimeDataSource::HuggingFaceCache) => "local_cached_data_proof",
            (true, false, _) => "live_solver_proof",
            (false, true, _) => "live_reflection_proof",
            (true, true, _) => match self.profile {
                AimeRunProfile::DspyQuickstart => "live_dspy_quickstart_comparison_attempt",
                AimeRunProfile::GepaAime => "full_live_aime_reproduction_attempt",
                AimeRunProfile::DeterministicSmoke => "live_smoke_override",
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AimeRunProfile {
    DeterministicSmoke,
    DspyQuickstart,
    GepaAime,
}

impl AimeRunProfile {
    const fn label(self) -> &'static str {
        match self {
            Self::DeterministicSmoke => "deterministic-smoke",
            Self::DspyQuickstart => "dspy-quickstart",
            Self::GepaAime => "gepa-aime",
        }
    }

    const fn comparison_target(self) -> &'static str {
        match self {
            Self::DeterministicSmoke => "none",
            Self::DspyQuickstart => "dspy_gepa_quickstart_aime_2025",
            Self::GepaAime => "gepa_cais_aime_math_artifact",
        }
    }

    const fn published_test_score(self) -> Option<f64> {
        match self {
            Self::DeterministicSmoke => None,
            Self::DspyQuickstart => Some(DSPY_QUICKSTART_TEST_SCORE_TARGET),
            Self::GepaAime => Some(0.600),
        }
    }

    const fn reflection_prompt_claim(self) -> &'static str {
        match self {
            Self::DeterministicSmoke | Self::DspyQuickstart | Self::GepaAime => {
                "upstream_gepa_instruction_template"
            }
        }
    }

    fn comparison_notes(self) -> Vec<&'static str> {
        match self {
            Self::DeterministicSmoke => Vec::new(),
            Self::DspyQuickstart => vec![
                "published_dspy_quickstart_reports_46.6_to_56.6_percent_on_aime_2025",
                "leaven_runs_without_dspy_runtime_or_dspy_chainofthought_lowering",
                "leaven_uses_gpt_5_4_mini_reflection_model_by_default",
            ],
            Self::GepaAime => vec![
                "published_gepa_cais_artifact_reports_46.67_to_60.00_percent_on_aime_2025",
                "leaven_runs_without_dspy_chainofthought_lowering",
                "leaven_uses_gpt_5_4_mini_reflection_model_by_default",
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AimeDataSource {
    DeterministicFixture,
    HuggingFaceCache,
}

impl AimeDataSource {
    fn configured() -> Self {
        if std::env::var_os("LEAVEN_AIME_CACHE").is_some() {
            Self::HuggingFaceCache
        } else {
            Self::DeterministicFixture
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::DeterministicFixture => "deterministic-fixture",
            Self::HuggingFaceCache => "huggingface-cache",
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
enum AimeLmRole {
    Solver,
    Reflection,
}

impl AimeLmRole {
    const fn label(self) -> &'static str {
        match self {
            Self::Solver => "solver",
            Self::Reflection => "reflection",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AimeLmProvider {
    Deterministic,
    OpenAi,
}

impl AimeLmProvider {
    const fn label(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic-fixture",
            Self::OpenAi => "openai",
        }
    }
}

#[derive(Clone, Debug)]
struct AimeRoleReports {
    solver: AimeLmRoleReport,
    reflection: AimeLmRoleReport,
}

impl AimeRoleReports {
    fn from_config(
        config: &AimeRunConfig,
        solver_metrics: AimeLmRoleMetrics,
        reflection_metrics: AimeLmRoleMetrics,
    ) -> Self {
        Self {
            solver: AimeLmRoleReport {
                role: AimeLmRole::Solver,
                provider: if config.solver.live {
                    AimeLmProvider::OpenAi
                } else {
                    AimeLmProvider::Deterministic
                },
                live: config.solver.live,
                model: config.solver.model.clone(),
                runtime_fingerprint: aime_runner_fingerprint(&config.solver),
                cache_policy: config.solver.cache_policy,
                cache_backend: config.solver.runtime.cache_backend,
                cache_durable: config.solver.runtime.cache_backend.is_durable(),
                max_concurrent_requests: config.solver.runtime.max_concurrent_requests,
                output: "answer-text",
                parser: "trimmed-answer",
                metrics: solver_metrics,
            },
            reflection: AimeLmRoleReport {
                role: AimeLmRole::Reflection,
                provider: if config.reflection.live {
                    AimeLmProvider::OpenAi
                } else {
                    AimeLmProvider::Deterministic
                },
                live: config.reflection.live,
                model: config.reflection.model.clone(),
                runtime_fingerprint: aime_reflection_role_fingerprint(&config.reflection),
                cache_policy: config.reflection.cache_policy,
                cache_backend: config.reflection.runtime.cache_backend,
                cache_durable: config.reflection.runtime.cache_backend.is_durable(),
                max_concurrent_requests: config.reflection.runtime.max_concurrent_requests,
                output: "text",
                parser: "plain-text-fenced",
                metrics: reflection_metrics,
            },
        }
    }

    fn iter(&self) -> impl Iterator<Item = &AimeLmRoleReport> {
        [&self.solver, &self.reflection].into_iter()
    }
}

#[derive(Clone, Debug)]
struct AimeLmRoleReport {
    role: AimeLmRole,
    provider: AimeLmProvider,
    live: bool,
    model: String,
    runtime_fingerprint: Fingerprint,
    cache_policy: LmCachePolicy,
    cache_backend: AimeLmCacheBackend,
    cache_durable: bool,
    max_concurrent_requests: NonZeroUsize,
    output: &'static str,
    parser: &'static str,
    metrics: AimeLmRoleMetrics,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct AimeLmRoleMetrics {
    calls: u64,
    usage: TokenUsage,
    cost: Cost,
    cache: AimeLmCacheMetrics,
    failures: AimeProviderFailureCounts,
}

impl AimeLmRoleMetrics {
    fn record_success(&mut self, policy: LmCachePolicy, response: &LmResponse, cost: &Cost) {
        self.calls += 1;
        self.usage.input_tokens += response.usage.input_tokens;
        self.usage.cached_input_tokens += response.usage.cached_input_tokens;
        self.usage.output_tokens += response.usage.output_tokens;
        self.usage.reasoning_tokens += response.usage.reasoning_tokens;
        self.cost = self.cost.clone().combine(cost);
        self.cache.record_success(policy, &response.usage, cost);
    }

    fn record_failure(&mut self, error: &LmError) {
        let kind = AimeProviderFailureKind::from_lm_error(error);
        self.failures.increment(kind);
        if kind == AimeProviderFailureKind::Cache {
            self.cache.write_errors += 1;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeLmCacheMetrics {
    hits: u64,
    misses: u64,
    bypass_policy_never: u64,
    bypass_refresh: u64,
    write_errors: u64,
    hit_cost_zero: bool,
}

impl Default for AimeLmCacheMetrics {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            bypass_policy_never: 0,
            bypass_refresh: 0,
            write_errors: 0,
            hit_cost_zero: true,
        }
    }
}

impl AimeLmCacheMetrics {
    fn record_success(&mut self, policy: LmCachePolicy, usage: &TokenUsage, cost: &Cost) {
        match policy {
            LmCachePolicy::Never => {
                self.bypass_policy_never += 1;
            }
            LmCachePolicy::Refresh => {
                self.bypass_refresh += 1;
            }
            LmCachePolicy::ReadWrite | LmCachePolicy::ReadOnly => {
                if cost.is_zero() && !usage.to_cost().is_zero() {
                    self.hits += 1;
                    self.hit_cost_zero &= cost.is_zero();
                } else {
                    self.misses += 1;
                }
            }
        }
    }

    const fn bypasses(&self) -> u64 {
        self.bypass_policy_never + self.bypass_refresh
    }
}

#[derive(Clone, Debug)]
struct AimeLmTelemetry {
    policy: LmCachePolicy,
    metrics: Arc<Mutex<AimeLmRoleMetrics>>,
}

impl AimeLmTelemetry {
    fn new(policy: LmCachePolicy) -> Self {
        Self {
            policy,
            metrics: Arc::new(Mutex::new(AimeLmRoleMetrics::default())),
        }
    }

    fn record(&self, result: &Result<Metered<LmResponse>, LmError>) {
        let mut metrics = self.metrics.lock().expect("AIME telemetry lock is valid");
        match result {
            Ok(metered) => {
                metrics.record_success(self.policy, &metered.value, &metered.cost);
            }
            Err(error) => {
                metrics.record_failure(error);
            }
        }
    }

    fn snapshot(&self) -> AimeLmRoleMetrics {
        self.metrics
            .lock()
            .expect("AIME telemetry lock is valid")
            .clone()
    }
}

#[derive(Clone)]
struct AimeInstrumentedLm<L> {
    inner: L,
    telemetry: AimeLmTelemetry,
}

impl<L> AimeInstrumentedLm<L> {
    fn new(inner: L, telemetry: AimeLmTelemetry) -> Self {
        Self { inner, telemetry }
    }
}

impl<L: std::fmt::Debug> std::fmt::Debug for AimeInstrumentedLm<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AimeInstrumentedLm")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<L: Lm> Lm for AimeInstrumentedLm<L> {
    fn id(&self) -> LmId {
        self.inner.id()
    }

    fn fingerprint(&self) -> Fingerprint {
        self.inner.fingerprint()
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        let result = self.inner.complete(request).await;
        self.telemetry.record(&result);
        result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AimeProviderFailureKind {
    MissingCredentials,
    Authentication,
    RateLimit,
    RetryExhausted,
    MalformedProviderResponse,
    AnswerParse,
    ScorerParse,
    BudgetRefusal,
    Cache,
    Transport,
    Provider,
    Unknown,
}

impl AimeProviderFailureKind {
    fn from_lm_error(error: &LmError) -> Self {
        match error {
            LmError::InvalidRequest { reason } if reason.contains("OPENAI_API_KEY") => {
                Self::MissingCredentials
            }
            LmError::InvalidRequest { reason } if reason.contains("retry") => Self::RetryExhausted,
            LmError::InvalidRequest { reason } if reason.contains("answer parse") => {
                Self::AnswerParse
            }
            LmError::InvalidRequest { reason } if reason.contains("scorer parse") => {
                Self::ScorerParse
            }
            LmError::InvalidRequest { reason } if reason.contains("budget") => Self::BudgetRefusal,
            LmError::InvalidRequest { .. } => Self::Unknown,
            LmError::InvalidResponse { .. } => Self::MalformedProviderResponse,
            LmError::Provider {
                status: Some(401 | 403),
                ..
            } => Self::Authentication,
            LmError::Provider {
                status: Some(429), ..
            } => Self::RateLimit,
            LmError::Provider { .. } => Self::Provider,
            LmError::Transport { .. } => Self::Transport,
            LmError::Cache { .. } => Self::Cache,
        }
    }

    #[cfg(test)]
    const fn label(self) -> &'static str {
        match self {
            Self::MissingCredentials => "missing_credentials",
            Self::Authentication => "authentication",
            Self::RateLimit => "rate_limit",
            Self::RetryExhausted => "retry_exhausted",
            Self::MalformedProviderResponse => "malformed_provider_response",
            Self::AnswerParse => "answer_parse",
            Self::ScorerParse => "scorer_parse",
            Self::BudgetRefusal => "budget_refusal",
            Self::Cache => "cache",
            Self::Transport => "transport",
            Self::Provider => "provider",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AimeProviderFailureCounts {
    missing_credentials: u64,
    authentication: u64,
    rate_limit: u64,
    retry_exhausted: u64,
    malformed_provider_response: u64,
    answer_parse: u64,
    scorer_parse: u64,
    budget_refusal: u64,
    cache: u64,
    transport: u64,
    provider: u64,
    unknown: u64,
}

impl AimeProviderFailureCounts {
    fn increment(&mut self, kind: AimeProviderFailureKind) {
        match kind {
            AimeProviderFailureKind::MissingCredentials => self.missing_credentials += 1,
            AimeProviderFailureKind::Authentication => self.authentication += 1,
            AimeProviderFailureKind::RateLimit => self.rate_limit += 1,
            AimeProviderFailureKind::RetryExhausted => self.retry_exhausted += 1,
            AimeProviderFailureKind::MalformedProviderResponse => {
                self.malformed_provider_response += 1;
            }
            AimeProviderFailureKind::AnswerParse => self.answer_parse += 1,
            AimeProviderFailureKind::ScorerParse => self.scorer_parse += 1,
            AimeProviderFailureKind::BudgetRefusal => self.budget_refusal += 1,
            AimeProviderFailureKind::Cache => self.cache += 1,
            AimeProviderFailureKind::Transport => self.transport += 1,
            AimeProviderFailureKind::Provider => self.provider += 1,
            AimeProviderFailureKind::Unknown => self.unknown += 1,
        }
    }

    const fn total(&self) -> u64 {
        self.missing_credentials
            + self.authentication
            + self.rate_limit
            + self.retry_exhausted
            + self.malformed_provider_response
            + self.answer_parse
            + self.scorer_parse
            + self.budget_refusal
            + self.cache
            + self.transport
            + self.provider
            + self.unknown
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeProviderFailureSummary {
    kind: AimeProviderFailureKind,
    provider: String,
    status: Option<u16>,
    message: &'static str,
}

#[cfg(test)]
impl AimeProviderFailureSummary {
    fn from_lm_error(error: &LmError) -> Self {
        let kind = AimeProviderFailureKind::from_lm_error(error);
        let (provider, status) = match error {
            LmError::InvalidRequest { .. } | LmError::Cache { .. } => ("unknown".to_owned(), None),
            LmError::InvalidResponse { provider, .. }
            | LmError::Provider { provider, .. }
            | LmError::Transport { provider, .. } => {
                let status = if let LmError::Provider { status, .. } = error {
                    *status
                } else {
                    None
                };
                (provider.clone(), status)
            }
        };
        Self {
            kind,
            provider,
            status,
            message: redacted_failure_message(kind),
        }
    }

    fn report_line(&self, role: AimeLmRole) -> String {
        format!(
            "provider_failure role={} kind={} provider={} status={} message={}",
            role.label(),
            self.kind.label(),
            self.provider,
            self.status
                .map_or_else(|| "none".to_owned(), |status| status.to_string()),
            self.message
        )
    }
}

#[cfg(test)]
const fn redacted_failure_message(kind: AimeProviderFailureKind) -> &'static str {
    match kind {
        AimeProviderFailureKind::MissingCredentials => "missing required credential",
        AimeProviderFailureKind::Authentication => "provider authentication failed",
        AimeProviderFailureKind::RateLimit => "provider rate limited the request",
        AimeProviderFailureKind::RetryExhausted => "retry policy exhausted",
        AimeProviderFailureKind::MalformedProviderResponse => {
            "provider response could not be lowered"
        }
        AimeProviderFailureKind::AnswerParse => "solver answer could not be parsed",
        AimeProviderFailureKind::ScorerParse => "scorer response could not be parsed",
        AimeProviderFailureKind::BudgetRefusal => {
            "budget refused the request before provider execution"
        }
        AimeProviderFailureKind::Cache => "lm response cache failed",
        AimeProviderFailureKind::Transport => "provider transport failed",
        AimeProviderFailureKind::Provider => "provider returned a failure response",
        AimeProviderFailureKind::Unknown => "provider failure was not classified",
    }
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
        return LmCachePolicy::ReadWrite;
    };
    match raw.to_ascii_lowercase().as_str() {
        "never" | "none" | "off" => LmCachePolicy::Never,
        "auto" | "read-write" | "read_write" | "readwrite" => LmCachePolicy::ReadWrite,
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
    Sqlite,
    EagerSqlite,
}

impl AimeLmCacheBackend {
    const fn is_durable(self) -> bool {
        match self {
            Self::InMemory => false,
            Self::Sqlite | Self::EagerSqlite => true,
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
        return AimeLmCacheBackend::Sqlite;
    };
    match raw.to_ascii_lowercase().as_str() {
        "in-memory" | "in_memory" | "memory" => AimeLmCacheBackend::InMemory,
        "sqlite" | "durable" | "local-sqlite" | "local_sqlite" => AimeLmCacheBackend::Sqlite,
        "eager" | "eager-sqlite" | "eager_sqlite" | "workspace-sqlite" | "workspace_sqlite"
        | "shared-sqlite" | "shared_sqlite" => AimeLmCacheBackend::EagerSqlite,
        _ => panic!(
            "unsupported {LEAVEN_AIME_LM_CACHE_BACKEND}={raw:?}; expected sqlite, eager-sqlite, or in-memory"
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

fn aime_reflection_lm(
    config: &AimeReflectionConfig,
    telemetry: AimeLmTelemetry,
    run_dir: &Path,
) -> AimeReflectionLm {
    if config.live {
        AimeReflectionLm::OpenAi(AimeInstrumentedLm::new(
            cached_openai_lm(
                config.cache_policy,
                config.runtime,
                run_dir,
                "live reflection",
            ),
            telemetry,
        ))
    } else {
        AimeReflectionLm::Deterministic(AimeInstrumentedLm::new(
            DeterministicReflectionLm,
            telemetry,
        ))
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

fn aime_run_dir_from_env() -> Option<PathBuf> {
    std::env::var_os(LEAVEN_AIME_RUN_DIR)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[derive(Clone)]
enum AimeReflectionLm {
    Deterministic(AimeInstrumentedLm<DeterministicReflectionLm>),
    OpenAi(AimeInstrumentedLm<AimeOpenAiLm>),
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

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(content_id(self.system.as_bytes())))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self {
            system: change.system.clone(),
        })
    }
}

#[derive(Default)]
struct AimeProgressCallback {
    evaluation_requests: u64,
    assessment_rows: u64,
}

impl Callback<RunProblem<AimePrompt, AimeInput, AimeTarget>> for AimeProgressCallback {
    fn on_event(
        &mut self,
        event: &RunEvent,
        _graph: RunGraphView<'_, RunProblem<AimePrompt, AimeInput, AimeTarget>>,
    ) {
        if let Some(line) = self.progress_line(event) {
            eprintln!("{line}");
        }
    }
}

impl AimeProgressCallback {
    fn progress_line(&mut self, event: &RunEvent) -> Option<String> {
        match event {
            RunEvent::OptimizationStarted { run_id } => Some(format!(
                "progress_event=optimization_started run_id={run_id}"
            )),
            RunEvent::IterationStarted { iteration } => Some(format!(
                "progress_event=iteration_started iteration={iteration}"
            )),
            RunEvent::IterationEnded { iteration } => Some(format!(
                "progress_event=iteration_ended iteration={iteration}"
            )),
            RunEvent::ProposalRecorded {
                proposal_id,
                batch_id,
                effect,
                informed_by_count,
                ..
            } => Some(format!(
                "progress_event=proposal_recorded proposal_id={proposal_id} batch_id={batch_id} effect={effect:?} informed_by_count={informed_by_count}"
            )),
            RunEvent::ApplySucceeded {
                proposal_id,
                candidate_id,
            } => Some(format!(
                "progress_event=apply_succeeded proposal_id={proposal_id} candidate_id={candidate_id}"
            )),
            RunEvent::EvaluationCompleted {
                request_id,
                evaluator,
                assessment_ids,
                cost,
                cache,
            } => {
                self.evaluation_requests += 1;
                self.assessment_rows += assessment_ids.len() as u64;
                Some(format!(
                    "progress_event=evaluation_completed request_id={request_id} evaluator={evaluator} request_count={} assessment_rows={} total_assessment_rows={} metric_calls={} llm_calls={} cache={}",
                    self.evaluation_requests,
                    assessment_ids.len(),
                    self.assessment_rows,
                    cost.metric_calls,
                    cost.llm_calls,
                    progress_cache_status(cache)
                ))
            }
            RunEvent::PopulationUpdated {
                population_id,
                events,
            } => Some(format!(
                "progress_event=population_updated population_id={population_id} event_count={}",
                events.len()
            )),
            RunEvent::OptimizationStopping { reason } => Some(format!(
                "progress_event=optimization_stopping reason={reason:?}"
            )),
            RunEvent::OptimizationEnded {
                run_id,
                best,
                budget,
            } => Some(format!(
                "progress_event=optimization_ended run_id={run_id} best={} metric_calls={} llm_calls={}",
                best.map_or_else(|| "none".to_owned(), |candidate| candidate.to_string()),
                budget.spent.metric_calls,
                budget.spent.llm_calls
            )),
            RunEvent::Error {
                stage: _,
                error,
                policy: ErrorPolicy::StoppedRun,
            } if error.kind == ErrorKind::Budget => None,
            RunEvent::Error {
                stage,
                error,
                policy,
            } => Some(format!(
                "progress_event=error stage={} kind={:?} policy={policy:?}",
                stage
                    .as_ref()
                    .map_or_else(|| "none".to_owned(), ToString::to_string),
                error.kind
            )),
            _ => None,
        }
    }
}

fn progress_cache_status(cache: &CacheStatus) -> &'static str {
    match cache {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
        CacheStatus::Bypassed(CacheBypassReason::DisabledByPolicy) => "bypass-disabled-by-policy",
        CacheStatus::Bypassed(CacheBypassReason::CacheUnavailable) => "bypass-cache-unavailable",
        CacheStatus::Bypassed(CacheBypassReason::MissingCandidateIdentity { .. }) => {
            "bypass-missing-candidate-identity"
        }
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
    role_reports: AimeRoleReports,
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
        let train = lowerer.lower_split(&SplitRole::Train, cache.train)?;
        let validation = lowerer.lower_split(&SplitRole::Validation, cache.validation)?;
        let test = lowerer.lower_split(&SplitRole::Test, cache.test)?;
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
        split: &SplitRole,
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
        parent: CandidateId,
        parent_assessments: &[AssessmentId],
        _part: &&'static str,
    ) -> Result<Vec<ReflectiveExample>, ReflectionError> {
        let mut examples = Vec::with_capacity(parent_assessments.len());
        for parent_assessment in parent_assessments {
            let assessment = ctx.graph().assessment(*parent_assessment).ok_or_else(|| {
                ReflectionError::builder(format!(
                    "AIME reflection assessment row `{parent_assessment}` is missing from graph"
                ))
            })?;
            if assessment.independent_candidate() != Some(parent) {
                return Err(ReflectionError::builder(
                    "AIME reflection assessment row belongs to a different candidate",
                ));
            }
            let case = match assessment.target() {
                AssessmentTarget::Case { case, .. } => *case,
                AssessmentTarget::Unscoped | AssessmentTarget::EvaluationSet(_) => {
                    return Err(ReflectionError::builder(
                        "AIME reflection expected case-targeted assessment rows",
                    ));
                }
            };
            let evidence = ctx.assessment_evidence(*parent_assessment)?;
            examples.push(ReflectiveExample {
                case: Some(case),
                input: self.inputs_by_case.get(&case).cloned().unwrap_or_default(),
                output: Some(format!("{:?}", evidence.output())),
                score: Some(evidence.score().score()),
                feedback: evidence.feedback().to_owned(),
                source_refs: vec![InfoRef::Assessment(*parent_assessment)],
            });
        }
        Ok(examples)
    }
}

#[derive(Clone)]
struct AimeOpenAiLm {
    inner: AimeOpenAiCachedLm,
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

#[derive(Clone)]
enum AimeOpenAiCachedLm {
    InMemory(CachedLm<OpenAiLm, InMemoryLmCache>),
    Sqlite(CachedLm<OpenAiLm, SqliteLmCache>),
    Unavailable {
        id: LmId,
        fingerprint: Fingerprint,
        reason: AimeOpenAiUnavailableReason,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AimeOpenAiUnavailableReason {
    MissingCredentials,
    Cache,
}

impl Lm for AimeOpenAiCachedLm {
    fn id(&self) -> LmId {
        match self {
            Self::InMemory(inner) => inner.id(),
            Self::Sqlite(inner) => inner.id(),
            Self::Unavailable { id, .. } => id.clone(),
        }
    }

    fn fingerprint(&self) -> Fingerprint {
        match self {
            Self::InMemory(inner) => inner.fingerprint(),
            Self::Sqlite(inner) => inner.fingerprint(),
            Self::Unavailable { fingerprint, .. } => *fingerprint,
        }
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        match self {
            Self::InMemory(inner) => inner.complete(request).await,
            Self::Sqlite(inner) => inner.complete(request).await,
            Self::Unavailable {
                reason, message, ..
            } => match reason {
                AimeOpenAiUnavailableReason::MissingCredentials => Err(LmError::InvalidRequest {
                    reason: message.clone(),
                }),
                AimeOpenAiUnavailableReason::Cache => Err(LmError::Cache {
                    message: message.clone(),
                }),
            },
        }
    }
}

fn aime_solver_lm(
    config: &AimeSolverConfig,
    telemetry: AimeLmTelemetry,
    run_dir: &Path,
) -> Option<AimeInstrumentedLm<AimeOpenAiLm>> {
    if config.live {
        Some(AimeInstrumentedLm::new(
            cached_openai_lm(config.cache_policy, config.runtime, run_dir, "live solver"),
            telemetry,
        ))
    } else {
        None
    }
}

fn cached_openai_lm(
    cache_policy: LmCachePolicy,
    runtime: AimeOpenAiRuntimeConfig,
    run_dir: &Path,
    role: &str,
) -> AimeOpenAiLm {
    let config = match OpenAiConfig::from_env() {
        Ok(config) => config.with_throttle_policy(OpenAiThrottlePolicy::new(
            runtime.max_concurrent_requests,
            Duration::ZERO,
        )),
        Err(source) => {
            return unavailable_openai_lm(
                role,
                AimeOpenAiUnavailableReason::MissingCredentials,
                format!("OPENAI_API_KEY is not set for {role}: {source}"),
            );
        }
    };
    let inner = OpenAiLm::new(config);
    match runtime.cache_backend {
        AimeLmCacheBackend::InMemory => AimeOpenAiLm {
            inner: AimeOpenAiCachedLm::InMemory(CachedLm::new(
                inner,
                InMemoryLmCache::default(),
                cache_policy,
            )),
        },
        AimeLmCacheBackend::Sqlite => AimeOpenAiLm {
            inner: AimeOpenAiCachedLm::Sqlite(CachedLm::new(
                inner,
                match SqliteLmCache::open_run_dir(run_dir) {
                    Ok(cache) => cache,
                    Err(source) => {
                        return unavailable_openai_lm(
                            role,
                            AimeOpenAiUnavailableReason::Cache,
                            format!("failed to open SQLite LM cache for {role}: {source}"),
                        );
                    }
                },
                cache_policy,
            )),
        },
        AimeLmCacheBackend::EagerSqlite => AimeOpenAiLm {
            inner: AimeOpenAiCachedLm::Sqlite(CachedLm::new(
                inner,
                match SqliteLmCache::open_workspace(".") {
                    Ok(cache) => cache,
                    Err(source) => {
                        return unavailable_openai_lm(
                            role,
                            AimeOpenAiUnavailableReason::Cache,
                            format!("failed to open eager SQLite LM cache for {role}: {source}"),
                        );
                    }
                },
                cache_policy,
            )),
        },
    }
}

fn unavailable_openai_lm(
    role: &str,
    reason: AimeOpenAiUnavailableReason,
    message: String,
) -> AimeOpenAiLm {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"p8-aime-openai-unavailable.v1");
    builder.update(role.as_bytes());
    builder.update(format!("{reason:?}").as_bytes());
    builder.update(message.as_bytes());
    AimeOpenAiLm {
        inner: AimeOpenAiCachedLm::Unavailable {
            id: LmId::new(format!("p8-aime-openai-{role}-unavailable")),
            fingerprint: builder.finish(),
            reason,
            message,
        },
    }
}

async fn run_solver(
    prompt: AimePrompt,
    case: RunCase<AimeInput>,
    solver: Option<AimeInstrumentedLm<AimeOpenAiLm>>,
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
    solver: AimeInstrumentedLm<AimeOpenAiLm>,
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

fn aime_reflection_role_fingerprint(config: &AimeReflectionConfig) -> Fingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"p8-aime-reflection-role.v1");
    builder.update([u8::from(config.live)]);
    builder.update(config.model.as_bytes());
    builder.update(
        serde_json::to_vec(&config.sampling).expect("AIME reflection sampling config serializes"),
    );
    builder.update(report_lm_cache_policy(config.cache_policy).as_bytes());
    builder.update(report_lm_cache_backend(config.runtime.cache_backend).as_bytes());
    builder.update(config.runtime.max_concurrent_requests.get().to_le_bytes());
    builder.update(b"output:text");
    builder.update(b"parser:plain-text-fenced");
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
        "Find the remainder when 5^4 is divided by 13."
        | "Find the remainder when 3^6 is divided by 7." => 1,
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
    use leaven::kernel::{EvaluationRequestId, EvaluatorId};
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
    fn aime_prompt_exposes_cache_safe_content_identity() {
        let prompt = AimePrompt::new("cache me");
        let expected = content_id(prompt.system.as_bytes());

        assert_eq!(prompt.identity(), ArtifactIdentity::Content(expected));
        assert_eq!(
            prompt.cache_identity(),
            Some(CacheIdentity::Content(expected))
        );
    }

    #[test]
    fn progress_callback_reports_evaluations_and_cache_status() {
        let mut progress = AimeProgressCallback::default();
        let line = progress
            .progress_line(&RunEvent::EvaluationCompleted {
                request_id: EvaluationRequestId::new(),
                evaluator: EvaluatorId::PRIMARY,
                assessment_ids: vec![AssessmentId::new(), AssessmentId::new()],
                cost: Cost::metric_calls(2),
                cache: CacheStatus::Miss,
            })
            .expect("evaluation event emits progress");

        assert!(line.starts_with("progress_event=evaluation_completed "));
        assert!(line.contains("request_count=1"));
        assert!(line.contains("assessment_rows=2"));
        assert!(line.contains("total_assessment_rows=2"));
        assert!(line.contains("metric_calls=2"));
        assert!(line.contains("cache=miss"));
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
        assert_eq!(run.role_reports.solver.metrics.calls, 0);
        assert_eq!(run.role_reports.reflection.metrics.calls, 1);
        assert_eq!(run.role_reports.reflection.metrics.cost.llm_calls, 1);
        assert_eq!(run.role_reports.reflection.metrics.usage.input_tokens, 37);
        assert_eq!(
            run.role_reports
                .reflection
                .metrics
                .cache
                .bypass_policy_never,
            1
        );
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
    fn reference_gepa_requires_validation_instead_of_silent_train_only_fallback() {
        let config = AimeRunConfig::deterministic_smoke();
        let mut dataset = deterministic_dataset();
        dataset.validation.clear();
        dataset.test.clear();
        let error = block_on(try_run_aime(config, dataset)).unwrap_err();

        assert!(error.to_string().contains("Validation"));
    }

    #[test]
    fn run_builder_requires_score_function() {
        let config = AimeRunConfig::deterministic_smoke();
        let solver_config = config.solver.clone();
        let dataset = deterministic_dataset();
        let reflective_dataset = dataset.reflective_dataset();
        let run_id = RunId::new();
        let run_dir = leaven::run::default_local_run_dir(run_id);
        let error = block_on(async {
            Box::pin(
                leaven::prelude::optimize(AimePrompt::new(config.seed_prompt))
                    .train(dataset.train)
                    .runner(move |prompt, case| {
                        let solver_config = solver_config.clone();
                        async move { run_solver(prompt, case, None, solver_config).await }
                    })
                    .using(
                        Gepa::reflect_with_lm(
                            aime_reflection_lm(
                                &config.reflection,
                                AimeLmTelemetry::new(config.reflection.cache_policy),
                                &run_dir,
                            ),
                            config.reflection.model.clone(),
                        )
                        .with_reflector_config(aime_reflector_config(&config.reflection))
                        .surface(AimePromptSurface)
                        .build()
                        .reflective_dataset(reflective_dataset),
                    )
                    .budget(Budget::metric_calls(8))
                    .run_id(run_id)
                    .run_dir(run_dir)
                    .run(),
            )
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
        assert_eq!(config.evaluation_cache_policy, CachePolicy::Deterministic);
        assert!(config.run_dir.is_none());
        assert!(config.solver.live);
        assert_eq!(config.solver.model, openai_model_name());
        assert_eq!(config.solver.cache_policy, LmCachePolicy::ReadWrite);
        assert_eq!(
            config.solver.sampling.temperature.map(FiniteF64::as_f64),
            Some(1.0)
        );
        assert_eq!(
            config.solver.sampling.max_output_tokens,
            Some(GEPA_AIME_MAX_OUTPUT_TOKENS)
        );
        assert!(config.reflection.live);
        assert_eq!(config.reflection.model, aime_reflection_model_name());
        assert_eq!(config.reflection.model, "gpt-5.4-mini");
        assert_eq!(config.reflection.cache_policy, LmCachePolicy::ReadWrite);
        assert_eq!(
            config.reflection.sampling.reasoning_effort,
            Some(ReasoningEffort::Medium)
        );
    }

    #[test]
    fn dspy_quickstart_profile_matches_published_comparison_denominator() {
        let config = AimeRunConfig::dspy_quickstart();

        assert_eq!(config.profile, AimeRunProfile::DspyQuickstart);
        assert_eq!(config.seed_prompt, BASELINE);
        assert_eq!(
            config.budget.metric_calls,
            Some(DSPY_QUICKSTART_METRIC_CALLS)
        );
        assert_eq!(
            config.profile.comparison_target(),
            "dspy_gepa_quickstart_aime_2025"
        );
        assert_eq!(
            config.profile.published_test_score(),
            Some(DSPY_QUICKSTART_TEST_SCORE_TARGET)
        );
        assert!(config.solver.live);
        assert!(config.reflection.live);
        assert_eq!(config.solver.model, openai_model_name());
        assert_eq!(config.reflection.model, "gpt-5.4-mini");
        assert_eq!(
            config.reflection.sampling.reasoning_effort,
            Some(ReasoningEffort::Medium)
        );
    }

    #[test]
    fn leaven_reflection_prompt_matches_upstream_gepa_instruction_template() {
        const UPSTREAM_GEPA_INSTRUCTION_TEMPLATE: &str = r"I provided an assistant with the following instructions to perform a task for me:
```
<curr_param>
```

The following are examples of different task inputs provided to the assistant along with the assistant's response for each of them, and some feedback on how the assistant's response could be better:
```
<side_info>
```

Your task is to write a new instruction for the assistant.

Read the inputs carefully and identify the input format and infer detailed task description about the task I wish to solve with the assistant.

Read all the assistant responses and the corresponding feedback. Identify all niche and domain specific factual information about the task and include it in the instruction, as a lot of it may not be available to the assistant in the future. The assistant may have utilized a generalizable strategy to solve the task, if so, include that in the instruction as well.

Provide the new instructions within ``` blocks.";

        assert_eq!(
            leaven::gepa::DEFAULT_REFLECTION_PROMPT_TEMPLATE,
            UPSTREAM_GEPA_INSTRUCTION_TEMPLATE
        );
    }

    #[test]
    fn report_lines_include_split_budget_and_case_identity() {
        let config = AimeRunConfig::deterministic_smoke();
        let result = block_on(run_deterministic_aime());
        let lines = report_lines(&config, &result);
        let validation_id = case_id_from_source_id("deterministic:default:validation:0");
        let test_id = case_id_from_source_id("deterministic:default:test:0");

        assert!(lines.iter().any(
            |line| line == "proof_classification=deterministic_mechanics_product_surface_proof"
        ));
        assert!(lines.iter().any(|line| line == "eval_cache_policy=never"));
        assert!(lines.iter().any(|line| line == "run_storage=stored"));
        assert!(lines.iter().any(|line| line == "run_resumable=true"));
        assert!(
            lines
                .iter()
                .any(|line| line == "run_resumability=resumable")
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("run_dir=.leaven/runs/"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("summary_json=.leaven/runs/")
                    && line.ends_with("/reports/summary.json"))
        );
        assert!(lines.iter().any(|line| {
            line.starts_with("compatibility=schema=leaven-run.compatibility.v1")
                && line.contains(" run_kind=leaven-run.optimize ")
                && line.contains(" cache=cache:auto/")
        }));
        assert!(
            lines
                .iter()
                .any(|line| line == "search_metric_call_cap=512")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "search_metric_calls_spent=8")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "final_report_metric_call_cap=unlimited")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "final_report_metric_calls_spent=12")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "optimization_metric_calls=8")
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
            line == "lm_role_cost=reflection calls=1 prompt_tokens=37 cached_input_tokens=0 completion_tokens=11 reasoning_tokens=0 cost_llm_calls=1 cost_prompt_tokens=37 cost_completion_tokens=11"
        }));
        assert!(lines.iter().any(|line| {
            line == "lm_role_cache=reflection hits=0 misses=0 bypasses=1 bypass_policy_never=1 bypass_refresh=0 write_errors=0 hit_cost_zero=true"
        }));
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
        config.budget = Budget::metric_calls(8);
        config.max_iterations = 2;
        let run = block_on(run_aime(config.clone(), deterministic_dataset()));
        let result = &run.optimized;

        assert_eq!(
            result.stop,
            leaven::run::OptimizationStopReason::BudgetReached
        );
        assert_eq!(result.summary.optimization_cost.metric_calls, 8);
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
                .any(|line| line == "optimization_metric_calls=8")
        );
    }

    #[test]
    fn deterministic_metric_budget_refusal_finishes_with_budget_stop() {
        let mut config = AimeRunConfig::deterministic_smoke();
        config.budget = Budget::metric_calls(7);
        config.max_iterations = 2;
        let run = block_on(run_aime(config, deterministic_dataset()));
        let result = &run.optimized;

        assert_eq!(
            result.stop,
            leaven::run::OptimizationStopReason::BudgetReached
        );
        assert_eq!(result.summary.optimization_cost.metric_calls, 7);
        assert!(
            result
                .events
                .contains(&RunEventSummary::OptimizationStopping)
        );
        assert!(
            !result.events.contains(&RunEventSummary::Error),
            "budget refusal at a GEPA step boundary should become a clean stop"
        );
    }

    #[test]
    fn live_cache_policy_parser_defaults_to_read_write_and_keeps_overrides_scaffolded() {
        let policies = AimeLmCachePolicies::from_values(Some("read-write"), Some("refresh"));

        assert_eq!(policies.solver, LmCachePolicy::ReadWrite);
        assert_eq!(policies.reflection, LmCachePolicy::Refresh);
        assert_eq!(
            AimeLmCachePolicies::from_values(None, None).solver,
            LmCachePolicy::ReadWrite
        );
        assert_eq!(
            AimeLmCachePolicies::from_values(Some("auto"), Some("off")).reflection,
            LmCachePolicy::Never
        );
    }

    #[test]
    fn live_openai_runtime_config_defaults_to_sqlite_cache_and_names_provider_throttle() {
        let runtime = AimeOpenAiRuntimeConfig::from_values(Some("8"), None);

        assert_eq!(runtime.max_concurrent_requests.get(), 8);
        assert_eq!(runtime.cache_backend, AimeLmCacheBackend::Sqlite);
        assert!(runtime.cache_backend.is_durable());
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, None)
                .max_concurrent_requests
                .get(),
            GEPA_AIME_MAX_WORKERS
        );
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, Some("in-memory")).cache_backend,
            AimeLmCacheBackend::InMemory
        );
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, Some("eager")).cache_backend,
            AimeLmCacheBackend::EagerSqlite
        );
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, Some("workspace-sqlite")).cache_backend,
            AimeLmCacheBackend::EagerSqlite
        );
    }

    #[test]
    fn report_lines_disclose_live_lm_role_cache_and_runtime_truth() {
        let mut config = AimeRunConfig::deterministic_smoke();
        config.profile = AimeRunProfile::GepaAime;
        config.solver.live = true;
        config.solver.model = "solver-model".to_owned();
        config.solver.cache_policy = LmCachePolicy::ReadWrite;
        config.solver.runtime = AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("sqlite"));
        config.reflection.live = true;
        config.reflection.model = "reflection-model".to_owned();
        config.reflection.cache_policy = LmCachePolicy::Refresh;
        config.reflection.runtime = config.solver.runtime;
        let mut result = block_on(run_deterministic_aime());
        result.role_reports = AimeRoleReports::from_config(
            &config,
            AimeLmRoleMetrics::default(),
            AimeLmRoleMetrics::default(),
        );

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
        assert!(lines.iter().any(|line| line == "lm_cache_backend=sqlite"));
        assert!(lines.iter().any(|line| line == "lm_cache_durable=true"));
        assert!(
            lines.iter().any(
                |line| line.ends_with("/lm-cache.sqlite") && line.starts_with("lm_cache_path=")
            )
        );
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
        assert!(lines.iter().any(|line| {
            line.starts_with(
                "lm_role=solver provider=openai live=true model=solver-model runtime_fingerprint=",
            )
        }));
        assert!(lines.iter().any(|line| {
            line == "lm_role_runtime=solver cache_policy=read-write cache_backend=sqlite cache_durable=true max_concurrent_requests=7 output=answer-text parser=trimmed-answer"
        }));
        assert!(lines.iter().any(|line| {
            line.starts_with(
                "lm_role=reflection provider=openai live=true model=reflection-model runtime_fingerprint=",
            )
        }));
        assert!(lines.iter().any(|line| {
            line == "lm_role_runtime=reflection cache_policy=refresh cache_backend=sqlite cache_durable=true max_concurrent_requests=7 output=text parser=plain-text-fenced"
        }));
    }

    #[test]
    fn p8_live_lm_cache_path_lives_in_run_dir() {
        let run_id = RunId::new();
        let run_dir = leaven::run::default_local_run_dir(run_id);

        assert_eq!(
            SqliteLmCache::path_in_run_dir(&run_dir),
            run_dir.join("lm-cache.sqlite")
        );
        assert_eq!(
            SqliteLmCache::path_in_workspace("."),
            std::path::PathBuf::from(".leaven").join("lm-cache.sqlite")
        );
    }

    #[test]
    fn deterministic_lm_cache_hit_reports_zero_new_cost() {
        let telemetry = AimeLmTelemetry::new(LmCachePolicy::ReadWrite);
        let cached = CachedLm::new(
            DeterministicReflectionLm,
            InMemoryLmCache::default(),
            LmCachePolicy::ReadWrite,
        );
        let lm = AimeInstrumentedLm::new(cached, telemetry.clone());
        let request = LmRequest::new(
            "deterministic-aime-reflector",
            Messages::from_user("incorrect modular feedback"),
        );

        let first = block_on(lm.complete(request.clone())).expect("first LM call succeeds");
        let second = block_on(lm.complete(request)).expect("cached LM call succeeds");

        assert_eq!(first.cost.llm_calls, 1);
        assert_eq!(second.cost, Cost::zero());
        assert_eq!(second.value.usage.input_tokens, 37);
        let metrics = telemetry.snapshot();
        assert_eq!(metrics.calls, 2);
        assert_eq!(metrics.cost.llm_calls, 1);
        assert_eq!(metrics.cache.misses, 1);
        assert_eq!(metrics.cache.hits, 1);
        assert!(metrics.cache.hit_cost_zero);
    }

    #[test]
    fn typed_live_provider_failure_summary_redacts_missing_credentials() {
        let secret = "sk-test-secret";
        let error = LmError::invalid_request(format!("OPENAI_API_KEY is not set; saw {secret}"));
        let summary = AimeProviderFailureSummary::from_lm_error(&error);
        let line = summary.report_line(AimeLmRole::Solver);
        let telemetry = AimeLmTelemetry::new(LmCachePolicy::Never);
        let failure: Result<Metered<LmResponse>, LmError> = Err(error);

        telemetry.record(&failure);

        assert_eq!(summary.kind, AimeProviderFailureKind::MissingCredentials);
        assert_eq!(summary.message, "missing required credential");
        assert!(line.contains("kind=missing_credentials"));
        assert!(!line.contains(secret));
        assert_eq!(telemetry.snapshot().failures.missing_credentials, 1);
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
