#![allow(clippy::too_many_lines)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    fs::OpenOptions,
    io::Write,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use leaven::core::{AssessmentTarget, CacheIdentity, InfoRef};
use leaven::engine::{
    CacheBypassReason, CacheStatus, Callback, ErrorPolicy, RunContext, RunEvent, RunGraphView,
};
use leaven::eval::{Case, SplitRole};
use leaven::gepa::{
    Gepa, GepaCandidateIndex, GepaEventSummary, GepaOptimizedExt, GepaProfile, GepaProposalAttempt,
    GepaReport, GepaSkipReason, ReflectionError, ReflectiveCase, ReflectiveDatasetBuilder,
    ReflectiveSideInfoValue, ReflectiveValue,
};
use leaven::kernel::Metered;
use leaven::kernel::{
    AssessmentId, CandidateId, CaseId, Cost, ErrorKind, FingerprintBuilder, MetadataValue, RunId,
};
use leaven::plumbing::{ContentId, Fingerprint, FiniteF64, MetadataBag};
use leaven::prelude::{
    Artifact, ArtifactIdentity, Budget, EditSurface, Optimized, Part, PartAddress, RunOutput,
    Score, ScoreContext, ScoreError, SurfaceError, SurfaceFingerprint,
};
use leaven::run::{
    CachePolicy, OptimizeError, ResumeCompatibilityError, RunCase, RunError, RunProblem,
    RunResumability, RunStorage, RuntimeFingerprint,
};
use leaven::stdlib::evidence::OutputRecord;
use leaven_gepa::{
    DefaultReflectionRenderer, LmBackedReflectorConfig, ReflectRequest, ReflectionRenderInput,
    ReflectionRenderer,
};
use leaven_lm::{
    JsonSchemaOutput, Lm, LmError, LmId, LmRequest, LmResponse, Message, Messages, OutputMode,
    ReasoningEffort, Role, SamplingOptions, TokenUsage,
};
use leaven_lm_cache::{
    CachedLm, InMemoryLmCache, LmCacheEntry, LmCacheError, LmCacheKey, LmCachePolicy, LmCacheStore,
    SqliteLmCache,
};
#[cfg(test)]
use leaven_lm_openai::OpenAiRetryPolicy;
use leaven_lm_openai::{OpenAiConfig, OpenAiLm, OpenAiThrottlePolicy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const BASELINE: &str = "Solve the math problem carefully. Break down the steps and provide the final answer as a single number.";
const OPTIMIZED: &str = "Solve with modular arithmetic when useful. Verify arithmetic before the final answer. Provide only the final integer.";
const GEPA_AIME_METRIC_CALLS: u64 = 500;
const DSPY_QUICKSTART_METRIC_CALLS: u64 = 150;
const DSPY_QUICKSTART_TEST_SCORE_TARGET: f64 = 0.566;
const GEPA_CAIS_AIME_PUBLISHED_TEST_SCORE: f64 = 0.600;
const GEPA_CAIS_AIME_PUBLISHED_VALIDATION_SCORE: f64 = 26.0 / 45.0;
const GEPA_CAIS_AIME_CONFIGURED_SEARCH_CAP: u64 = 500;
const GEPA_CAIS_AIME_CHECKPOINT_METRIC_CALLS: u64 = 621;
const GEPA_CAIS_AIME_CHECKPOINT_CANDIDATES: u64 = 10;
const GEPA_AIME_MAX_WORKERS: usize = 32;
const GEPA_AIME_MAX_OUTPUT_TOKENS: u32 = 32_000;
// GEPA AIME is controlled by max_metric_calls, not max_iterations. This is a
// Leaven-local safety ceiling; the public metric-call budget is the stop control.
const GEPA_AIME_INTERNAL_ITERATION_CEILING: usize = 500;
const GEPA_AIME_SOLVER_MODEL: &str = "gpt-4.1-mini";
const GEPA_AIME_REFLECTION_MODEL: &str = "gpt-5.4-mini";
const UPSTREAM_GEPA_AIME_REFLECTION_MODEL: &str = "openai/gpt-5.1";
const DETERMINISTIC_SOLVER_MODEL: &str = "deterministic-aime-solver";
const LEAVEN_AIME_SOLVER_CACHE_POLICY: &str = "LEAVEN_AIME_SOLVER_CACHE_POLICY";
const LEAVEN_AIME_REFLECTION_CACHE_POLICY: &str = "LEAVEN_AIME_REFLECTION_CACHE_POLICY";
const LEAVEN_AIME_LM_CACHE_BACKEND: &str = "LEAVEN_AIME_LM_CACHE_BACKEND";
const LEAVEN_AIME_PROFILE: &str = "LEAVEN_AIME_PROFILE";
const LEAVEN_AIME_GEPA_PROFILE: &str = "LEAVEN_AIME_GEPA_PROFILE";
const LEAVEN_AIME_RUN_DIR: &str = "LEAVEN_AIME_RUN_DIR";
const LEAVEN_AIME_DETERMINISTIC_REFLECTION: &str = "LEAVEN_AIME_DETERMINISTIC_REFLECTION";
const LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS: &str = "LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS";
const LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS: &str = "LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS";
const GEPA_AIME_OPENAI_REQUEST_TIMEOUT_SECONDS: u64 = 120;
const DETERMINISTIC_SMOKE_METRIC_CALLS: u64 = 512;
const DETERMINISTIC_SMOKE_ITERATIONS: usize = 1;
const OPTIMIZE_ANYTHING_REFLECTION_PROMPT_TEMPLATE: &str = r"I am optimizing a parameter in my system. The current parameter value is:
```
<curr_param>
```

Below is evaluation data showing how this parameter value performed across multiple test cases. The data contains performance metrics, diagnostic information, and other relevant details from the evaluation:
```
<side_info>
```

Your task is to propose a new, improved parameter value that can be used as a drop-in replacement for the current one.

Carefully analyze all the evaluation data provided above. Look for patterns that indicate what works and what doesn't. Pay special attention to:
- Performance metrics and how they correlate with parameter behavior
- Recurring issues, errors, or failure patterns across multiple test cases
- Successful patterns or behaviors that should be preserved or enhanced
- Any domain-specific requirements, constraints, or factual information revealed in the evaluation data
- Specific technical details that are crucial for understanding the parameter's role

Based on your analysis, propose a new parameter value that addresses the identified issues while maintaining or improving upon what works well. Your proposal should be directly informed by the patterns and insights from the evaluation data.

Provide the new parameter value within ``` blocks.";

#[tokio::main(flavor = "multi_thread", worker_threads = 32)]
async fn main() {
    let started_at = Instant::now();
    let config = AimeRunConfig::configured();
    let result = match write_p8_aime_start_report(&config, SystemTime::now()) {
        Ok(_) => Box::pin(try_run_configured_aime(config.clone())).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(result) => {
            for line in report_lines(&config, &result) {
                println!("{line}");
            }
        }
        Err(error) => {
            let wall_time = started_at.elapsed();
            let failure_report = write_p8_aime_failure_report(&config, &error, wall_time);
            for line in error_report_lines(&config, &error, wall_time) {
                eprintln!("{line}");
            }
            match failure_report {
                Ok(Some(path)) => eprintln!("p8_aime_failure_json={}", path.display()),
                Ok(None) => {}
                Err(report_error) => eprintln!("p8_aime_failure_json_error={report_error}"),
            }
            std::process::exit(1);
        }
    }
}

fn error_report_lines(
    config: &AimeRunConfig,
    error: &(dyn std::error::Error + 'static),
    wall_time: Duration,
) -> Vec<String> {
    let mut lines = vec![format!("p8_aime_gepa_failed={error}")];
    lines.extend(error_report_context_lines(config));
    lines.push(format!(
        "p8_aime_gepa_failed_wall_time_ms={}",
        wall_time.as_millis()
    ));
    for (index, source) in
        std::iter::successors(error.source(), |source| source.source()).enumerate()
    {
        lines.push(format!(
            "p8_aime_gepa_failure_source_{}={source}",
            index + 1
        ));
    }
    if let Some(line) = p8_failure_compatibility_line(error) {
        lines.push(line);
    }
    lines
}

fn error_report_context_lines(config: &AimeRunConfig) -> Vec<String> {
    vec![
        format!("run_profile={}", config.profile.label()),
        format!("gepa_profile={}", config.gepa_profile.label()),
        format!("data_source={}", config.data_source.label()),
        format!(
            "proof_classification={}",
            proof_classification_for_config(config)
        ),
        format!(
            "run_dir={}",
            config
                .run_dir
                .as_ref()
                .map_or_else(|| "auto".to_owned(), |path| path.display().to_string())
        ),
        format!(
            "solver_runtime=live={} model={} cache_policy={} cache_backend={} cache_durable={} max_concurrent_requests={} request_timeout_seconds={}",
            config.solver.live,
            config.solver.model,
            report_lm_cache_policy(config.solver.cache_policy),
            report_lm_cache_backend(config.solver.runtime.cache_backend),
            config.solver.runtime.cache_backend.is_durable(),
            config.solver.runtime.max_concurrent_requests,
            config.solver.runtime.request_timeout_seconds
        ),
        format!(
            "reflection_runtime=live={} model={} cache_policy={} cache_backend={} cache_durable={} max_concurrent_requests={} request_timeout_seconds={}",
            config.reflection.live,
            config.reflection.model,
            report_lm_cache_policy(config.reflection.cache_policy),
            report_lm_cache_backend(config.reflection.runtime.cache_backend),
            config.reflection.runtime.cache_backend.is_durable(),
            config.reflection.runtime.max_concurrent_requests,
            config.reflection.runtime.request_timeout_seconds
        ),
    ]
}

fn report_lines(config: &AimeRunConfig, run: &AimeRunResult) -> Vec<String> {
    let result = &run.optimized;
    let mut lines = report_run_header_lines(config, run);
    lines.extend(report_score_lines(result));
    lines.extend(report_runtime_lines(config, run));
    lines.extend(report_budget_and_cache_lines(config, result));
    lines.extend(report_best_and_event_lines(result));
    lines.extend(report_gepa_lines(run));
    for role in run.role_reports.iter() {
        lines.extend(report_lm_role_lines(role));
    }
    lines.extend(report_case_lines(run));
    lines
}

fn report_run_header_lines(config: &AimeRunConfig, run: &AimeRunResult) -> Vec<String> {
    let result = &run.optimized;
    let mut lines = vec![
        format!("run_profile={}", config.profile.label()),
        format!("gepa_profile={}", config.gepa_profile.label()),
        format!(
            "proof_classification={}",
            proof_classification_for_report(config, &run.role_reports)
        ),
        format!("comparison_target={}", config.profile.comparison_target()),
        format!(
            "comparison_published_test_score={}",
            report_score(config.profile.published_test_score())
        ),
        format!(
            "comparison_published_validation_score={}",
            report_score(config.profile.published_validation_score())
        ),
        format!(
            "comparison_upstream_configured_search_metric_call_cap={}",
            report_optional_u64_value(config.profile.upstream_configured_search_metric_call_cap())
        ),
        format!(
            "comparison_upstream_checkpoint_metric_calls={}",
            report_optional_u64_value(config.profile.upstream_checkpoint_metric_calls())
        ),
        format!(
            "comparison_upstream_checkpoint_candidate_count={}",
            report_optional_u64_value(config.profile.upstream_checkpoint_candidate_count())
        ),
        format!(
            "comparison_upstream_run_log_available={}",
            report_optional_bool(config.profile.upstream_run_log_available())
        ),
        format!(
            "comparison_reflection_prompt={}",
            config.profile.reflection_prompt_claim()
        ),
        format!(
            "comparison_upstream_reflection_model={}",
            config.profile.upstream_reflection_model()
        ),
        format!(
            "comparison_leaven_reflection_model={}",
            config.reflection.model
        ),
        format!(
            "comparison_reflection_model_alignment={}",
            config
                .profile
                .reflection_model_alignment(&config.reflection.model)
        ),
        format!("data_source={}", config.data_source.label()),
        format!("seed_system_prompt={}", config.seed_prompt),
        format!("aime_train_count={}", run.dataset_proof.train_count),
        format!(
            "aime_validation_count={}",
            run.dataset_proof.validation_count
        ),
        format!("aime_test_count={}", run.dataset_proof.test_count),
        format!(
            "aime_split_seed={}",
            run.dataset_proof
                .split_seed
                .map(|seed| seed.to_string())
                .unwrap_or_else(|| "none".to_owned())
        ),
        format!("aime_test_repeated={}", run.dataset_proof.test_repeated),
        format!(
            "aime_cache_hash={}",
            run.dataset_proof
                .materialized_cache
                .as_ref()
                .map(|cache| cache.sha256.as_str())
                .unwrap_or("none")
        ),
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
        format!(
            "p8_aime_json={}",
            p8_aime_report_path(&result.summary.storage)
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_owned())
        ),
        format!("compatibility={}", report_compatibility(result)),
    ];
    lines.extend(
        config
            .profile
            .comparison_notes(&config.reflection.model)
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

fn report_runtime_lines(config: &AimeRunConfig, run: &AimeRunResult) -> Vec<String> {
    let result = &run.optimized;
    vec![
        format!(
            "optimizer_wall_time_ms={}",
            run.optimizer_wall_time.as_millis()
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
            "lm_cache_path={}",
            report_lm_cache_path(config.solver.runtime.cache_backend, &result.summary.storage)
        ),
        format!(
            "lm_cache_read_paths={}",
            report_lm_cache_read_paths_line(
                config.solver.runtime.cache_backend,
                &result.summary.storage
            )
        ),
        format!(
            "lm_cache_write_path={}",
            report_lm_cache_write_path(
                config.solver.runtime.cache_backend,
                &result.summary.storage
            )
        ),
        format!(
            "openai_max_concurrent_requests={}",
            config.solver.runtime.max_concurrent_requests
        ),
        format!(
            "openai_request_timeout_seconds={}",
            config.solver.runtime.request_timeout_seconds
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
        format!(
            "search_metric_calls_overshoot={}",
            metric_calls_overshoot(
                config.budget.metric_calls,
                result.summary.optimization_cost.metric_calls,
            )
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

fn report_gepa_lines(run: &AimeRunResult) -> Vec<String> {
    let Some(report) = &run.gepa_report else {
        return vec!["gepa_report=unavailable".to_owned()];
    };
    let (accepted, accepted_unadmitted) = gepa_attempt_counts(report);
    vec![
        "gepa_report=available".to_owned(),
        format!(
            "gepa_best_index={}",
            report
                .best_index
                .map(|index| index.get().to_string())
                .unwrap_or_else(|| "none".to_owned())
        ),
        format!(
            "gepa_validation_best_index={}",
            report
                .validation_best_index
                .map(|index| index.get().to_string())
                .unwrap_or_else(|| "none".to_owned())
        ),
        format!("gepa_candidate_count={}", report.candidates.len()),
        format!(
            "gepa_proposal_attempt_count={}",
            report.proposal_attempts.len()
        ),
        format!("gepa_accepted_count={accepted}"),
        format!("gepa_accepted_unadmitted_count={accepted_unadmitted}"),
        format!(
            "gepa_full_validation_evals={}",
            report.full_validation_evals
        ),
        format!("gepa_search_metric_calls={}", report.total_metric_calls),
    ]
}

fn gepa_attempt_counts(report: &GepaReport) -> (usize, usize) {
    let accepted = report
        .proposal_attempts
        .iter()
        .filter(|attempt| attempt.accepted == Some(true))
        .count();
    let accepted_unadmitted = report
        .proposal_attempts
        .iter()
        .filter(|attempt| attempt.accepted == Some(true) && attempt.admitted_index.is_none())
        .count();
    (accepted, accepted_unadmitted)
}

fn report_case_lines(run: &AimeRunResult) -> Vec<String> {
    let mut lines = Vec::new();
    for split in &run.optimized.summary.evaluation.splits_reported {
        for (candidate_index, candidate) in split.candidates.iter().enumerate() {
            let candidate_role = p8_candidate_report_role(candidate_index);
            for case in &candidate.cases {
                let source_id = run
                    .report_metadata
                    .get(&case.case_id)
                    .map(AimeReportMetadata::source_id)
                    .unwrap_or_else(|| "missing-source-id".to_owned());
                lines.push(format!(
                    "report_case={} source_id={} split={:?} candidate_role={} score_state=present score={:.3} output_ref={} feedback_ref={} trace_refs={} output_chars={} feedback_chars={}",
                    case.case_id,
                    source_id,
                    split.role,
                    candidate_role,
                    case.score,
                    case.output_ref
                        .as_ref()
                        .map(evidence_ref_text)
                        .unwrap_or_else(|| "none".to_owned()),
                    case.feedback_ref
                        .as_ref()
                        .map(evidence_ref_text)
                        .unwrap_or_else(|| "none".to_owned()),
                    case.trace_refs
                        .iter()
                        .map(evidence_ref_text)
                        .collect::<Vec<_>>()
                        .join(","),
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

fn p8_aime_report_path(storage: &RunStorage) -> Option<PathBuf> {
    match storage {
        RunStorage::Stored {
            run_dir: Some(run_dir),
            ..
        } => Some(run_dir.join("reports").join("p8-aime.json")),
        RunStorage::Stored { .. } | RunStorage::Ephemeral { .. } => None,
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

fn report_optional_u64_value(value: Option<u64>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.to_string())
}

fn report_optional_bool(value: Option<bool>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.to_string())
}

fn metric_calls_overshoot(cap: Option<u64>, spent: u64) -> u64 {
    cap.map_or(0, |cap| spent.saturating_sub(cap))
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
            "lm_role_runtime={} cache_policy={} cache_backend={} cache_durable={} max_concurrent_requests={} request_timeout_seconds={} output={} parser={}",
            role.role.label(),
            report_lm_cache_policy(role.cache_policy),
            report_lm_cache_backend(role.cache_backend),
            role.cache_durable,
            role.max_concurrent_requests,
            role.request_timeout_seconds,
            role.output,
            role.parser
        ),
        format!(
            "lm_role_prompt_contract={} renderer={} upstream={} request_shape_fingerprint={}",
            role.role.label(),
            role.prompt_contract.renderer,
            role.prompt_contract.upstream,
            report_full_fingerprint(role.prompt_contract.request_shape_fingerprint)
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
            "lm_role_cache={} hits={} misses={} bypasses={} bypass_policy_never={} bypass_refresh={} required_misses={} read_errors={} write_errors={} other_errors={} hit_cost_zero={}",
            role.role.label(),
            role.metrics.cache.hits,
            role.metrics.cache.misses,
            role.metrics.cache.bypasses(),
            role.metrics.cache.bypass_policy_never,
            role.metrics.cache.bypass_refresh,
            role.metrics.cache.required_misses,
            role.metrics.cache.read_errors,
            role.metrics.cache.write_errors,
            role.metrics.cache.other_errors,
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
        format!(
            "lm_role_durable_failures={} scope=run_dir_jsonl count={} missing_credentials={} authentication={} rate_limit={} retry_exhausted={} malformed_provider_response={} answer_parse={} scorer_parse={} budget_refusal={} cache={} transport={} provider={} unknown={}",
            role.role.label(),
            role.durable_failures.total(),
            role.durable_failures.missing_credentials,
            role.durable_failures.authentication,
            role.durable_failures.rate_limit,
            role.durable_failures.retry_exhausted,
            role.durable_failures.malformed_provider_response,
            role.durable_failures.answer_parse,
            role.durable_failures.scorer_parse,
            role.durable_failures.budget_refusal,
            role.durable_failures.cache,
            role.durable_failures.transport,
            role.durable_failures.provider,
            role.durable_failures.unknown
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

fn report_full_fingerprint(fingerprint: Fingerprint) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in &fingerprint.0 {
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
        LmCachePolicy::CacheOnly => "cache-only",
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

fn report_lm_cache_read_paths(backend: AimeLmCacheBackend, storage: &RunStorage) -> Vec<String> {
    match backend {
        AimeLmCacheBackend::InMemory => Vec::new(),
        AimeLmCacheBackend::Sqlite => match storage {
            RunStorage::Stored {
                run_dir: Some(run_dir),
                ..
            } => vec![
                SqliteLmCache::path_in_run_dir(run_dir)
                    .display()
                    .to_string(),
            ],
            RunStorage::Stored { .. } | RunStorage::Ephemeral { .. } => Vec::new(),
        },
        AimeLmCacheBackend::EagerSqlite => {
            let mut paths = Vec::new();
            if let RunStorage::Stored {
                run_dir: Some(run_dir),
                ..
            } = storage
            {
                paths.push(
                    SqliteLmCache::path_in_run_dir(run_dir)
                        .display()
                        .to_string(),
                );
            }
            paths.push(SqliteLmCache::path_in_workspace(".").display().to_string());
            paths
        }
    }
}

fn report_lm_cache_read_paths_line(backend: AimeLmCacheBackend, storage: &RunStorage) -> String {
    let paths = report_lm_cache_read_paths(backend, storage);
    if paths.is_empty() {
        "none".to_owned()
    } else {
        paths.join(";")
    }
}

fn report_lm_cache_write_path(backend: AimeLmCacheBackend, storage: &RunStorage) -> String {
    match backend {
        AimeLmCacheBackend::InMemory => "none".to_owned(),
        AimeLmCacheBackend::Sqlite => report_lm_cache_path(backend, storage),
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
    let optimizer_started_at = Instant::now();
    let run_id = RunId::new();
    let run_dir = config
        .run_dir
        .clone()
        .unwrap_or_else(|| leaven::run::default_local_run_dir(run_id));
    let provider_failures_path = aime_provider_failures_path(&run_dir);
    let solver_telemetry = AimeLmTelemetry::new(config.solver.cache_policy)
        .with_durable_provider_failures(AimeLmRole::Solver, provider_failures_path.clone());
    let reflection_telemetry = AimeLmTelemetry::new(config.reflection.cache_policy)
        .with_durable_provider_failures(AimeLmRole::Reflection, provider_failures_path.clone());
    let solver = aime_solver_lm(&config.solver, solver_telemetry.clone(), &run_dir);
    let solver_lm_fingerprint = solver.as_ref().map(Lm::fingerprint);
    let runner_fingerprint = aime_runner_fingerprint(&config.solver, solver_lm_fingerprint);
    let scorer_fingerprint = aime_scorer_fingerprint();
    let reflection_lm =
        aime_reflection_lm(&config.reflection, reflection_telemetry.clone(), &run_dir);
    let reflection_role_fingerprint =
        aime_reflection_role_fingerprint(&config.reflection, reflection_lm.fingerprint());
    let solver_config = config.solver.clone();
    let side_infos = AimeSolverSideInfoStore::default();
    let gepa_events = Arc::new(Mutex::new(Vec::<GepaEventSummary>::new()));
    let gepa_progress = Arc::new(Mutex::new(AimeGepaProgress::default()));
    let report_metadata = dataset.report_metadata.clone();
    let dataset_proof = dataset.proof.clone();
    let reflective_dataset = dataset.reflective_dataset(side_infos.clone());
    let gepa_event_sink = gepa_events.clone();
    let gepa_progress_sink = gepa_progress.clone();
    let optimized = Box::pin(
        leaven::prelude::optimize(AimePrompt::new(config.seed_prompt))
            .train(dataset.train)
            .validation(dataset.validation)
            .test(dataset.test)
            .runner(move |prompt, case| {
                let solver = solver.clone();
                let solver_config = solver_config.clone();
                let side_infos = side_infos.clone();
                async move { run_solver(prompt, case, solver, solver_config, side_infos).await }
            })
            .score(score_answer)
            .runner_fingerprint(runner_fingerprint)
            .scorer_fingerprint(scorer_fingerprint)
            .lm_role_fingerprint("solver", runner_fingerprint)
            .lm_role_fingerprint("reflection", reflection_role_fingerprint)
            .evaluation_cache_policy(config.evaluation_cache_policy.clone())
            .evaluation_parallelism(config.evaluation_parallelism)
            .on_event(AimeProgressCallback::default())
            .using(
                Gepa::reflect_with_lm(reflection_lm, config.reflection.model.clone())
                    .with_reflector_config(aime_reflector_config(&config.reflection))
                    .surface(AimePromptSurface)
                    .build()
                    .with_profile(config.gepa_profile)
                    .on_event(move |event| {
                        gepa_event_sink
                            .lock()
                            .expect("AIME GEPA event sink lock")
                            .push(event.clone());
                        let progress_line = gepa_progress_sink
                            .lock()
                            .expect("AIME GEPA progress lock")
                            .progress_line(event);
                        if let Some(line) = progress_line {
                            eprintln!("{line}");
                        }
                    })
                    .reflective_dataset(reflective_dataset)
                    .max_iterations(config.max_iterations),
            )
            .budget(config.budget.clone())
            .run_id(run_id)
            .run_dir(run_dir)
            .run(),
    )
    .await?;
    let optimizer_wall_time = optimizer_started_at.elapsed();
    let gepa_report = optimized.gepa_report().cloned();
    let role_reports = AimeRoleReports::from_config(
        &config,
        AimeRoleRuntimeFingerprints {
            solver: runner_fingerprint,
            reflection: reflection_role_fingerprint,
        },
        solver_telemetry.snapshot(),
        reflection_telemetry.snapshot(),
    )
    .with_durable_failures(AimeDurableProviderFailures::read(&provider_failures_path));
    let result = AimeRunResult {
        optimized,
        report_metadata,
        dataset_proof,
        role_reports,
        optimizer_wall_time,
        gepa_events: gepa_events
            .lock()
            .expect("AIME GEPA event sink lock")
            .clone(),
        gepa_report,
    };
    write_p8_aime_report(&config, &result)?;
    Ok(result)
}

fn write_p8_aime_report(
    config: &AimeRunConfig,
    run: &AimeRunResult,
) -> Result<(), leaven::run::OptimizeError> {
    let Some(path) = p8_aime_report_path(&run.optimized.summary.storage) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| {
            leaven::run::OptimizeError::ReportStore {
                operation: "create P8 AIME report directory",
                source,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&p8_aime_report_json(config, run))
        .expect("P8 AIME report JSON serializes");
    write_p8_report_atomic(&path, &bytes, "write P8 AIME report json")
}

fn write_p8_aime_failure_report(
    config: &AimeRunConfig,
    error: &(dyn std::error::Error + 'static),
    wall_time: Duration,
) -> Result<Option<PathBuf>, leaven::run::OptimizeError> {
    let Some(path) = p8_aime_failure_report_path(config) else {
        return Ok(None);
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| {
            leaven::run::OptimizeError::ReportStore {
                operation: "create P8 AIME failure report directory",
                source,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&p8_aime_failure_report_json(config, error, wall_time))
        .expect("P8 AIME failure report JSON serializes");
    write_p8_report_atomic(&path, &bytes, "write P8 AIME failure report json")?;
    Ok(Some(path))
}

fn write_p8_aime_start_report(
    config: &AimeRunConfig,
    started_at: SystemTime,
) -> Result<Option<PathBuf>, leaven::run::OptimizeError> {
    let Some(path) = p8_aime_start_report_path(config) else {
        return Ok(None);
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| {
            leaven::run::OptimizeError::ReportStore {
                operation: "create P8 AIME start report directory",
                source,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&p8_aime_start_report_json(config, started_at))
        .expect("P8 AIME start report JSON serializes");
    write_p8_report_atomic(&path, &bytes, "write P8 AIME start report json")?;
    Ok(Some(path))
}

fn p8_aime_failure_report_path(config: &AimeRunConfig) -> Option<PathBuf> {
    config
        .run_dir
        .as_ref()
        .map(|run_dir| run_dir.join("reports").join("p8-aime-failure.json"))
}

fn p8_aime_start_report_path(config: &AimeRunConfig) -> Option<PathBuf> {
    config
        .run_dir
        .as_ref()
        .map(|run_dir| run_dir.join("reports").join("p8-aime-start.json"))
}

fn write_p8_report_atomic(
    path: &Path,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), leaven::run::OptimizeError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| leaven::run::OptimizeError::ReportStore {
            operation,
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name"),
        })?;
    let temp = path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = write_p8_report_atomic_inner(path, &temp, parent, bytes, operation);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_p8_report_atomic_inner(
    path: &Path,
    temp: &Path,
    parent: &Path,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), leaven::run::OptimizeError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp)
        .map_err(|source| leaven::run::OptimizeError::ReportStore { operation, source })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| leaven::run::OptimizeError::ReportStore { operation, source })?;
    drop(file);
    fs::rename(temp, path)
        .map_err(|source| leaven::run::OptimizeError::ReportStore { operation, source })?;
    let dir = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(|source| leaven::run::OptimizeError::ReportStore { operation, source })?;
    dir.sync_all()
        .map_err(|source| leaven::run::OptimizeError::ReportStore { operation, source })
}

fn p8_aime_report_json(config: &AimeRunConfig, run: &AimeRunResult) -> serde_json::Value {
    let result = &run.optimized;
    serde_json::json!({
        "schema": "leaven.p8_aime.report.v1",
        "run_profile": config.profile.label(),
        "gepa_profile": config.gepa_profile.label(),
        "proof_classification": proof_classification_for_report(config, &run.role_reports),
        "comparison_target": config.profile.comparison_target(),
        "comparison_published_test_score": config.profile.published_test_score(),
        "comparison_published_validation_score": config.profile.published_validation_score(),
        "comparison_reflection_prompt": config.profile.reflection_prompt_claim(),
        "comparison": p8_comparison_json(config),
        "data_source": config.data_source.label(),
        "dataset": p8_dataset_proof_json(&run.dataset_proof),
        "seed": {
            "system_prompt": config.seed_prompt,
        },
        "run": {
            "id": result.run_id.to_string(),
            "optimizer_wall_time_ms": run.optimizer_wall_time.as_millis(),
            "storage": report_run_storage(&result.summary.storage),
            "resumable": result.summary.storage.is_resumable(),
            "resumability": report_resumability(&result.summary.storage),
            "run_dir": report_run_dir(&result.summary.storage),
            "latest_checkpoint": report_latest_checkpoint(&result.summary.storage),
            "summary_json": result.summary.reports.summary_json.as_ref().map(|path| path.display().to_string()),
        },
        "scores": {
            "baseline_train": result.summary.baseline_train_score,
            "optimized_train": result.summary.optimized_train_score,
            "baseline_validation": result.summary.baseline_validation_score,
            "validation": result.summary.validation_score,
            "baseline_test": result.summary.baseline_test_score,
            "test": result.summary.test_score,
            "test_score_use": "final_report_only",
        },
        "budget": {
            "stop_reason": report_stop_reason(result.stop),
            "search_metric_call_cap": config.budget.metric_calls,
            "search_metric_calls_spent": result.summary.optimization_cost.metric_calls,
            "search_metric_calls_overshoot": metric_calls_overshoot(
                config.budget.metric_calls,
                result.summary.optimization_cost.metric_calls,
            ),
            "final_report_metric_call_cap": serde_json::Value::String("unlimited".to_owned()),
            "final_report_metric_calls_spent": result.summary.final_report_cost.metric_calls,
            "total_metric_calls": result.budget.spent.metric_calls,
            "total_lm_calls": result.budget.spent.llm_calls,
        },
        "cache": {
            "evaluation": {
                "policy": report_evaluation_cache_policy(&config.evaluation_cache_policy),
                "backend": result.summary.cache.evaluation.backend.as_str(),
                "durable": result.summary.cache.evaluation.durable,
                "hits": result.summary.cache.evaluation.hits,
                "misses": result.summary.cache.evaluation.misses,
                "bypasses": evaluation_cache_bypass_count(&result.summary.cache.evaluation),
                "bypass_reasons": report_evaluation_cache_bypasses(&result.summary.cache.evaluation),
                "write_errors": result.summary.cache.evaluation.write_errors,
                "hit_cost_zero": result.summary.cache.evaluation.hit_cost_zero,
            },
            "lm_backend": report_lm_cache_backend(config.solver.runtime.cache_backend),
            "lm_durable": config.solver.runtime.cache_backend.is_durable(),
            "lm_path": report_lm_cache_path(config.solver.runtime.cache_backend, &result.summary.storage),
            "lm_read_paths": report_lm_cache_read_paths(config.solver.runtime.cache_backend, &result.summary.storage),
            "lm_write_path": report_lm_cache_write_path(config.solver.runtime.cache_backend, &result.summary.storage),
        },
        "best": {
            "system_prompt": result.best().map(|best| best.system.clone()),
        },
        "lm_roles": run.role_reports.iter().map(p8_lm_role_report_json).collect::<Vec<_>>(),
        "live_provider_proof": p8_live_provider_proof_json(&run.role_reports),
        "provider_failures": p8_provider_failures_json(&run.role_reports),
        "gepa_events": p8_gepa_events_for_report(run)
            .iter()
            .map(p8_gepa_event_json)
            .collect::<Vec<_>>(),
        "gepa_report": run
            .gepa_report
            .as_ref()
            .map(|report| p8_gepa_report_json(report, &run.role_reports, config.seed_prompt)),
        "cases": p8_case_report_json(run),
        "case_deltas": p8_case_delta_report_json(run),
        "events": result.events.iter().map(|event| event.as_str()).collect::<Vec<_>>(),
    })
}

fn p8_comparison_json(config: &AimeRunConfig) -> serde_json::Value {
    serde_json::json!({
        "target": config.profile.comparison_target(),
        "published_test_score": config.profile.published_test_score(),
        "published_validation_score": config.profile.published_validation_score(),
        "upstream_configured_search_metric_call_cap": config
            .profile
            .upstream_configured_search_metric_call_cap(),
        "upstream_checkpoint_metric_calls": config.profile.upstream_checkpoint_metric_calls(),
        "upstream_checkpoint_candidate_count": config.profile.upstream_checkpoint_candidate_count(),
        "upstream_run_log_available": config.profile.upstream_run_log_available(),
        "reflection_prompt": config.profile.reflection_prompt_claim(),
        "upstream_reflection_model": config.profile.upstream_reflection_model(),
        "leaven_reflection_model": config.reflection.model,
        "reflection_model_alignment": config
            .profile
            .reflection_model_alignment(&config.reflection.model),
        "notes": config.profile.comparison_notes(&config.reflection.model),
    })
}

fn p8_failure_compatibility_line(error: &(dyn std::error::Error + 'static)) -> Option<String> {
    let compatibility = p8_resume_compatibility_error(error)?;
    Some(match compatibility {
        ResumeCompatibilityError::RunnerFingerprintMismatch { stored, live } => format!(
            "resume_compatibility_mismatch=runner stored={} live={}",
            report_runtime_fingerprint(*stored),
            report_runtime_fingerprint(*live)
        ),
        ResumeCompatibilityError::ScorerFingerprintMismatch { stored, live } => format!(
            "resume_compatibility_mismatch=scorer stored={} live={}",
            report_runtime_fingerprint(*stored),
            report_runtime_fingerprint(*live)
        ),
        ResumeCompatibilityError::EvaluatorFingerprintMismatch { stored, live } => format!(
            "resume_compatibility_mismatch=evaluator stored={} live={}",
            report_runtime_fingerprint(*stored),
            report_runtime_fingerprint(*live)
        ),
        ResumeCompatibilityError::LmRoleFingerprintMismatch { role, stored, live } => format!(
            "resume_compatibility_mismatch=lm-role role={} stored={} live={}",
            role,
            stored
                .as_ref()
                .copied()
                .map(report_runtime_fingerprint)
                .unwrap_or_else(|| "none".to_owned()),
            live.as_ref()
                .copied()
                .map(report_runtime_fingerprint)
                .unwrap_or_else(|| "none".to_owned())
        ),
        ResumeCompatibilityError::DatasetFingerprintMismatch { .. } => {
            "resume_compatibility_mismatch=dataset".to_owned()
        }
        ResumeCompatibilityError::SchemaMismatch { stored, live } => {
            format!("resume_compatibility_mismatch=schema stored={stored} live={live}")
        }
        ResumeCompatibilityError::OptimizerCompatibilityMismatch { .. } => {
            "resume_compatibility_mismatch=optimizer".to_owned()
        }
        ResumeCompatibilityError::CacheCompatibilityMismatch => {
            "resume_compatibility_mismatch=cache".to_owned()
        }
        ResumeCompatibilityError::BudgetPolicyMismatch => {
            "resume_compatibility_mismatch=budget".to_owned()
        }
        ResumeCompatibilityError::Read { path, .. } => {
            format!(
                "resume_compatibility_mismatch=manifest-read path={}",
                path.display()
            )
        }
        ResumeCompatibilityError::Decode { path, .. } => {
            format!(
                "resume_compatibility_mismatch=manifest-decode path={}",
                path.display()
            )
        }
    })
}

fn p8_failure_compatibility_json(error: &(dyn std::error::Error + 'static)) -> serde_json::Value {
    let Some(compatibility) = p8_resume_compatibility_error(error) else {
        return serde_json::Value::Null;
    };
    match compatibility {
        ResumeCompatibilityError::RunnerFingerprintMismatch { stored, live } => {
            runtime_compatibility_json("runner", *stored, *live)
        }
        ResumeCompatibilityError::ScorerFingerprintMismatch { stored, live } => {
            runtime_compatibility_json("scorer", *stored, *live)
        }
        ResumeCompatibilityError::EvaluatorFingerprintMismatch { stored, live } => {
            runtime_compatibility_json("evaluator", *stored, *live)
        }
        ResumeCompatibilityError::LmRoleFingerprintMismatch { role, stored, live } => {
            serde_json::json!({
                "kind": "lm-role",
                "role": role,
                "stored": stored.as_ref().copied().map(report_runtime_fingerprint),
                "live": live.as_ref().copied().map(report_runtime_fingerprint),
            })
        }
        ResumeCompatibilityError::DatasetFingerprintMismatch { .. } => {
            serde_json::json!({ "kind": "dataset" })
        }
        ResumeCompatibilityError::SchemaMismatch { stored, live } => {
            serde_json::json!({ "kind": "schema", "stored": stored, "live": live })
        }
        ResumeCompatibilityError::OptimizerCompatibilityMismatch { .. } => {
            serde_json::json!({ "kind": "optimizer" })
        }
        ResumeCompatibilityError::CacheCompatibilityMismatch => {
            serde_json::json!({ "kind": "cache" })
        }
        ResumeCompatibilityError::BudgetPolicyMismatch => {
            serde_json::json!({ "kind": "budget" })
        }
        ResumeCompatibilityError::Read { path, .. } => {
            serde_json::json!({ "kind": "manifest-read", "path": path.display().to_string() })
        }
        ResumeCompatibilityError::Decode { path, .. } => {
            serde_json::json!({ "kind": "manifest-decode", "path": path.display().to_string() })
        }
    }
}

fn p8_resume_compatibility_error<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> Option<&'a ResumeCompatibilityError> {
    error
        .downcast_ref::<OptimizeError>()
        .and_then(|error| match error {
            OptimizeError::ResumeCompatibility(source) => Some(source.as_ref()),
            _ => None,
        })
}

fn runtime_compatibility_json(
    kind: &'static str,
    stored: RuntimeFingerprint,
    live: RuntimeFingerprint,
) -> serde_json::Value {
    serde_json::json!({
        "kind": kind,
        "stored": report_runtime_fingerprint(stored),
        "live": report_runtime_fingerprint(live),
    })
}

fn report_runtime_fingerprint(fingerprint: RuntimeFingerprint) -> String {
    report_full_fingerprint(fingerprint.fingerprint())
}

fn p8_aime_failure_report_json(
    config: &AimeRunConfig,
    error: &(dyn std::error::Error + 'static),
    wall_time: Duration,
) -> serde_json::Value {
    let role_reports = p8_failure_role_reports(config);
    serde_json::json!({
        "schema": "leaven.p8_aime.failure_report.v1",
        "run_profile": config.profile.label(),
        "gepa_profile": config.gepa_profile.label(),
        "proof_classification": proof_classification_for_config(config),
        "comparison_target": config.profile.comparison_target(),
        "comparison_published_test_score": config.profile.published_test_score(),
        "comparison_published_validation_score": config.profile.published_validation_score(),
        "comparison_reflection_prompt": config.profile.reflection_prompt_claim(),
        "comparison": p8_comparison_json(config),
        "data_source": config.data_source.label(),
        "run_dir": config
            .run_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        "wall_time_ms": wall_time.as_millis(),
        "search_metric_call_cap": config.budget.metric_calls,
        "final_report_metric_call_cap": "unlimited",
        "cache": {
            "evaluation_policy": report_evaluation_cache_policy(&config.evaluation_cache_policy),
            "lm_backend": report_lm_cache_backend(config.solver.runtime.cache_backend),
            "lm_durable": config.solver.runtime.cache_backend.is_durable(),
            "lm_path": p8_failure_lm_cache_path(config),
            "lm_read_paths": p8_failure_lm_cache_read_paths(config),
            "lm_write_path": p8_failure_lm_cache_write_path(config),
        },
        "error": error.to_string(),
        "resume_compatibility": p8_failure_compatibility_json(error),
        "sources": std::iter::successors(error.source(), |source| source.source())
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "lm_roles": role_reports.iter().map(p8_lm_role_report_json).collect::<Vec<_>>(),
        "live_provider_proof": p8_live_provider_proof_json(&role_reports),
        "provider_failures": p8_provider_failures_json(&role_reports),
        "solver_runtime": p8_failure_runtime_json(
            config.solver.live,
            &config.solver.model,
            config.solver.cache_policy,
            config.solver.runtime
        ),
        "reflection_runtime": p8_failure_runtime_json(
            config.reflection.live,
            &config.reflection.model,
            config.reflection.cache_policy,
            config.reflection.runtime
        ),
    })
}

fn p8_failure_role_reports(config: &AimeRunConfig) -> AimeRoleReports {
    let reports = AimeRoleReports::from_config(
        config,
        AimeRoleRuntimeFingerprints::from_config(config),
        AimeLmRoleMetrics::default(),
        AimeLmRoleMetrics::default(),
    );
    match &config.run_dir {
        Some(run_dir) => reports.with_durable_failures(AimeDurableProviderFailures::read(
            &aime_provider_failures_path(run_dir),
        )),
        None => reports,
    }
}

fn p8_failure_lm_cache_path(config: &AimeRunConfig) -> String {
    match config.solver.runtime.cache_backend {
        AimeLmCacheBackend::InMemory => "none".to_owned(),
        AimeLmCacheBackend::Sqlite => config
            .run_dir
            .as_ref()
            .map(|run_dir| {
                SqliteLmCache::path_in_run_dir(run_dir)
                    .display()
                    .to_string()
            })
            .unwrap_or_else(|| "none".to_owned()),
        AimeLmCacheBackend::EagerSqlite => {
            SqliteLmCache::path_in_workspace(".").display().to_string()
        }
    }
}

fn p8_failure_lm_cache_read_paths(config: &AimeRunConfig) -> Vec<String> {
    match config.solver.runtime.cache_backend {
        AimeLmCacheBackend::InMemory => Vec::new(),
        AimeLmCacheBackend::Sqlite => config
            .run_dir
            .as_ref()
            .map(|run_dir| {
                vec![
                    SqliteLmCache::path_in_run_dir(run_dir)
                        .display()
                        .to_string(),
                ]
            })
            .unwrap_or_default(),
        AimeLmCacheBackend::EagerSqlite => {
            let mut paths = config
                .run_dir
                .as_ref()
                .map(|run_dir| {
                    vec![
                        SqliteLmCache::path_in_run_dir(run_dir)
                            .display()
                            .to_string(),
                    ]
                })
                .unwrap_or_default();
            paths.push(SqliteLmCache::path_in_workspace(".").display().to_string());
            paths
        }
    }
}

fn p8_failure_lm_cache_write_path(config: &AimeRunConfig) -> String {
    match config.solver.runtime.cache_backend {
        AimeLmCacheBackend::InMemory => "none".to_owned(),
        AimeLmCacheBackend::Sqlite => p8_failure_lm_cache_path(config),
        AimeLmCacheBackend::EagerSqlite => {
            SqliteLmCache::path_in_workspace(".").display().to_string()
        }
    }
}

fn p8_aime_start_report_json(config: &AimeRunConfig, started_at: SystemTime) -> serde_json::Value {
    serde_json::json!({
        "schema": "leaven.p8_aime.start_report.v1",
        "run_profile": config.profile.label(),
        "gepa_profile": config.gepa_profile.label(),
        "proof_classification": proof_classification_for_config(config),
        "comparison_target": config.profile.comparison_target(),
        "comparison_published_test_score": config.profile.published_test_score(),
        "comparison_published_validation_score": config.profile.published_validation_score(),
        "comparison_reflection_prompt": config.profile.reflection_prompt_claim(),
        "comparison": p8_comparison_json(config),
        "data_source": config.data_source.label(),
        "run_dir": config
            .run_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        "started_unix_ms": system_time_unix_millis(started_at),
        "search_metric_call_cap": config.budget.metric_calls,
        "solver_runtime": p8_failure_runtime_json(
            config.solver.live,
            &config.solver.model,
            config.solver.cache_policy,
            config.solver.runtime
        ),
        "reflection_runtime": p8_failure_runtime_json(
            config.reflection.live,
            &config.reflection.model,
            config.reflection.cache_policy,
            config.reflection.runtime
        ),
    })
}

fn system_time_unix_millis(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn p8_failure_runtime_json(
    live: bool,
    model: &str,
    cache_policy: LmCachePolicy,
    runtime: AimeOpenAiRuntimeConfig,
) -> serde_json::Value {
    serde_json::json!({
        "live": live,
        "model": model,
        "cache_policy": report_lm_cache_policy(cache_policy),
        "cache_backend": report_lm_cache_backend(runtime.cache_backend),
        "cache_durable": runtime.cache_backend.is_durable(),
        "max_concurrent_requests": runtime.max_concurrent_requests.get(),
        "request_timeout_seconds": runtime.request_timeout_seconds,
    })
}

fn p8_dataset_proof_json(proof: &AimeDatasetProof) -> serde_json::Value {
    serde_json::json!({
        "train_count": proof.train_count,
        "validation_count": proof.validation_count,
        "test_count": proof.test_count,
        "source_splits": proof.source_splits.iter().map(|source| serde_json::json!({
            "role": source.role,
            "dataset": source.dataset,
            "config": source.config,
            "split": source.split,
            "count": source.count,
        })).collect::<Vec<_>>(),
        "materialized_cache": proof.materialized_cache.as_ref().map(|cache| serde_json::json!({
            "path": cache.path,
            "sha256": cache.sha256,
            "bytes": cache.bytes,
        })),
        "split_seed": proof.split_seed,
        "test_repeated": proof.test_repeated,
    })
}

fn p8_gepa_report_json(
    report: &GepaReport,
    roles: &AimeRoleReports,
    seed_prompt: &str,
) -> serde_json::Value {
    let reflection_requests = roles.reflection.metrics.requests.as_slice();
    let candidate_prompts = p8_gepa_candidate_prompt_map(report, reflection_requests, seed_prompt);
    let candidate_admissions = p8_gepa_candidate_admission_map(report);
    let (accepted_count, accepted_unadmitted_count) = gepa_attempt_counts(report);
    serde_json::json!({
        "profile": &report.profile,
        "best_index": report.best_index.map(GepaCandidateIndex::get),
        "best_candidate": report.best_candidate.map(|candidate| candidate.to_string()),
        "validation_best_index": report.validation_best_index.map(GepaCandidateIndex::get),
        "validation_best_candidate": report.validation_best_candidate.map(|candidate| candidate.to_string()),
        "total_metric_calls": report.total_metric_calls,
        "full_validation_evals": report.full_validation_evals,
        "accepted_count": accepted_count,
        "accepted_unadmitted_count": accepted_unadmitted_count,
        "quality_summary": &report.quality_summary,
        "reflection_summary": p8_gepa_reflection_summary_json(report, reflection_requests),
        "skip_perfect_score": report.skip_perfect_score,
        "perfect_score": report.perfect_score,
        "candidates": report.candidates.iter().map(|candidate| serde_json::json!({
            "index": candidate.index.get(),
            "candidate": candidate.candidate.to_string(),
            "system_prompt": p8_gepa_candidate_prompt_text(&candidate_prompts, candidate.candidate),
            "system_prompt_source": p8_gepa_candidate_prompt_source(
                &candidate_prompts,
                candidate.candidate
            ),
            "parents": candidate.parents.iter().map(|index| index.get()).collect::<Vec<_>>(),
            "discovery_metric_calls": candidate.discovery_metric_calls,
            "validation_score": candidate.validation_score,
            "validation_rows": candidate.validation_rows.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "validation_subscores": candidate.validation_subscores.iter().map(|subscore| serde_json::json!({
                "case": subscore.case.to_string(),
                "score": subscore.score,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "validation_frontier": report.validation_frontier.iter().map(|frontier| serde_json::json!({
            "case": frontier.case.to_string(),
            "candidates": frontier.candidates.iter().map(|index| index.get()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "candidate_history": report.candidate_history.iter().map(|entry| serde_json::json!({
            "candidate": entry.candidate.to_string(),
            "candidate_index": entry.candidate_index.map(GepaCandidateIndex::get),
            "assessments": entry.assessments.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "score": entry.score,
        })).collect::<Vec<_>>(),
        "proposal_attempts": report.proposal_attempts.iter().scan(0usize, |reflection_request_index, attempt| {
            let request_index = if attempt.skip_reason.is_none() {
                let index = *reflection_request_index;
                *reflection_request_index += 1;
                Some(index)
            } else {
                None
            };
            let reflection = request_index
                .and_then(|index| p8_gepa_attempt_reflection_json(index, reflection_requests));
            Some(serde_json::json!({
                "attempt_index": attempt.attempt_index,
                "iteration": attempt.iteration,
                "reflection_request_index": request_index,
                "reflection": reflection,
                "parent_index": attempt.parent_index.get(),
                "parent": attempt.parent.to_string(),
                "parent_assessments": attempt.parent_assessments.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "parent_cases": attempt.parent_cases.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "parent_score": attempt.parent_score,
                "part_label": attempt.part_label.as_deref(),
                "reflective_example_count": attempt.reflective_example_count,
                "child": attempt.child.map(|candidate| candidate.to_string()),
                "child_index": attempt
                    .child
                    .and_then(|candidate| candidate_admissions.get(&candidate))
                    .map(|admission| admission.index),
                "child_validation_score": attempt
                    .child
                    .and_then(|candidate| candidate_admissions.get(&candidate))
                    .and_then(|admission| admission.validation_score),
                "child_assessments": attempt.child_assessments.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "child_cases": attempt.child_cases.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "child_score": attempt.child_score,
                "accepted": attempt.accepted,
                "admitted": attempt.admitted_index.is_some(),
                "admitted_index": attempt.admitted_index.map(GepaCandidateIndex::get),
                "skip_reason": attempt.skip_reason.map(p8_gepa_skip_reason),
            }))
        }).collect::<Vec<_>>(),
        "events": report.events.iter().map(p8_gepa_event_json).collect::<Vec<_>>(),
    })
}

fn p8_gepa_reflection_summary_json(
    report: &GepaReport,
    reflection_requests: &[AimeLmRequestRecord],
) -> serde_json::Value {
    let attempted_count = report
        .proposal_attempts
        .iter()
        .filter(|attempt| attempt.skip_reason.is_none())
        .count();
    let visible_prompts = reflection_requests
        .iter()
        .map(p8_lm_visible_prompt_text)
        .collect::<Vec<_>>();
    let request_chars = visible_prompts
        .iter()
        .map(|prompt| prompt.chars().count())
        .collect::<Vec<_>>();
    let assistant_chars = reflection_requests
        .iter()
        .filter_map(|request| request.response.as_ref())
        .map(|response| response.assistant.content.chars().count())
        .collect::<Vec<_>>();
    let proposed_text_chars = reflection_requests
        .iter()
        .filter_map(|request| request.response.as_ref())
        .map(|response| p8_extract_reflection_replacement(&response.assistant.content))
        .map(|text| text.chars().count())
        .collect::<Vec<_>>();
    let mut accepted_proposed_text_chars = Vec::new();
    let mut rejected_proposed_text_chars = Vec::new();
    for (attempt, request) in p8_gepa_attempt_reflection_pairs(report, reflection_requests) {
        let Some(response) = request.and_then(|request| request.response.as_ref()) else {
            continue;
        };
        let proposed_chars = p8_extract_reflection_replacement(&response.assistant.content)
            .chars()
            .count();
        match attempt.accepted {
            Some(true) => accepted_proposed_text_chars.push(proposed_chars),
            Some(false) => rejected_proposed_text_chars.push(proposed_chars),
            None => {}
        }
    }
    serde_json::json!({
        "attempted_count": attempted_count,
        "observed_request_count": reflection_requests.len(),
        "observed_response_count": assistant_chars.len(),
        "visible_prompt_unique_count": unique_string_count(&visible_prompts),
        "visible_prompt_duplicate_count": duplicate_string_count(&visible_prompts),
        "request_chars": p8_len_summary_json(&request_chars),
        "assistant_chars": p8_len_summary_json(&assistant_chars),
        "proposed_text_chars": p8_len_summary_json(&proposed_text_chars),
        "accepted_proposed_text_chars": p8_len_summary_json(&accepted_proposed_text_chars),
        "rejected_proposed_text_chars": p8_len_summary_json(&rejected_proposed_text_chars),
    })
}

fn p8_gepa_attempt_reflection_pairs<'a>(
    report: &'a GepaReport,
    reflection_requests: &'a [AimeLmRequestRecord],
) -> impl Iterator<Item = (&'a GepaProposalAttempt, Option<&'a AimeLmRequestRecord>)> {
    report
        .proposal_attempts
        .iter()
        .scan(0usize, |request_index, attempt| {
            let request = if attempt.skip_reason.is_none() {
                let request = reflection_requests.get(*request_index);
                *request_index += 1;
                request
            } else {
                None
            };
            Some((attempt, request))
        })
}

fn p8_lm_visible_prompt_text(request: &AimeLmRequestRecord) -> String {
    request
        .messages
        .iter()
        .map(|message| format!("{}:{}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn unique_string_count(values: &[String]) -> usize {
    values
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn duplicate_string_count(values: &[String]) -> usize {
    values.len().saturating_sub(unique_string_count(values))
}

fn p8_len_summary_json(lengths: &[usize]) -> serde_json::Value {
    let count = lengths.len();
    let sum = lengths.iter().sum::<usize>();
    let average = if count == 0 {
        serde_json::Value::Null
    } else {
        let sum = u32::try_from(sum).expect("AIME report length sum fits u32");
        let count = u32::try_from(count).expect("AIME report length count fits u32");
        serde_json::json!(f64::from(sum) / f64::from(count))
    };
    serde_json::json!({
        "count": count,
        "min": lengths.iter().min().copied(),
        "max": lengths.iter().max().copied(),
        "average": average,
    })
}

#[derive(Clone, Copy, Debug)]
struct P8GepaCandidateAdmission {
    index: u32,
    validation_score: Option<f64>,
}

#[derive(Clone, Debug)]
struct P8GepaCandidatePrompt {
    text: String,
    source: &'static str,
}

fn p8_gepa_candidate_admission_map(
    report: &GepaReport,
) -> BTreeMap<CandidateId, P8GepaCandidateAdmission> {
    report
        .candidates
        .iter()
        .map(|candidate| {
            (
                candidate.candidate,
                P8GepaCandidateAdmission {
                    index: candidate.index.get(),
                    validation_score: candidate.validation_score,
                },
            )
        })
        .collect()
}

fn p8_gepa_candidate_prompt_map(
    report: &GepaReport,
    reflection_requests: &[AimeLmRequestRecord],
    seed_prompt: &str,
) -> BTreeMap<CandidateId, P8GepaCandidatePrompt> {
    let mut prompts = BTreeMap::new();
    if let Some(seed) = report
        .candidates
        .iter()
        .find(|candidate| candidate.index.get() == 0)
    {
        prompts.insert(
            seed.candidate,
            P8GepaCandidatePrompt {
                text: seed_prompt.to_owned(),
                source: "seed_config",
            },
        );
    }

    let mut reflection_request_index = 0usize;
    for attempt in &report.proposal_attempts {
        let request_index = if attempt.skip_reason.is_none() {
            let index = reflection_request_index;
            reflection_request_index += 1;
            Some(index)
        } else {
            None
        };
        let Some(child) = attempt.child.filter(|_| attempt.admitted_index.is_some()) else {
            continue;
        };
        let Some(request) = request_index.and_then(|index| reflection_requests.get(index)) else {
            continue;
        };
        let Some(response) = &request.response else {
            continue;
        };
        prompts.insert(
            child,
            P8GepaCandidatePrompt {
                text: p8_extract_reflection_replacement(&response.assistant.content),
                source: "observed_reflection_response",
            },
        );
    }
    prompts
}

fn p8_gepa_candidate_prompt_text(
    prompts: &BTreeMap<CandidateId, P8GepaCandidatePrompt>,
    candidate: CandidateId,
) -> Option<&str> {
    prompts.get(&candidate).map(|prompt| prompt.text.as_str())
}

fn p8_gepa_candidate_prompt_source(
    prompts: &BTreeMap<CandidateId, P8GepaCandidatePrompt>,
    candidate: CandidateId,
) -> &'static str {
    prompts
        .get(&candidate)
        .map_or("unavailable_process_local_lm_telemetry", |prompt| {
            prompt.source
        })
}

fn p8_gepa_attempt_reflection_json(
    index: usize,
    requests: &[AimeLmRequestRecord],
) -> Option<serde_json::Value> {
    let request = requests.get(index)?;
    Some(serde_json::json!({
        "request_index": index,
        "model": request.model,
        "request": p8_lm_request_json(request),
        "assistant_text": request.response.as_ref().map(|response| response.assistant.content.clone()),
        "proposed_text": request
            .response
            .as_ref()
            .map(|response| p8_extract_reflection_replacement(&response.assistant.content)),
        "provider_response_id": request
            .response
            .as_ref()
            .and_then(|response| response.provider_response_id.clone()),
    }))
}

fn p8_extract_reflection_replacement(assistant_text: &str) -> String {
    let text = assistant_text.trim();
    let Some(start) = text.find("```") else {
        return text.to_owned();
    };
    let content_start = start + 3;
    let Some(end) = text.rfind("```").filter(|end| *end >= content_start) else {
        return p8_strip_opening_fence(text);
    };
    if end == start {
        return p8_strip_opening_fence(text);
    }

    p8_strip_optional_language(&text[content_start..end])
        .trim()
        .to_owned()
}

fn p8_strip_opening_fence(text: &str) -> String {
    text.strip_prefix("```")
        .map(p8_strip_optional_language)
        .unwrap_or(text)
        .trim()
        .trim_end_matches("```")
        .trim()
        .to_owned()
}

fn p8_strip_optional_language(text: &str) -> &str {
    let trimmed = text.trim_start();
    match trimmed.find('\n') {
        Some(newline) => {
            let first_line = &trimmed[..newline];
            if !first_line.is_empty() && !first_line.contains(char::is_whitespace) {
                &trimmed[newline + 1..]
            } else {
                trimmed
            }
        }
        None => trimmed,
    }
}

fn p8_gepa_events_for_report(run: &AimeRunResult) -> &[GepaEventSummary] {
    run.gepa_report
        .as_ref()
        .map_or(run.gepa_events.as_slice(), |report| {
            report.events.as_slice()
        })
}

fn p8_gepa_event_json(event: &GepaEventSummary) -> serde_json::Value {
    match event {
        GepaEventSummary::ProfileResolved { profile } => serde_json::json!({
            "phase": "profile_resolved",
            "profile": &profile.label,
            "train_minibatch_size": profile.train_minibatch_size,
            "proposal_count": profile.proposal_count,
            "proposal_mode": &profile.proposal_mode,
            "validation_policy": &profile.validation_policy,
            "certification_mode": &profile.certification_mode,
            "skip_perfect_score": profile.skip_perfect_score,
            "perfect_score": &profile.perfect_score,
        }),
        GepaEventSummary::SeedValidationStarted { candidate } => serde_json::json!({
            "phase": "seed_validation_started",
            "candidate": candidate.to_string(),
        }),
        GepaEventSummary::SeedValidationCompleted {
            candidate_index,
            metric_calls_delta,
            score,
        } => serde_json::json!({
            "phase": "seed_validation_completed",
            "candidate_index": candidate_index.get(),
            "metric_calls_delta": metric_calls_delta,
            "score": score,
        }),
        GepaEventSummary::IterationStarted { iteration } => serde_json::json!({
            "phase": "iteration_started",
            "iteration": iteration,
        }),
        GepaEventSummary::ParentSelected { candidate_index } => serde_json::json!({
            "phase": "parent_selected",
            "candidate_index": candidate_index.get(),
        }),
        GepaEventSummary::TrainMinibatchSampled { cases } => serde_json::json!({
            "phase": "train_minibatch_sampled",
            "cases": cases.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }),
        GepaEventSummary::ParentEvaluated {
            metric_calls_delta,
            score,
        } => serde_json::json!({
            "phase": "parent_evaluated",
            "metric_calls_delta": metric_calls_delta,
            "score": score,
        }),
        GepaEventSummary::ProposalSkipped { reason } => serde_json::json!({
            "phase": "proposal_skipped",
            "reason": p8_gepa_skip_reason(*reason),
        }),
        GepaEventSummary::ReflectiveDatasetBuilt {
            records,
            cases,
            source_ref_count,
        } => serde_json::json!({
            "phase": "reflective_dataset_built",
            "records": records,
            "cases": cases.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "source_ref_count": source_ref_count,
        }),
        GepaEventSummary::ReflectionStarted {
            parent,
            part_label,
            records,
            cases,
            source_ref_count,
        } => serde_json::json!({
            "phase": "reflection_started",
            "parent": parent.to_string(),
            "part_label": part_label,
            "records": records,
            "cases": cases.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "source_ref_count": source_ref_count,
        }),
        GepaEventSummary::ReflectionCompleted { parent, child } => serde_json::json!({
            "phase": "reflection_completed",
            "parent": parent.to_string(),
            "child": child.map(|candidate| candidate.to_string()),
        }),
        GepaEventSummary::ChildBuilt { candidate } => serde_json::json!({
            "phase": "child_built",
            "candidate": candidate.to_string(),
        }),
        GepaEventSummary::ChildEvaluated {
            metric_calls_delta,
            score,
        } => serde_json::json!({
            "phase": "child_evaluated",
            "metric_calls_delta": metric_calls_delta,
            "score": score,
        }),
        GepaEventSummary::ProposalAccepted { child } => serde_json::json!({
            "phase": "proposal_accepted",
            "child": child.to_string(),
        }),
        GepaEventSummary::ProposalRejected => serde_json::json!({
            "phase": "proposal_rejected",
        }),
        GepaEventSummary::AcceptedValidationCompleted {
            candidate_index,
            metric_calls_delta,
            score,
        } => serde_json::json!({
            "phase": "accepted_validation_completed",
            "candidate_index": candidate_index.get(),
            "metric_calls_delta": metric_calls_delta,
            "score": score,
        }),
        GepaEventSummary::CandidateAdmitted {
            candidate,
            candidate_index,
        } => serde_json::json!({
            "phase": "candidate_admitted",
            "candidate": candidate.to_string(),
            "candidate_index": candidate_index.get(),
        }),
        GepaEventSummary::FrontierUpdated => serde_json::json!({
            "phase": "frontier_updated",
        }),
        GepaEventSummary::OptimizationEnded { best } => serde_json::json!({
            "phase": "optimization_ended",
            "best": best.map(GepaCandidateIndex::get),
        }),
    }
}

fn p8_gepa_skip_reason(reason: GepaSkipReason) -> &'static str {
    match reason {
        GepaSkipReason::NoReflectiveExamples => "no_reflective_examples",
        GepaSkipReason::AllScoresPerfect => "all_scores_perfect",
    }
}

#[cfg(test)]
fn gepa_frontier_signature(report: &GepaReport) -> Vec<(String, Vec<u32>)> {
    report
        .validation_frontier
        .iter()
        .map(|case| {
            (
                case.case.to_string(),
                case.candidates
                    .iter()
                    .map(|candidate| candidate.get())
                    .collect(),
            )
        })
        .collect()
}

fn p8_live_provider_proof_json(roles: &AimeRoleReports) -> serde_json::Value {
    let live_roles = roles.iter().filter(|role| role.live).count();
    let role_count = roles.iter().count();
    serde_json::json!({
        "role_count": role_count,
        "live_roles": live_roles,
        "all_roles_live": role_count > 0 && live_roles == role_count,
        "roles": roles.iter().map(|role| serde_json::json!({
            "role": role.role.label(),
            "provider": role.provider.label(),
            "live": role.live,
            "model": role.model,
            "runtime_fingerprint": report_fingerprint(role.runtime_fingerprint),
            "request_shape_fingerprint": report_full_fingerprint(
                role.prompt_contract.request_shape_fingerprint
            ),
            "cache_policy": report_lm_cache_policy(role.cache_policy),
            "cache_backend": report_lm_cache_backend(role.cache_backend),
            "cache_durable": role.cache_durable,
            "max_concurrent_requests": role.max_concurrent_requests.get(),
            "request_timeout_seconds": role.request_timeout_seconds,
        })).collect::<Vec<_>>(),
    })
}

fn p8_provider_failures_json(roles: &AimeRoleReports) -> serde_json::Value {
    let failures = p8_provider_failure_totals(roles);
    let durable_failures = p8_durable_provider_failure_totals(roles);
    serde_json::json!({
        "count": failures.total(),
        "scope": "process_local",
        "totals": p8_provider_failure_counts_json(&failures),
        "durable": {
            "scope": "run_dir_jsonl",
            "count": durable_failures.total(),
            "totals": p8_provider_failure_counts_json(&durable_failures),
            "roles": roles.iter().map(|role| serde_json::json!({
                "role": role.role.label(),
                "provider": role.provider.label(),
                "live": role.live,
                "model": role.model,
                "failures": p8_provider_failure_counts_json(&role.durable_failures),
            })).collect::<Vec<_>>(),
        },
        "roles": roles.iter().map(|role| serde_json::json!({
            "role": role.role.label(),
            "provider": role.provider.label(),
            "live": role.live,
            "model": role.model,
            "failures": p8_provider_failure_counts_json(&role.metrics.failures),
        })).collect::<Vec<_>>(),
    })
}

fn p8_durable_provider_failure_totals(roles: &AimeRoleReports) -> AimeProviderFailureCounts {
    AimeProviderFailureCounts {
        missing_credentials: roles
            .iter()
            .map(|role| role.durable_failures.missing_credentials)
            .sum(),
        authentication: roles
            .iter()
            .map(|role| role.durable_failures.authentication)
            .sum(),
        rate_limit: roles
            .iter()
            .map(|role| role.durable_failures.rate_limit)
            .sum(),
        retry_exhausted: roles
            .iter()
            .map(|role| role.durable_failures.retry_exhausted)
            .sum(),
        malformed_provider_response: roles
            .iter()
            .map(|role| role.durable_failures.malformed_provider_response)
            .sum(),
        answer_parse: roles
            .iter()
            .map(|role| role.durable_failures.answer_parse)
            .sum(),
        scorer_parse: roles
            .iter()
            .map(|role| role.durable_failures.scorer_parse)
            .sum(),
        budget_refusal: roles
            .iter()
            .map(|role| role.durable_failures.budget_refusal)
            .sum(),
        cache: roles.iter().map(|role| role.durable_failures.cache).sum(),
        transport: roles
            .iter()
            .map(|role| role.durable_failures.transport)
            .sum(),
        provider: roles
            .iter()
            .map(|role| role.durable_failures.provider)
            .sum(),
        unknown: roles.iter().map(|role| role.durable_failures.unknown).sum(),
    }
}

fn p8_provider_failure_totals(roles: &AimeRoleReports) -> AimeProviderFailureCounts {
    AimeProviderFailureCounts {
        missing_credentials: roles
            .iter()
            .map(|role| role.metrics.failures.missing_credentials)
            .sum(),
        authentication: roles
            .iter()
            .map(|role| role.metrics.failures.authentication)
            .sum(),
        rate_limit: roles
            .iter()
            .map(|role| role.metrics.failures.rate_limit)
            .sum(),
        retry_exhausted: roles
            .iter()
            .map(|role| role.metrics.failures.retry_exhausted)
            .sum(),
        malformed_provider_response: roles
            .iter()
            .map(|role| role.metrics.failures.malformed_provider_response)
            .sum(),
        answer_parse: roles
            .iter()
            .map(|role| role.metrics.failures.answer_parse)
            .sum(),
        scorer_parse: roles
            .iter()
            .map(|role| role.metrics.failures.scorer_parse)
            .sum(),
        budget_refusal: roles
            .iter()
            .map(|role| role.metrics.failures.budget_refusal)
            .sum(),
        cache: roles.iter().map(|role| role.metrics.failures.cache).sum(),
        transport: roles
            .iter()
            .map(|role| role.metrics.failures.transport)
            .sum(),
        provider: roles
            .iter()
            .map(|role| role.metrics.failures.provider)
            .sum(),
        unknown: roles.iter().map(|role| role.metrics.failures.unknown).sum(),
    }
}

fn p8_provider_failure_counts_json(failures: &AimeProviderFailureCounts) -> serde_json::Value {
    serde_json::json!({
        "count": failures.total(),
        "missing_credentials": failures.missing_credentials,
        "authentication": failures.authentication,
        "rate_limit": failures.rate_limit,
        "retry_exhausted": failures.retry_exhausted,
        "malformed_provider_response": failures.malformed_provider_response,
        "answer_parse": failures.answer_parse,
        "scorer_parse": failures.scorer_parse,
        "budget_refusal": failures.budget_refusal,
        "cache": failures.cache,
        "transport": failures.transport,
        "provider": failures.provider,
        "unknown": failures.unknown,
    })
}

fn p8_lm_role_report_json(role: &AimeLmRoleReport) -> serde_json::Value {
    serde_json::json!({
        "role": role.role.label(),
        "provider": role.provider.label(),
        "live": role.live,
        "model": role.model,
        "runtime_fingerprint": report_fingerprint(role.runtime_fingerprint),
        "prompt_contract": {
            "renderer": role.prompt_contract.renderer,
            "upstream": role.prompt_contract.upstream,
            "request_shape_fingerprint": report_full_fingerprint(
                role.prompt_contract.request_shape_fingerprint
            ),
            "request_example": p8_lm_request_json(&role.prompt_contract.request_example),
        },
        "observed_requests_scope": "process_local",
        "observed_request_count": role.metrics.requests.len(),
        "observed_requests": role
            .metrics
            .requests
            .iter()
            .map(p8_lm_request_json)
            .collect::<Vec<_>>(),
        "runtime": {
            "cache_policy": report_lm_cache_policy(role.cache_policy),
            "cache_backend": report_lm_cache_backend(role.cache_backend),
            "cache_durable": role.cache_durable,
            "max_concurrent_requests": role.max_concurrent_requests.get(),
            "request_timeout_seconds": role.request_timeout_seconds,
            "output": role.output,
            "parser": role.parser,
        },
        "metrics": {
            "calls": role.metrics.calls,
            "tokens": {
                "prompt": role.metrics.usage.input_tokens,
                "cached_input": role.metrics.usage.cached_input_tokens,
                "completion": role.metrics.usage.output_tokens,
                "reasoning": role.metrics.usage.reasoning_tokens,
            },
            "cost": {
                "llm_calls": role.metrics.cost.llm_calls,
                "prompt_tokens": role.metrics.cost.prompt_tokens,
                "completion_tokens": role.metrics.cost.completion_tokens,
            },
            "cache": {
                "hits": role.metrics.cache.hits,
                "misses": role.metrics.cache.misses,
                "bypasses": role.metrics.cache.bypasses(),
                "bypass_policy_never": role.metrics.cache.bypass_policy_never,
                "bypass_refresh": role.metrics.cache.bypass_refresh,
                "required_misses": role.metrics.cache.required_misses,
                "read_errors": role.metrics.cache.read_errors,
                "write_errors": role.metrics.cache.write_errors,
                "other_errors": role.metrics.cache.other_errors,
                "hit_cost_zero": role.metrics.cache.hit_cost_zero,
            },
            "failures": p8_provider_failure_counts_json(&role.metrics.failures),
            "durable_failures": p8_provider_failure_counts_json(&role.durable_failures),
        },
    })
}

fn p8_lm_request_json(request: &AimeLmRequestRecord) -> serde_json::Value {
    serde_json::json!({
        "model": request.model,
        "messages": request.messages.iter().map(p8_lm_message_json).collect::<Vec<_>>(),
        "output": request.output,
        "sampling": {
            "temperature": request.sampling.temperature,
            "max_output_tokens": request.sampling.max_output_tokens,
            "reasoning_effort": request.sampling.reasoning_effort,
        },
        "response": request.response.as_ref().map(p8_lm_response_json),
    })
}

fn p8_lm_message_json(message: &AimeLmMessageRecord) -> serde_json::Value {
    serde_json::json!({
        "role": message.role,
        "content": message.content,
    })
}

fn p8_lm_response_json(response: &AimeLmResponseRecord) -> serde_json::Value {
    serde_json::json!({
        "assistant": p8_lm_message_json(&response.assistant),
        "provider_response_id": response.provider_response_id,
        "continuation": {
            "provider": response.continuation_provider,
            "response_id": response.continuation_response_id,
            "covered_messages": response.continuation_covered_messages,
        },
    })
}

fn p8_case_report_json(run: &AimeRunResult) -> Vec<serde_json::Value> {
    let mut cases = Vec::new();
    for split in &run.optimized.summary.evaluation.splits_reported {
        for (candidate_index, candidate) in split.candidates.iter().enumerate() {
            let candidate_role = p8_candidate_report_role(candidate_index);
            for case in &candidate.cases {
                cases.push(serde_json::json!({
                    "case_id": case.case_id.to_string(),
                    "source_id": run
                        .report_metadata
                        .get(&case.case_id)
                        .map(AimeReportMetadata::source_id)
                        .unwrap_or_else(|| "missing-source-id".to_owned()),
                    "split": split_role_label(&split.role),
                    "candidate_role": candidate_role,
                    "candidate": candidate.candidate.to_string(),
                    "score_state": "present",
                    "score": case.score,
                    "output_ref": case.output_ref.as_ref().map(p8_evidence_ref_json),
                    "feedback_ref": case.feedback_ref.as_ref().map(p8_evidence_ref_json),
                    "trace_refs": case
                        .trace_refs
                        .iter()
                        .map(p8_evidence_ref_json)
                        .collect::<Vec<_>>(),
                    "output_chars": case.output.len(),
                    "feedback_chars": case.feedback.len(),
                }));
            }
        }
    }
    cases
}

#[derive(Clone, Debug, Default)]
struct P8CaseDeltaRow {
    case_id: String,
    source_id: String,
    split: String,
    baseline_score: Option<f64>,
    optimized_score: Option<f64>,
}

#[derive(Clone, Debug, Default)]
struct P8CaseDeltaSummary {
    improved: u64,
    regressed: u64,
    unchanged_correct: u64,
    unchanged_wrong: u64,
    unchanged_other: u64,
    missing_baseline: u64,
    missing_optimized: u64,
}

impl P8CaseDeltaSummary {
    fn record(&mut self, outcome: &str) {
        match outcome {
            "improved" => self.improved += 1,
            "regressed" => self.regressed += 1,
            "unchanged_correct" => self.unchanged_correct += 1,
            "unchanged_wrong" => self.unchanged_wrong += 1,
            "missing_baseline" => self.missing_baseline += 1,
            "missing_optimized" => self.missing_optimized += 1,
            _ => self.unchanged_other += 1,
        }
    }

    fn total(&self) -> u64 {
        self.improved
            + self.regressed
            + self.unchanged_correct
            + self.unchanged_wrong
            + self.unchanged_other
            + self.missing_baseline
            + self.missing_optimized
    }
}

fn p8_case_delta_report_json(run: &AimeRunResult) -> serde_json::Value {
    let mut rows = BTreeMap::<(String, String), P8CaseDeltaRow>::new();
    for split in &run.optimized.summary.evaluation.splits_reported {
        let split_label = split_role_label(&split.role).to_owned();
        for (candidate_index, candidate) in split.candidates.iter().enumerate() {
            let is_baseline = candidate_index == 0;
            for case in &candidate.cases {
                let source_id = run
                    .report_metadata
                    .get(&case.case_id)
                    .map(AimeReportMetadata::source_id)
                    .unwrap_or_else(|| "missing-source-id".to_owned());
                let row = rows
                    .entry((split_label.clone(), source_id.clone()))
                    .or_insert_with(|| P8CaseDeltaRow {
                        case_id: case.case_id.to_string(),
                        source_id,
                        split: split_label.clone(),
                        baseline_score: None,
                        optimized_score: None,
                    });
                if is_baseline {
                    row.baseline_score = Some(case.score);
                } else {
                    row.optimized_score = Some(case.score);
                }
            }
        }
    }

    let mut summary = BTreeMap::<String, P8CaseDeltaSummary>::new();
    let mut cases = Vec::new();
    for row in rows.into_values() {
        let (score_delta, outcome) = p8_case_delta_outcome(row.baseline_score, row.optimized_score);
        summary
            .entry(row.split.clone())
            .or_default()
            .record(outcome);
        cases.push(serde_json::json!({
            "case_id": row.case_id,
            "source_id": row.source_id,
            "split": row.split,
            "baseline_score": row.baseline_score,
            "optimized_score": row.optimized_score,
            "score_delta": score_delta,
            "outcome": outcome,
        }));
    }

    serde_json::json!({
        "summary": summary
            .into_iter()
            .map(|(split, counts)| {
                (split, serde_json::json!({
                    "total": counts.total(),
                    "improved": counts.improved,
                    "regressed": counts.regressed,
                    "unchanged_correct": counts.unchanged_correct,
                    "unchanged_wrong": counts.unchanged_wrong,
                    "unchanged_other": counts.unchanged_other,
                    "missing_baseline": counts.missing_baseline,
                    "missing_optimized": counts.missing_optimized,
                }))
            })
            .collect::<serde_json::Map<_, _>>(),
        "cases": cases,
    })
}

fn p8_case_delta_outcome(
    baseline_score: Option<f64>,
    optimized_score: Option<f64>,
) -> (Option<f64>, &'static str) {
    const EPSILON: f64 = 1e-12;
    match (baseline_score, optimized_score) {
        (Some(baseline), Some(optimized)) => {
            let delta = optimized - baseline;
            let outcome = if delta > EPSILON {
                "improved"
            } else if delta < -EPSILON {
                "regressed"
            } else if optimized >= 1.0 - EPSILON {
                "unchanged_correct"
            } else if optimized <= EPSILON {
                "unchanged_wrong"
            } else {
                "unchanged_other"
            };
            (Some(delta), outcome)
        }
        (None, Some(_)) => (None, "missing_baseline"),
        (Some(_), None) => (None, "missing_optimized"),
        (None, None) => (None, "unchanged_other"),
    }
}

fn p8_candidate_report_role(candidate_index: usize) -> &'static str {
    match candidate_index {
        0 => "baseline",
        _ => "optimized",
    }
}

fn evidence_ref_text(reference: &leaven::kernel::EvidenceRef) -> String {
    format!("{}:{}", reference.store.as_str(), reference.key.as_str())
}

fn p8_evidence_ref_json(reference: &leaven::kernel::EvidenceRef) -> serde_json::Value {
    serde_json::json!({
        "store": reference.store.as_str(),
        "key": reference.key.as_str(),
    })
}

#[derive(Clone, Debug)]
struct AimeRunConfig {
    profile: AimeRunProfile,
    data_source: AimeDataSource,
    gepa_profile: GepaProfile,
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
        Self::live_openai_with_controls(profile, data_source, metric_calls, cache_policies, runtime)
    }

    fn live_openai_with_controls(
        profile: AimeRunProfile,
        data_source: AimeDataSource,
        metric_calls: u64,
        cache_policies: AimeLmCachePolicies,
        runtime: AimeOpenAiRuntimeConfig,
    ) -> Self {
        Self {
            profile,
            data_source,
            gepa_profile: aime_gepa_profile_from_env(),
            seed_prompt: BASELINE,
            budget: Budget::metric_calls(metric_calls),
            evaluation_parallelism: runtime.max_concurrent_requests,
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
            gepa_profile: GepaProfile::Reference,
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

fn proof_classification_for_report(
    config: &AimeRunConfig,
    role_reports: &AimeRoleReports,
) -> &'static str {
    if role_reports
        .iter()
        .any(|role| role.live && role.cache_policy == LmCachePolicy::CacheOnly)
    {
        return "cache_only_aime_replay_not_live_proof";
    }
    proof_classification_for_config(config)
}

fn proof_classification_for_config(config: &AimeRunConfig) -> &'static str {
    if (config.solver.live && config.solver.cache_policy == LmCachePolicy::CacheOnly)
        || (config.reflection.live && config.reflection.cache_policy == LmCachePolicy::CacheOnly)
    {
        return "cache_only_aime_replay_not_live_proof";
    }
    config.proof_classification()
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
            Self::GepaAime => Some(GEPA_CAIS_AIME_PUBLISHED_TEST_SCORE),
        }
    }

    const fn published_validation_score(self) -> Option<f64> {
        match self {
            Self::DeterministicSmoke | Self::DspyQuickstart => None,
            Self::GepaAime => Some(GEPA_CAIS_AIME_PUBLISHED_VALIDATION_SCORE),
        }
    }

    const fn upstream_configured_search_metric_call_cap(self) -> Option<u64> {
        match self {
            Self::DeterministicSmoke | Self::DspyQuickstart => None,
            Self::GepaAime => Some(GEPA_CAIS_AIME_CONFIGURED_SEARCH_CAP),
        }
    }

    const fn upstream_checkpoint_metric_calls(self) -> Option<u64> {
        match self {
            Self::DeterministicSmoke | Self::DspyQuickstart => None,
            Self::GepaAime => Some(GEPA_CAIS_AIME_CHECKPOINT_METRIC_CALLS),
        }
    }

    const fn upstream_checkpoint_candidate_count(self) -> Option<u64> {
        match self {
            Self::DeterministicSmoke | Self::DspyQuickstart => None,
            Self::GepaAime => Some(GEPA_CAIS_AIME_CHECKPOINT_CANDIDATES),
        }
    }

    const fn upstream_run_log_available(self) -> Option<bool> {
        match self {
            Self::DeterministicSmoke | Self::DspyQuickstart => None,
            Self::GepaAime => Some(false),
        }
    }

    const fn reflection_prompt_claim(self) -> &'static str {
        match self {
            Self::DeterministicSmoke | Self::DspyQuickstart | Self::GepaAime => {
                "upstream_optimize_anything_reflection_template"
            }
        }
    }

    const fn upstream_reflection_model(self) -> &'static str {
        match self {
            Self::DeterministicSmoke => "none",
            Self::DspyQuickstart | Self::GepaAime => UPSTREAM_GEPA_AIME_REFLECTION_MODEL,
        }
    }

    fn reflection_model_alignment(self, reflection_model: &str) -> &'static str {
        match self {
            Self::DeterministicSmoke => "not-applicable",
            Self::DspyQuickstart | Self::GepaAime
                if normalized_openai_model_name(reflection_model)
                    == normalized_openai_model_name(self.upstream_reflection_model()) =>
            {
                "upstream-matched"
            }
            Self::DspyQuickstart | Self::GepaAime => "model-delta",
        }
    }

    fn comparison_notes(self, reflection_model: &str) -> Vec<&'static str> {
        match self {
            Self::DeterministicSmoke => Vec::new(),
            Self::DspyQuickstart => {
                let mut notes = vec![
                    "published_dspy_quickstart_reports_46.6_to_56.6_percent_on_aime_2025",
                    "leaven_uses_rust_local_dspy_chainofthought_prompt_rendering_without_dspy_runtime",
                ];
                notes.push(self.reflection_model_note(reflection_model));
                notes
            }
            Self::GepaAime => {
                // This is intentionally a report-visible provenance caveat:
                // the local CAIS checkpoint is inspectable, but its README's
                // cited `logs/run.log` is absent in this checkout.
                let mut notes = vec![
                    "published_gepa_cais_artifact_reports_46.67_to_60.00_percent_on_aime_2025",
                    "upstream_source_uses_serial_proposals_but_local_checkpoint_has_10_candidates_621_metric_calls_and_missing_run_log",
                    "leaven_uses_rust_local_dspy_chainofthought_prompt_rendering_without_dspy_runtime",
                ];
                notes.push(self.reflection_model_note(reflection_model));
                notes
            }
        }
    }

    fn reflection_model_note(self, reflection_model: &str) -> &'static str {
        match self.reflection_model_alignment(reflection_model) {
            "upstream-matched" => "leaven_reflection_model_matches_upstream_aime_profile",
            _ => "leaven_reflection_model_differs_from_upstream_aime_profile",
        }
    }
}

fn normalized_openai_model_name(model: &str) -> &str {
    model.strip_prefix("openai/").unwrap_or(model)
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

const fn lm_role_label(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

const fn reasoning_effort_label(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::None => "none",
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
    }
}

#[derive(Clone, Debug)]
struct AimeRoleReports {
    solver: AimeLmRoleReport,
    reflection: AimeLmRoleReport,
}

#[derive(Clone, Copy, Debug)]
struct AimeRoleRuntimeFingerprints {
    solver: Fingerprint,
    reflection: Fingerprint,
}

impl AimeRoleRuntimeFingerprints {
    fn from_config(config: &AimeRunConfig) -> Self {
        let solver_lm = if config.solver.live {
            Some(openai_provider_fingerprint_for_runtime(
                config.solver.runtime,
            ))
        } else {
            None
        };
        let reflection_lm = if config.reflection.live {
            openai_provider_fingerprint_for_runtime(config.reflection.runtime)
        } else {
            DeterministicReflectionLm.fingerprint()
        };
        Self {
            solver: aime_runner_fingerprint(&config.solver, solver_lm),
            reflection: aime_reflection_role_fingerprint(&config.reflection, reflection_lm),
        }
    }
}

impl AimeRoleReports {
    fn from_config(
        config: &AimeRunConfig,
        runtime_fingerprints: AimeRoleRuntimeFingerprints,
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
                runtime_fingerprint: runtime_fingerprints.solver,
                cache_policy: config.solver.cache_policy,
                cache_backend: config.solver.runtime.cache_backend,
                cache_durable: config.solver.runtime.cache_backend.is_durable(),
                max_concurrent_requests: config.solver.runtime.max_concurrent_requests,
                request_timeout_seconds: config.solver.runtime.request_timeout_seconds,
                output: "dspy-chain-of-thought-with-json-fallback",
                parser: "dspy-chat-adapter-fields-or-json-adapter",
                prompt_contract: AimePromptContractReport {
                    renderer: "dspy-chat-adapter-chain-of-thought-with-json-fallback",
                    upstream: "dspy.ChatAdapter->JSONAdapter",
                    request_shape_fingerprint: aime_solver_request_shape_fingerprint(),
                    request_example: aime_solver_request_example(&config.solver),
                },
                metrics: solver_metrics,
                durable_failures: AimeProviderFailureCounts::default(),
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
                runtime_fingerprint: runtime_fingerprints.reflection,
                cache_policy: config.reflection.cache_policy,
                cache_backend: config.reflection.runtime.cache_backend,
                cache_durable: config.reflection.runtime.cache_backend.is_durable(),
                max_concurrent_requests: config.reflection.runtime.max_concurrent_requests,
                request_timeout_seconds: config.reflection.runtime.request_timeout_seconds,
                output: "text",
                parser: "plain-text-fenced",
                prompt_contract: AimePromptContractReport {
                    renderer: "gepa-default-markdown-side-info",
                    upstream: "gepa.optimize_anything",
                    request_shape_fingerprint: aime_reflection_request_shape_fingerprint(),
                    request_example: aime_reflection_request_example(&config.reflection),
                },
                metrics: reflection_metrics,
                durable_failures: AimeProviderFailureCounts::default(),
            },
        }
    }

    fn iter(&self) -> impl Iterator<Item = &AimeLmRoleReport> {
        [&self.solver, &self.reflection].into_iter()
    }

    fn with_durable_failures(mut self, failures: AimeDurableProviderFailures) -> Self {
        self.solver.durable_failures = failures.solver;
        self.reflection.durable_failures = failures.reflection;
        self
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
    request_timeout_seconds: u64,
    output: &'static str,
    parser: &'static str,
    prompt_contract: AimePromptContractReport,
    metrics: AimeLmRoleMetrics,
    durable_failures: AimeProviderFailureCounts,
}

#[derive(Clone, Debug)]
struct AimePromptContractReport {
    renderer: &'static str,
    upstream: &'static str,
    request_shape_fingerprint: Fingerprint,
    request_example: AimeLmRequestRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeLmRequestRecord {
    model: String,
    messages: Vec<AimeLmMessageRecord>,
    output: String,
    sampling: AimeLmSamplingRecord,
    response: Option<AimeLmResponseRecord>,
}

impl AimeLmRequestRecord {
    fn from_request(request: &LmRequest) -> Self {
        Self {
            model: request.model.to_string(),
            messages: request
                .messages
                .iter()
                .map(AimeLmMessageRecord::from_message)
                .collect(),
            output: format!("{:?}", request.output),
            sampling: AimeLmSamplingRecord::from_sampling(&request.sampling),
            response: None,
        }
    }

    fn from_exchange(request: &LmRequest, response: &LmResponse) -> Self {
        let mut record = Self::from_request(request);
        record.response = Some(AimeLmResponseRecord::from_response(response));
        record
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeLmResponseRecord {
    assistant: AimeLmMessageRecord,
    provider_response_id: Option<String>,
    continuation_provider: Option<String>,
    continuation_response_id: Option<String>,
    continuation_covered_messages: Option<usize>,
}

impl AimeLmResponseRecord {
    fn from_response(response: &LmResponse) -> Self {
        Self {
            assistant: AimeLmMessageRecord::from_message(&response.assistant),
            provider_response_id: response.provider_response_id.clone(),
            continuation_provider: response
                .continuation
                .as_ref()
                .map(|continuation| continuation.provider.to_string()),
            continuation_response_id: response
                .continuation
                .as_ref()
                .map(|continuation| continuation.response_id.clone()),
            continuation_covered_messages: response
                .continuation
                .as_ref()
                .map(|continuation| continuation.covered_messages),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeLmMessageRecord {
    role: &'static str,
    content: String,
}

impl AimeLmMessageRecord {
    fn from_message(message: &Message) -> Self {
        Self {
            role: lm_role_label(message.role()),
            content: message.content().to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeLmSamplingRecord {
    temperature: Option<String>,
    max_output_tokens: Option<u32>,
    reasoning_effort: Option<String>,
}

impl AimeLmSamplingRecord {
    fn from_sampling(sampling: &SamplingOptions) -> Self {
        Self {
            temperature: sampling.temperature.map(|value| value.as_f64().to_string()),
            max_output_tokens: sampling.max_output_tokens,
            reasoning_effort: sampling
                .reasoning_effort
                .as_ref()
                .map(|effort| reasoning_effort_label(*effort).to_owned()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct AimeLmRoleMetrics {
    calls: u64,
    usage: TokenUsage,
    cost: Cost,
    cache: AimeLmCacheMetrics,
    failures: AimeProviderFailureCounts,
    requests: Vec<AimeLmRequestRecord>,
}

impl AimeLmRoleMetrics {
    fn record_request(&mut self, request: &LmRequest) {
        self.requests
            .push(AimeLmRequestRecord::from_request(request));
    }

    fn record_exchange(&mut self, request: &LmRequest, response: &LmResponse) {
        self.requests
            .push(AimeLmRequestRecord::from_exchange(request, response));
    }

    fn record_success(&mut self, policy: LmCachePolicy, response: &LmResponse, cost: &Cost) {
        self.calls += 1;
        self.usage.input_tokens += response.usage.input_tokens;
        self.usage.cached_input_tokens += response.usage.cached_input_tokens;
        self.usage.output_tokens += response.usage.output_tokens;
        self.usage.reasoning_tokens += response.usage.reasoning_tokens;
        self.cost = self.cost.clone().combine(cost);
        self.cache.record_success(policy, &response.usage, cost);
    }

    fn record_failure_kind(&mut self, kind: AimeProviderFailureKind) {
        self.failures.increment(kind);
    }

    fn record_failure(&mut self, policy: LmCachePolicy, error: &LmError) {
        let kind = AimeProviderFailureKind::from_lm_error(error);
        self.record_failure_kind(kind);
        if kind == AimeProviderFailureKind::Cache {
            self.cache.record_failure(policy, error);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeLmCacheMetrics {
    hits: u64,
    misses: u64,
    bypass_policy_never: u64,
    bypass_refresh: u64,
    required_misses: u64,
    read_errors: u64,
    write_errors: u64,
    other_errors: u64,
    hit_cost_zero: bool,
}

impl Default for AimeLmCacheMetrics {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            bypass_policy_never: 0,
            bypass_refresh: 0,
            required_misses: 0,
            read_errors: 0,
            write_errors: 0,
            other_errors: 0,
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
            LmCachePolicy::ReadWrite | LmCachePolicy::ReadOnly | LmCachePolicy::CacheOnly => {
                if cost.is_zero() && !usage.to_cost().is_zero() {
                    self.hits += 1;
                    self.hit_cost_zero &= cost.is_zero();
                } else {
                    self.misses += 1;
                }
            }
        }
    }

    fn record_failure(&mut self, policy: LmCachePolicy, error: &LmError) {
        let LmError::Cache { message } = error else {
            return;
        };
        if message.contains("required lm cache entry was missing") {
            self.required_misses += 1;
        } else if message.contains("during put") {
            self.write_errors += 1;
        } else if message.contains("during get") || message.contains("codec") {
            self.read_errors += 1;
        } else if policy == LmCachePolicy::CacheOnly {
            self.required_misses += 1;
        } else {
            self.other_errors += 1;
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
    durable_failures: Option<AimeDurableProviderFailureLog>,
}

impl AimeLmTelemetry {
    fn new(policy: LmCachePolicy) -> Self {
        Self {
            policy,
            metrics: Arc::new(Mutex::new(AimeLmRoleMetrics::default())),
            durable_failures: None,
        }
    }

    fn with_durable_provider_failures(mut self, role: AimeLmRole, path: PathBuf) -> Self {
        self.durable_failures = Some(AimeDurableProviderFailureLog {
            role,
            path: Arc::new(path),
            lock: Arc::new(Mutex::new(())),
        });
        self
    }

    fn record(&self, result: &Result<Metered<LmResponse>, LmError>) {
        let mut metrics = self.metrics.lock().expect("AIME telemetry lock is valid");
        match result {
            Ok(metered) => {
                metrics.record_success(self.policy, &metered.value, &metered.cost);
            }
            Err(error) => {
                metrics.record_failure(self.policy, error);
                if let Some(durable) = &self.durable_failures {
                    durable.record_failure(AimeProviderFailureKind::from_lm_error(error));
                }
            }
        }
    }

    fn record_failure_kind(&self, kind: AimeProviderFailureKind) {
        self.metrics
            .lock()
            .expect("AIME telemetry lock is valid")
            .record_failure_kind(kind);
        if let Some(durable) = &self.durable_failures {
            durable.record_failure(kind);
        }
    }

    fn record_request(&self, request: &LmRequest) {
        self.metrics
            .lock()
            .expect("AIME telemetry lock is valid")
            .record_request(request);
    }

    fn record_exchange(&self, request: &LmRequest, response: &LmResponse) {
        self.metrics
            .lock()
            .expect("AIME telemetry lock is valid")
            .record_exchange(request, response);
    }

    fn snapshot(&self) -> AimeLmRoleMetrics {
        self.metrics
            .lock()
            .expect("AIME telemetry lock is valid")
            .clone()
    }
}

#[derive(Clone, Debug)]
struct AimeDurableProviderFailureLog {
    role: AimeLmRole,
    path: Arc<PathBuf>,
    lock: Arc<Mutex<()>>,
}

impl AimeDurableProviderFailureLog {
    fn record_failure(&self, kind: AimeProviderFailureKind) {
        let _guard = self.lock.lock().expect("AIME durable failure log lock");
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let line = serde_json::json!({
            "schema": "leaven.p8_aime.provider_failure.v1",
            "role": self.role.label(),
            "kind": kind.label(),
        })
        .to_string();
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&*self.path)
        {
            use std::io::Write as _;
            let _ = writeln!(file, "{line}");
            let _ = file.sync_all();
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AimeDurableProviderFailures {
    solver: AimeProviderFailureCounts,
    reflection: AimeProviderFailureCounts,
}

impl AimeDurableProviderFailures {
    fn read(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let mut failures = Self::default();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(role) = record.get("role").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let Some(kind) = record
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .and_then(aime_provider_failure_kind_from_label)
            else {
                continue;
            };
            match role {
                "solver" => failures.solver.increment(kind),
                "reflection" => failures.reflection.increment(kind),
                _ => {}
            }
        }
        failures
    }
}

fn aime_provider_failures_path(run_dir: &Path) -> PathBuf {
    run_dir.join("lm-provider-failures.jsonl")
}

fn aime_provider_failure_kind_from_label(label: &str) -> Option<AimeProviderFailureKind> {
    Some(match label {
        "missing_credentials" => AimeProviderFailureKind::MissingCredentials,
        "authentication" => AimeProviderFailureKind::Authentication,
        "rate_limit" => AimeProviderFailureKind::RateLimit,
        "retry_exhausted" => AimeProviderFailureKind::RetryExhausted,
        "malformed_provider_response" => AimeProviderFailureKind::MalformedProviderResponse,
        "answer_parse" => AimeProviderFailureKind::AnswerParse,
        "scorer_parse" => AimeProviderFailureKind::ScorerParse,
        "budget_refusal" => AimeProviderFailureKind::BudgetRefusal,
        "cache" => AimeProviderFailureKind::Cache,
        "transport" => AimeProviderFailureKind::Transport,
        "provider" => AimeProviderFailureKind::Provider,
        "unknown" => AimeProviderFailureKind::Unknown,
        _ => return None,
    })
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

    fn record_failure_kind(&self, kind: AimeProviderFailureKind) {
        self.telemetry.record_failure_kind(kind);
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
        let result = self.inner.complete(request.clone()).await;
        match &result {
            Ok(metered) => self.telemetry.record_exchange(&request, &metered.value),
            Err(_) => self.telemetry.record_request(&request),
        }
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
        "cache-only" | "cache_only" | "cacheonly" | "require-hit" | "require_hit" | "required" => {
            LmCachePolicy::CacheOnly
        }
        "refresh" => LmCachePolicy::Refresh,
        _ => panic!(
            "unsupported {env_name}={raw:?}; expected never, read-write, read-only, cache-only, or refresh"
        ),
    }
}

fn aime_gepa_profile_from_env() -> GepaProfile {
    let value = std::env::var(LEAVEN_AIME_GEPA_PROFILE).ok();
    parse_gepa_profile(LEAVEN_AIME_GEPA_PROFILE, value.as_deref())
}

fn parse_gepa_profile(env_name: &str, value: Option<&str>) -> GepaProfile {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return GepaProfile::OptimizeAnything;
    };
    match raw.to_ascii_lowercase().as_str() {
        "reference" | "ref" => GepaProfile::Reference,
        "optimize-anything" | "optimize_anything" | "optimizeanything" => {
            GepaProfile::OptimizeAnything
        }
        "fast-certified" | "fast_certified" | "fastcertified" => GepaProfile::FastCertified,
        _ => panic!(
            "unsupported {env_name}={raw:?}; expected optimize-anything, reference, or fast-certified"
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AimeOpenAiRuntimeConfig {
    max_concurrent_requests: NonZeroUsize,
    cache_backend: AimeLmCacheBackend,
    request_timeout_seconds: u64,
}

impl AimeOpenAiRuntimeConfig {
    fn from_env() -> Self {
        let max_concurrent = std::env::var(LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS).ok();
        let cache_backend = std::env::var(LEAVEN_AIME_LM_CACHE_BACKEND).ok();
        let request_timeout = std::env::var(LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS).ok();
        Self::from_values(
            max_concurrent.as_deref(),
            cache_backend.as_deref(),
            request_timeout.as_deref(),
        )
    }

    fn from_values(
        max_concurrent: Option<&str>,
        cache_backend: Option<&str>,
        request_timeout: Option<&str>,
    ) -> Self {
        Self {
            max_concurrent_requests: parse_max_concurrent_requests(max_concurrent),
            cache_backend: parse_lm_cache_backend(cache_backend),
            request_timeout_seconds: parse_request_timeout_seconds(request_timeout),
        }
    }

    fn default_for_p8() -> Self {
        Self {
            max_concurrent_requests: NonZeroUsize::new(GEPA_AIME_MAX_WORKERS)
                .expect("GEPA AIME worker count is non-zero"),
            cache_backend: AimeLmCacheBackend::InMemory,
            request_timeout_seconds: GEPA_AIME_OPENAI_REQUEST_TIMEOUT_SECONDS,
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

fn parse_request_timeout_seconds(value: Option<&str>) -> u64 {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return GEPA_AIME_OPENAI_REQUEST_TIMEOUT_SECONDS;
    };
    let parsed = raw.parse::<u64>().unwrap_or_else(|source| {
        panic!("unsupported {LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS}={raw:?}: {source}")
    });
    assert!(
        parsed != 0,
        "unsupported {LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS}=0; expected a positive integer"
    );
    parsed
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
        prompt_template: Some(OPTIMIZE_ANYTHING_REFLECTION_PROMPT_TEMPLATE.to_owned()),
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

#[derive(Default)]
struct AimeGepaProgress {
    seed_validation_score: Option<f64>,
    best_validation_score: Option<f64>,
    last_parent_train_score: Option<f64>,
    accepted_count: u64,
    admitted_count: u64,
    rejected_count: u64,
    skipped_count: u64,
    full_validation_evals: u64,
}

impl AimeGepaProgress {
    fn progress_line(&mut self, event: &GepaEventSummary) -> Option<String> {
        match event {
            GepaEventSummary::ProfileResolved { profile } => Some(format!(
                "progress_event=gepa_profile_resolved profile={} train_minibatch_size={} proposal_count={} validation_policy={} certification_mode={} skip_perfect_score={} perfect_score={}",
                profile.label,
                progress_optional_usize(profile.train_minibatch_size),
                profile.proposal_count,
                profile.validation_policy,
                profile.certification_mode,
                profile.skip_perfect_score,
                profile.perfect_score
            )),
            GepaEventSummary::IterationStarted { iteration } => Some(format!(
                "progress_event=gepa_iteration_started iteration={} current_best_validation_score={} baseline_validation_score={} accepted_count={} admitted_count={} rejected_count={} skipped_count={} full_validation_evals={}",
                iteration,
                progress_score(self.best_validation_score),
                progress_score(self.seed_validation_score),
                self.accepted_count,
                self.admitted_count,
                self.rejected_count,
                self.skipped_count,
                self.full_validation_evals
            )),
            GepaEventSummary::ParentSelected { candidate_index } => Some(format!(
                "progress_event=gepa_parent_selected candidate_index={}",
                candidate_index.get()
            )),
            GepaEventSummary::ParentEvaluated {
                metric_calls_delta,
                score,
            } => {
                self.last_parent_train_score = parse_gepa_score(score);
                Some(format!(
                    "progress_event=gepa_parent_evaluated train_screen_score={} metric_calls_delta={metric_calls_delta}",
                    progress_score(self.last_parent_train_score)
                ))
            }
            GepaEventSummary::ProposalSkipped { reason } => {
                self.skipped_count += 1;
                Some(format!(
                    "progress_event=gepa_proposal_skipped reason={} skipped_count={}",
                    p8_gepa_skip_reason(*reason),
                    self.skipped_count
                ))
            }
            GepaEventSummary::ReflectionStarted {
                parent,
                part_label,
                records,
                cases,
                source_ref_count,
            } => Some(format!(
                "progress_event=gepa_reflection_started parent={parent} part_label={part_label} records={records} cases={} source_ref_count={source_ref_count}",
                cases.len()
            )),
            GepaEventSummary::ReflectionCompleted { parent, child } => Some(format!(
                "progress_event=gepa_reflection_completed parent={parent} child={}",
                child.map_or_else(|| "none".to_owned(), |candidate| candidate.to_string())
            )),
            GepaEventSummary::ChildEvaluated {
                metric_calls_delta,
                score,
            } => {
                let child_score = parse_gepa_score(score);
                Some(format!(
                    "progress_event=gepa_child_evaluated train_screen_score={} parent_train_screen_score={} delta_vs_parent={} signal={} metric_calls_delta={metric_calls_delta}",
                    progress_score(child_score),
                    progress_score(self.last_parent_train_score),
                    progress_delta(child_score, self.last_parent_train_score),
                    progress_signal(child_score, self.last_parent_train_score)
                ))
            }
            GepaEventSummary::ProposalAccepted { child } => {
                self.accepted_count += 1;
                Some(format!(
                    "progress_event=gepa_proposal_accepted child={child} accepted_count={}",
                    self.accepted_count
                ))
            }
            GepaEventSummary::ProposalRejected => {
                self.rejected_count += 1;
                Some(format!(
                    "progress_event=gepa_proposal_rejected rejected_count={}",
                    self.rejected_count
                ))
            }
            GepaEventSummary::SeedValidationCompleted {
                candidate_index,
                metric_calls_delta,
                score,
            } => {
                let validation_score = parse_gepa_score(score);
                self.seed_validation_score = validation_score;
                self.best_validation_score = validation_score;
                self.full_validation_evals += 1;
                Some(format!(
                    "progress_event=gepa_seed_validation_completed candidate_index={} validation_score={} current_best_validation_score={} metric_calls_delta={metric_calls_delta} full_validation_evals={}",
                    candidate_index.get(),
                    progress_score(validation_score),
                    progress_score(self.best_validation_score),
                    self.full_validation_evals
                ))
            }
            GepaEventSummary::AcceptedValidationCompleted {
                candidate_index,
                metric_calls_delta,
                score,
            } => {
                let validation_score = parse_gepa_score(score);
                let previous_best = self.best_validation_score;
                if let Some(score) = validation_score {
                    if self.best_validation_score.is_none_or(|best| score > best) {
                        self.best_validation_score = Some(score);
                    }
                }
                self.full_validation_evals += 1;
                Some(format!(
                    "progress_event=gepa_accepted_validation_completed candidate_index={} validation_score={} baseline_validation_score={} delta_vs_baseline={} previous_best_validation_score={} delta_vs_previous_best={} signal={} current_best_validation_score={} metric_calls_delta={metric_calls_delta} full_validation_evals={}",
                    candidate_index.get(),
                    progress_score(validation_score),
                    progress_score(self.seed_validation_score),
                    progress_delta(validation_score, self.seed_validation_score),
                    progress_score(previous_best),
                    progress_delta(validation_score, previous_best),
                    progress_signal(validation_score, previous_best),
                    progress_score(self.best_validation_score),
                    self.full_validation_evals
                ))
            }
            GepaEventSummary::CandidateAdmitted {
                candidate,
                candidate_index,
            } => {
                self.admitted_count += 1;
                Some(format!(
                    "progress_event=gepa_candidate_admitted candidate={candidate} candidate_index={} admitted_count={}",
                    candidate_index.get(),
                    self.admitted_count
                ))
            }
            GepaEventSummary::OptimizationEnded { best } => Some(format!(
                "progress_event=gepa_optimization_ended best_index={} current_best_validation_score={} baseline_validation_score={} delta_vs_baseline={} accepted_count={} admitted_count={} rejected_count={} skipped_count={} full_validation_evals={}",
                best.map_or_else(|| "none".to_owned(), |index| index.get().to_string()),
                progress_score(self.best_validation_score),
                progress_score(self.seed_validation_score),
                progress_delta(self.best_validation_score, self.seed_validation_score),
                self.accepted_count,
                self.admitted_count,
                self.rejected_count,
                self.skipped_count,
                self.full_validation_evals
            )),
            _ => None,
        }
    }
}

fn parse_gepa_score(score: &str) -> Option<f64> {
    score.parse::<f64>().ok().filter(|score| score.is_finite())
}

fn progress_score(score: Option<f64>) -> String {
    score.map_or_else(|| "absent".to_owned(), |value| format!("{value:.3}"))
}

fn progress_delta(score: Option<f64>, baseline: Option<f64>) -> String {
    match (score, baseline) {
        (Some(score), Some(baseline)) => format!("{:+.3}", score - baseline),
        _ => "absent".to_owned(),
    }
}

fn progress_optional_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "unknown".to_owned(), |value| value.to_string())
}

fn progress_signal(score: Option<f64>, baseline: Option<f64>) -> &'static str {
    match (score, baseline) {
        (Some(score), Some(baseline)) if score > baseline => "improved",
        (Some(score), Some(baseline)) if score < baseline => "regressed",
        (Some(_), Some(_)) => "tied",
        _ => "unknown",
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
    dataset_proof: AimeDatasetProof,
    role_reports: AimeRoleReports,
    optimizer_wall_time: Duration,
    gepa_events: Vec<GepaEventSummary>,
    gepa_report: Option<GepaReport>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeDatasetProof {
    train_count: usize,
    validation_count: usize,
    test_count: usize,
    source_splits: Vec<AimeSourceSplitProof>,
    materialized_cache: Option<AimeMaterializedCacheProof>,
    split_seed: Option<u64>,
    test_repeated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeSourceSplitProof {
    role: String,
    dataset: String,
    config: String,
    split: String,
    count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeMaterializedCacheProof {
    path: String,
    sha256: String,
    bytes: usize,
}

impl AimeDatasetProof {
    fn from_parts(
        train_count: usize,
        validation_count: usize,
        test_count: usize,
        report_metadata: &BTreeMap<CaseId, AimeReportMetadata>,
        materialized_cache: Option<AimeMaterializedCacheProof>,
        split_seed: Option<u64>,
    ) -> Self {
        let mut source_split_counts = BTreeMap::<(String, String, String, String), usize>::new();
        for metadata in report_metadata.values() {
            let key = (
                split_role_label(&metadata.split).to_owned(),
                metadata.source.dataset.clone(),
                metadata.source.config.clone(),
                metadata.source.split.clone(),
            );
            *source_split_counts.entry(key).or_default() += 1;
        }
        let source_splits = source_split_counts
            .into_iter()
            .map(
                |((role, dataset, config, split), count)| AimeSourceSplitProof {
                    role,
                    dataset,
                    config,
                    split,
                    count,
                },
            )
            .collect();
        Self {
            train_count,
            validation_count,
            test_count,
            source_splits,
            materialized_cache,
            split_seed,
            test_repeated: false,
        }
    }
}

#[derive(Clone, Debug)]
struct AimeDataset {
    train: Vec<AimeRunCase>,
    validation: Vec<AimeRunCase>,
    test: Vec<AimeRunCase>,
    report_metadata: BTreeMap<CaseId, AimeReportMetadata>,
    proof: AimeDatasetProof,
}

impl AimeDataset {
    fn from_cache(cache: AimeDatasetCache) -> Result<Self, AimeDatasetError> {
        Self::from_cache_with_proof(cache, None, None)
    }

    fn from_cache_with_proof(
        cache: AimeDatasetCache,
        materialized_cache: Option<AimeMaterializedCacheProof>,
        split_seed: Option<u64>,
    ) -> Result<Self, AimeDatasetError> {
        let mut lowerer = AimeDatasetLowerer::default();
        let train = lowerer.lower_split(&SplitRole::Train, cache.train)?;
        let validation = lowerer.lower_split(&SplitRole::Validation, cache.validation)?;
        let test = lowerer.lower_split(&SplitRole::Test, cache.test)?;
        let proof = AimeDatasetProof::from_parts(
            train.len(),
            validation.len(),
            test.len(),
            &lowerer.report_metadata,
            materialized_cache,
            split_seed,
        );
        Ok(Self {
            train,
            validation,
            test,
            report_metadata: lowerer.report_metadata,
            proof,
        })
    }

    fn reflective_dataset(&self, side_infos: AimeSolverSideInfoStore) -> AimeReflectiveDataset {
        AimeReflectiveDataset {
            inputs_by_case: self
                .train
                .iter()
                .chain(&self.validation)
                .chain(&self.test)
                .map(|case| (case.id, case.input.problem.clone()))
                .collect(),
            side_infos,
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
    let materialized_cache = AimeMaterializedCacheProof {
        path: path.display().to_string(),
        sha256: sha256_hex(&bytes),
        bytes: bytes.len(),
    };
    let cache: AimeDatasetCache = serde_json::from_slice(&bytes).unwrap_or_else(|source| {
        panic!(
            "failed to parse LEAVEN_AIME_CACHE={}: {source}",
            path.display()
        )
    });
    AimeDataset::from_cache_with_proof(cache, Some(materialized_cache), Some(0)).unwrap_or_else(
        |source| {
            panic!(
                "failed to lower LEAVEN_AIME_CACHE={}: {source}",
                path.display()
            )
        },
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut rendered = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut rendered, "{byte:02x}").expect("writing to a String cannot fail");
    }
    rendered
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
struct AimeDspyChatRequest {
    system: String,
    user: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AimeRunOutput {
    answer: String,
    reasoning: String,
    raw: String,
}

#[derive(Clone, Debug)]
struct AimeReflectionSideInfo {
    score: f64,
    input: String,
    prompt: String,
    output: String,
    reasoning: String,
    execution_feedback: String,
}

#[derive(Clone, Debug, Default)]
struct AimeSolverSideInfoStore {
    entries: Arc<Mutex<BTreeMap<AimeSolverSideInfoKey, AimeRunOutput>>>,
}

impl AimeSolverSideInfoStore {
    fn insert(&self, prompt: &AimePrompt, case: CaseId, output: AimeRunOutput) {
        self.entries
            .lock()
            .expect("AIME side-info store lock is not poisoned")
            .insert(
                AimeSolverSideInfoKey {
                    prompt: prompt.system.clone(),
                    case,
                },
                output,
            );
    }

    fn get(&self, prompt: &AimePrompt, case: CaseId) -> Option<AimeRunOutput> {
        self.entries
            .lock()
            .expect("AIME side-info store lock is not poisoned")
            .get(&AimeSolverSideInfoKey {
                prompt: prompt.system.clone(),
                case,
            })
            .cloned()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AimeSolverSideInfoKey {
    prompt: String,
    case: CaseId,
}

fn render_dspy_aime_chain_of_thought_request(
    instructions: &str,
    input: &str,
) -> AimeDspyChatRequest {
    let dedented_instructions = dedent_dspy_instructions(instructions);
    let objective = dedented_instructions
        .lines()
        .map(|line| format!("        {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let objective = if objective.is_empty() {
        String::new()
    } else {
        format!("\n{objective}")
    };
    let system = format!(
        "Your input fields are:\n1. `input` (str): The math problem to solve.\nYour output fields are:\n1. `reasoning` (str): \n2. `answer` (str): The final numerical answer.\nAll interactions will be structured in the following way, with the appropriate values filled in.\n\n[[ ## input ## ]]\n{{input}}\n\n[[ ## reasoning ## ]]\n{{reasoning}}\n\n[[ ## answer ## ]]\n{{answer}}\n\n[[ ## completed ## ]]\nIn adhering to this structure, your objective is: {objective}"
    );
    let user = format!(
        "[[ ## input ## ]]\n{input}\n\nRespond with the corresponding output fields, starting with the field `[[ ## reasoning ## ]]`, then `[[ ## answer ## ]]`, and then ending with the marker for `[[ ## completed ## ]]`."
    );
    AimeDspyChatRequest { system, user }
}

fn render_dspy_aime_json_adapter_request(instructions: &str, input: &str) -> AimeDspyChatRequest {
    let dedented_instructions = dedent_dspy_instructions(instructions);
    let objective = dedented_instructions
        .lines()
        .map(|line| format!("        {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let objective = if objective.is_empty() {
        String::new()
    } else {
        format!("\n{objective}")
    };
    let system = format!(
        "Your input fields are:\n1. `input` (str): The math problem to solve.\nYour output fields are:\n1. `reasoning` (str): \n2. `answer` (str): The final numerical answer.\nAll interactions will be structured in the following way, with the appropriate values filled in.\n\nInputs will have the following structure:\n\n[[ ## input ## ]]\n{{input}}\n\nOutputs will be a JSON object with the following fields.\n\n{{\n  \"reasoning\": \"{{reasoning}}\",\n  \"answer\": \"{{answer}}\"\n}}\nIn adhering to this structure, your objective is: {objective}"
    );
    let user = format!(
        "[[ ## input ## ]]\n{input}\n\nRespond with a JSON object in the following order of fields: `reasoning`, then `answer`."
    );
    AimeDspyChatRequest { system, user }
}

fn dspy_aime_json_adapter_output_schema() -> OutputMode {
    OutputMode::JsonSchema(JsonSchemaOutput {
        name: "DSPyProgramOutputs".to_owned(),
        strict: true,
        schema: serde_json::json!({
            "type": "object",
            "properties": {
                "reasoning": { "type": "string" },
                "answer": { "type": "string" }
            },
            "required": ["reasoning", "answer"],
            "additionalProperties": false
        }),
    })
}

fn dedent_dspy_instructions(instructions: &str) -> String {
    let common_indent = instructions
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(line_indent_width)
        .min()
        .unwrap_or(0);
    if common_indent == 0 {
        return instructions.to_owned();
    }

    instructions
        .lines()
        .map(|line| strip_indent_width(line, common_indent))
        .collect::<Vec<_>>()
        .join("\n")
}

fn line_indent_width(line: &str) -> Option<usize> {
    let mut width = 0;
    for ch in line.chars() {
        match ch {
            ' ' | '\t' => width += 1,
            _ => return Some(width),
        }
    }
    None
}

fn strip_indent_width(line: &str, width: usize) -> &str {
    let mut byte_index = 0;
    for (stripped, (index, ch)) in line.char_indices().enumerate() {
        if stripped >= width || (ch != ' ' && ch != '\t') {
            byte_index = index;
            break;
        }
        byte_index = index + ch.len_utf8();
    }
    &line[byte_index..]
}

fn parse_dspy_aime_chain_of_thought_response(raw: &str) -> Result<AimeRunOutput, String> {
    let mut fields = BTreeMap::<String, String>::new();
    let mut current: Option<String> = None;
    let mut buffer = Vec::<String>::new();
    for line in raw.lines() {
        if let Some((field, remaining)) = dspy_field_header(line.trim()) {
            if let Some(name) = current.replace(field) {
                fields
                    .entry(name)
                    .or_insert_with(|| buffer.join("\n").trim().to_owned());
                buffer.clear();
            }
            if !remaining.is_empty() {
                buffer.push(remaining);
            }
        } else {
            buffer.push(line.to_owned());
        }
    }
    if let Some(name) = current {
        fields
            .entry(name)
            .or_insert_with(|| buffer.join("\n").trim().to_owned());
    }
    let reasoning = fields
        .get("reasoning")
        .cloned()
        .ok_or_else(|| "missing DSPy `reasoning` field".to_owned())?;
    let answer = fields
        .get("answer")
        .cloned()
        .ok_or_else(|| "missing DSPy `answer` field".to_owned())?;
    Ok(AimeRunOutput {
        answer,
        reasoning,
        raw: raw.to_owned(),
    })
}

fn parse_dspy_aime_json_adapter_response(raw: &str) -> Result<AimeRunOutput, String> {
    let value = serde_json::from_str::<serde_json::Value>(raw)
        .or_else(|_| {
            let start = raw.find('{').ok_or_else(|| {
                serde_json::Error::io(std::io::Error::other("missing JSON object"))
            })?;
            let end = raw.rfind('}').ok_or_else(|| {
                serde_json::Error::io(std::io::Error::other("missing JSON object"))
            })?;
            serde_json::from_str(&raw[start..=end])
        })
        .map_err(|source| format!("DSPy JSONAdapter response was not JSON: {source}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "DSPy JSONAdapter response was not a JSON object".to_owned())?;
    let reasoning = object
        .get("reasoning")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "missing DSPy JSONAdapter `reasoning` field".to_owned())?
        .to_owned();
    let answer = object
        .get("answer")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "missing DSPy JSONAdapter `answer` field".to_owned())?
        .to_owned();
    Ok(AimeRunOutput {
        answer,
        reasoning,
        raw: raw.to_owned(),
    })
}

fn dspy_field_header(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("[[ ## ")?;
    let (field, remaining) = rest.split_once(" ## ]]")?;
    if field
        .chars()
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        Some((field.to_owned(), remaining.trim().to_owned()))
    } else {
        None
    }
}

fn aime_reflection_side_info_example(
    info: AimeReflectionSideInfo,
) -> Vec<(String, ReflectiveSideInfoValue)> {
    vec![
        ("score".to_owned(), format!("{:.1}", info.score).into()),
        ("input".to_owned(), info.input.into()),
        ("prompt".to_owned(), info.prompt.into()),
        ("output".to_owned(), info.output.into()),
        ("reasoning".to_owned(), info.reasoning.into()),
        (
            "execution_feedback".to_owned(),
            info.execution_feedback.into(),
        ),
    ]
}

fn output_record_text(output: &OutputRecord) -> String {
    match output {
        OutputRecord::Inline { text, .. } => text.clone(),
        OutputRecord::BlobRef(reference) => format!("blob:{}:{}", reference.store, reference.key),
    }
}

#[derive(Clone, Debug)]
struct AimeReflectiveDataset {
    inputs_by_case: BTreeMap<CaseId, String>,
    side_infos: AimeSolverSideInfoStore,
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
    ) -> Result<Vec<ReflectiveCase>, ReflectionError> {
        let mut examples = Vec::with_capacity(parent_assessments.len());
        let parent_prompt = ctx.graph().artifact(parent).ok_or_else(|| {
            ReflectionError::builder(format!(
                "AIME reflection parent candidate `{parent}` is missing from graph"
            ))
        })?;
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
            let trace = self.side_infos.get(parent_prompt, case);
            let output = trace.as_ref().map_or_else(
                || output_record_text(evidence.output()),
                |trace| trace.answer.clone(),
            );
            let reasoning = trace.map_or_else(String::new, |trace| trace.reasoning);
            let input = self.inputs_by_case.get(&case).cloned().unwrap_or_default();
            let feedback = evidence.feedback().to_owned();
            let mut reflective_case = ReflectiveCase::from_example(
                ReflectiveValue::Text(input.clone()),
                None,
                Some(ReflectiveValue::Text(output.clone())),
                Some(evidence.score().score()),
                feedback.clone(),
            );
            reflective_case.case_id = Some(case);
            reflective_case.runs[0].attempt_index = None;
            reflective_case.runs[0].side_info =
                aime_reflection_side_info_example(AimeReflectionSideInfo {
                    score: evidence.score().score(),
                    input: input.clone(),
                    prompt: parent_prompt.system.clone(),
                    output: output.clone(),
                    reasoning: reasoning.clone(),
                    execution_feedback: feedback.clone(),
                });
            reflective_case
                .source_refs
                .push(InfoRef::Assessment(*parent_assessment));
            examples.push(reflective_case);
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
    EagerSqlite(CachedLm<OpenAiLm, AimeEagerSqliteLmCache>),
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
            Self::EagerSqlite(inner) => inner.id(),
            Self::Unavailable { id, .. } => id.clone(),
        }
    }

    fn fingerprint(&self) -> Fingerprint {
        match self {
            Self::InMemory(inner) => inner.fingerprint(),
            Self::Sqlite(inner) => inner.fingerprint(),
            Self::EagerSqlite(inner) => inner.fingerprint(),
            Self::Unavailable { fingerprint, .. } => *fingerprint,
        }
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        match self {
            Self::InMemory(inner) => inner.complete(request).await,
            Self::Sqlite(inner) => inner.complete(request).await,
            Self::EagerSqlite(inner) => inner.complete(request).await,
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
    let config = match openai_config_for_cache_policy(cache_policy, runtime) {
        Ok(config) => config,
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
        AimeLmCacheBackend::EagerSqlite => {
            let cache = match AimeEagerSqliteLmCache::open(run_dir) {
                Ok(cache) => cache,
                Err(source) => {
                    return unavailable_openai_lm(
                        role,
                        AimeOpenAiUnavailableReason::Cache,
                        format!("failed to open eager SQLite LM cache for {role}: {source}"),
                    );
                }
            };
            AimeOpenAiLm {
                inner: AimeOpenAiCachedLm::EagerSqlite(CachedLm::new(inner, cache, cache_policy)),
            }
        }
    }
}

fn openai_config_for_cache_policy(
    cache_policy: LmCachePolicy,
    runtime: AimeOpenAiRuntimeConfig,
) -> Result<OpenAiConfig, LmError> {
    let config = if cache_policy == LmCachePolicy::CacheOnly {
        OpenAiConfig::new("p8-aime-cache-only-placeholder")
    } else {
        OpenAiConfig::from_env()?
    };
    Ok(apply_aime_openai_runtime_config(config, runtime))
}

#[derive(Clone)]
struct AimeEagerSqliteLmCache {
    workspace: SqliteLmCache,
    run_dir: SqliteLmCache,
}

impl AimeEagerSqliteLmCache {
    fn open(run_dir: &Path) -> Result<Self, LmCacheError> {
        Self::open_with_workspace(run_dir, Path::new("."))
    }

    fn open_with_workspace(run_dir: &Path, workspace_root: &Path) -> Result<Self, LmCacheError> {
        Ok(Self {
            workspace: SqliteLmCache::open_workspace(workspace_root)?,
            run_dir: SqliteLmCache::open_run_dir(run_dir)?,
        })
    }
}

impl LmCacheStore for AimeEagerSqliteLmCache {
    async fn get(&self, key: LmCacheKey) -> Result<Option<LmCacheEntry>, LmCacheError> {
        if let Some(entry) = self.run_dir.get(key).await? {
            return Ok(Some(entry));
        }
        self.workspace.get(key).await
    }

    async fn put(&self, key: LmCacheKey, entry: LmCacheEntry) -> Result<(), LmCacheError> {
        self.workspace.put(key, entry).await
    }
}

fn openai_provider_fingerprint_for_runtime(runtime: AimeOpenAiRuntimeConfig) -> Fingerprint {
    OpenAiLm::new(apply_aime_openai_runtime_config(
        OpenAiConfig::new("p8-aime-fingerprint-placeholder"),
        runtime,
    ))
    .fingerprint()
}

fn apply_aime_openai_runtime_config(
    config: OpenAiConfig,
    runtime: AimeOpenAiRuntimeConfig,
) -> OpenAiConfig {
    config
        .with_throttle_policy(OpenAiThrottlePolicy::new(
            runtime.max_concurrent_requests,
            Duration::ZERO,
        ))
        .with_request_timeout(Duration::from_secs(runtime.request_timeout_seconds))
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
    side_infos: AimeSolverSideInfoStore,
) -> Result<RunOutput<AimeRunOutput>, RunError> {
    if let Some(solver) = solver {
        return run_openai_solver(solver, &prompt, &case, &solver_config, side_infos).await;
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
    let output = AimeRunOutput {
        answer: answer.to_string(),
        reasoning: "deterministic fixture evaluated the target-safe arithmetic rule".to_owned(),
        raw: answer.to_string(),
    };
    side_infos.insert(&prompt, case.id(), output.clone());
    let reasoning_trace = format!("reasoning: {}", output.reasoning);
    Ok(RunOutput::typed(output).with_trace(reasoning_trace))
}

async fn run_openai_solver<L>(
    solver: AimeInstrumentedLm<L>,
    prompt: &AimePrompt,
    case: &RunCase<AimeInput>,
    solver_config: &AimeSolverConfig,
    side_infos: AimeSolverSideInfoStore,
) -> Result<RunOutput<AimeRunOutput>, RunError>
where
    L: Lm,
{
    let (output, cost) = complete_openai_solver_output(
        &solver,
        &prompt.system,
        &case.input().problem,
        solver_config,
    )
    .await?;
    side_infos.insert(prompt, case.id(), output.clone());
    let reasoning_trace = format!("reasoning: {}", output.reasoning);
    let raw_trace = format!("raw_response: {}", output.raw);
    Ok(RunOutput::typed(output)
        .with_trace(reasoning_trace)
        .with_trace(raw_trace)
        .with_cost(cost))
}

async fn complete_openai_solver_output<L>(
    solver: &AimeInstrumentedLm<L>,
    prompt_system: &str,
    problem: &str,
    solver_config: &AimeSolverConfig,
) -> Result<(AimeRunOutput, Cost), RunError>
where
    L: Lm,
{
    let dspy_request = render_dspy_aime_chain_of_thought_request(prompt_system, problem);
    let request = LmRequest::new(
        solver_config.model.clone(),
        Messages::new()
            .with_system(dspy_request.system)
            .with_user(dspy_request.user),
    )
    .with_sampling(solver_config.sampling.clone());
    let metered = solver
        .complete(request)
        .await
        .map_err(|source| RunError::with_source("AIME solver LM failed", source))?;
    let raw = metered.value.assistant.content().trim().to_owned();
    let (output, cost) = match parse_dspy_aime_chain_of_thought_response(&raw) {
        Ok(output) => (output, metered.cost),
        Err(chat_parse_error) => {
            let dspy_request = render_dspy_aime_json_adapter_request(prompt_system, problem);
            let fallback_request = LmRequest::new(
                solver_config.model.clone(),
                Messages::new()
                    .with_system(dspy_request.system)
                    .with_user(dspy_request.user),
            )
            .with_sampling(solver_config.sampling.clone())
            .with_output(dspy_aime_json_adapter_output_schema());
            let fallback = solver.complete(fallback_request).await.map_err(|source| {
                RunError::with_source("AIME solver JSONAdapter fallback LM failed", source)
                    .with_trace(chat_parse_error.clone())
                    .with_cost(metered.cost.clone())
            })?;
            let fallback_raw = fallback.value.assistant.content().trim().to_owned();
            match parse_dspy_aime_json_adapter_response(&fallback_raw) {
                Ok(output) => (output, metered.cost.combine(&fallback.cost)),
                Err(json_parse_error) => {
                    solver.record_failure_kind(AimeProviderFailureKind::AnswerParse);
                    return Err(RunError::new(
                        "AIME solver response did not match DSPy ChatAdapter or JSONAdapter fields",
                    )
                    .with_trace(chat_parse_error)
                    .with_trace(json_parse_error)
                    .with_cost(metered.cost.combine(&fallback.cost)));
                }
            }
        }
    };
    Ok((output, cost))
}

#[cfg(test)]
fn parse_openai_solver_response<L>(
    solver: &AimeInstrumentedLm<L>,
    raw: &str,
    cost: &Cost,
) -> Result<AimeRunOutput, RunError> {
    parse_dspy_aime_chain_of_thought_response(raw).map_err(|source| {
        solver.record_failure_kind(AimeProviderFailureKind::AnswerParse);
        RunError::new("AIME solver response did not match DSPy ChainOfThought fields")
            .with_trace(source)
            .with_cost(cost.clone())
    })
}

fn openai_model_name() -> String {
    std::env::var("LEAVEN_OPENAI_MODEL").unwrap_or_else(|_| GEPA_AIME_SOLVER_MODEL.to_owned())
}

fn aime_runner_fingerprint(
    config: &AimeSolverConfig,
    solver_lm_fingerprint: Option<Fingerprint>,
) -> Fingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"p8-aime-runner.dspy-chain-of-thought-json-fallback.v2");
    builder.update([u8::from(config.live)]);
    builder.update(config.model.as_bytes());
    builder.update(
        serde_json::to_vec(&config.sampling).expect("AIME solver sampling config serializes"),
    );
    builder.update(config.runtime.max_concurrent_requests.get().to_le_bytes());
    builder.update(config.runtime.request_timeout_seconds.to_le_bytes());
    if let Some(fingerprint) = solver_lm_fingerprint {
        builder.update(b"solver-lm");
        builder.update(fingerprint.0);
    } else {
        builder.update(b"solver-lm:none");
    }
    builder.finish()
}

fn aime_solver_request_shape_fingerprint() -> Fingerprint {
    let request = render_dspy_aime_chain_of_thought_request("<instructions>", "<input>");
    let fallback = render_dspy_aime_json_adapter_request("<instructions>", "<input>");
    let mut builder = FingerprintBuilder::new();
    builder.update(b"p8-aime-solver-request-shape.v2");
    builder.update(b"upstream:dspy.ChatAdapter->JSONAdapter");
    builder.update(b"adapter:dspy-chain-of-thought");
    builder.update(b"system");
    builder.update(request.system.as_bytes());
    builder.update(b"user");
    builder.update(request.user.as_bytes());
    builder.update(b"fallback-adapter:dspy-json-adapter");
    builder.update(b"fallback-system");
    builder.update(fallback.system.as_bytes());
    builder.update(b"fallback-user");
    builder.update(fallback.user.as_bytes());
    builder.update(b"fallback-output");
    builder.update(
        serde_json::to_vec(&dspy_aime_json_adapter_output_schema())
            .expect("DSPy JSONAdapter output schema serializes"),
    );
    builder.finish()
}

fn aime_solver_request_example(config: &AimeSolverConfig) -> AimeLmRequestRecord {
    let request = render_dspy_aime_chain_of_thought_request("<instructions>", "<input>");
    let lm_request = LmRequest::new(
        config.model.clone(),
        Messages::new()
            .with_system(request.system)
            .with_user(request.user),
    )
    .with_sampling(config.sampling.clone());
    AimeLmRequestRecord::from_request(&lm_request)
}

fn aime_reflection_role_fingerprint(
    config: &AimeReflectionConfig,
    reflection_lm_fingerprint: Fingerprint,
) -> Fingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"p8-aime-reflection-role.v1");
    builder.update([u8::from(config.live)]);
    builder.update(config.model.as_bytes());
    builder.update(
        serde_json::to_vec(&config.sampling).expect("AIME reflection sampling config serializes"),
    );
    builder.update(config.runtime.max_concurrent_requests.get().to_le_bytes());
    builder.update(config.runtime.request_timeout_seconds.to_le_bytes());
    builder.update(b"reflection-lm");
    builder.update(reflection_lm_fingerprint.0);
    builder.update(b"output:text");
    builder.update(b"parser:plain-text-fenced");
    builder.update(b"prompt:optimize-anything");
    builder.finish()
}

fn aime_reflection_request_shape_fingerprint() -> Fingerprint {
    let lm_request = canonical_aime_reflection_request();
    let rendered = lm_request
        .messages
        .iter()
        .map(Message::content)
        .collect::<Vec<_>>()
        .join("\n");
    let mut builder = FingerprintBuilder::new();
    builder.update(b"p8-aime-reflection-request-shape.v1");
    builder.update(b"upstream:gepa.optimize_anything");
    builder.update(b"renderer:gepa-default-markdown-side-info");
    builder.update(rendered.as_bytes());
    builder.finish()
}

fn aime_reflection_request_example(config: &AimeReflectionConfig) -> AimeLmRequestRecord {
    AimeLmRequestRecord::from_request(&canonical_aime_reflection_request_with_sampling(
        config.model.clone().into(),
        config.sampling.clone(),
    ))
}

fn canonical_aime_reflection_request() -> LmRequest {
    canonical_aime_reflection_request_with_sampling(
        "shape-model".into(),
        SamplingOptions::default(),
    )
}

fn canonical_aime_reflection_request_with_sampling(
    model: leaven_lm::ModelName,
    sampling: SamplingOptions,
) -> LmRequest {
    let config = LmBackedReflectorConfig {
        sampling,
        output: leaven_lm::OutputMode::Text,
        prompt_template: Some(OPTIMIZE_ANYTHING_REFLECTION_PROMPT_TEMPLATE.to_owned()),
    };
    let artifact = AimePrompt::new("<curr_param>");
    let surface = AimePromptSurface;
    let request =
        ReflectRequest::for_part(CandidateId::new(), "system", "system").with_examples([{
            let mut case = ReflectiveCase::from_example(
                ReflectiveValue::default(),
                None,
                None,
                None,
                String::new(),
            );
            case.runs[0].side_info = aime_reflection_side_info_example(AimeReflectionSideInfo {
                score: 0.0,
                input: "<input>".to_owned(),
                prompt: "<prompt>".to_owned(),
                output: "<output>".to_owned(),
                reasoning: "<reasoning>".to_owned(),
                execution_feedback: "<execution_feedback>".to_owned(),
            });
            case
        }]);
    DefaultReflectionRenderer
        .render(ReflectionRenderInput::<
            RunProblem<AimePrompt, AimeInput, AimeTarget>,
            AimePromptSurface,
        > {
            request: &request,
            artifact: &artifact,
            surface: &surface,
            model,
            config: &config,
        })
        .expect("canonical AIME reflection prompt renders")
}

fn aime_scorer_fingerprint() -> Fingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"p8-aime-scorer.exact-integer.v1");
    builder.update(b"target-answer-integer");
    builder.update(b"solution-feedback-visible-to-scorer");
    builder.finish()
}

async fn score_answer(
    ctx: ScoreContext<AimePrompt, AimeInput, AimeTarget, AimeRunOutput>,
) -> Result<Score, ScoreError> {
    let target = ctx
        .case
        .target()
        .ok_or_else(|| ScoreError::new("AIME scorer requires a target answer"))?;
    let (score, feedback) = aime_score_feedback(target, &ctx.output.output.answer);
    Ok(Score::new(score, feedback).with_text_output(ctx.output.output.answer))
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
    if target.solution.is_empty() {
        String::new()
    } else {
        format!(
            " Here's the full step-by-step solution:\n{}\n\nThink about what takeaways you can learn from this solution to improve your future answers and approach to similar problems",
            target.solution
        )
    }
}

fn aime_score_feedback(target: &AimeTarget, raw_answer: &str) -> (f64, String) {
    let correct_answer = target.answer.integer;
    let solution_suffix = solution_feedback(target);
    match raw_answer.parse::<i64>() {
        Ok(answer) => {
            let score = f64::from(answer == correct_answer);
            let status = if answer == correct_answer {
                "correct"
            } else {
                "incorrect"
            };
            (
                score,
                format!(
                    "Your answer is {status}. The correct answer is '{correct_answer}'.{solution_suffix}"
                ),
            )
        }
        Err(_) => (
            0.0,
            format!(
                "The final answer must be a valid integer and nothing else. You responded with '{raw_answer}', which couldn't be parsed as a python integer. Please ensure your answer is a valid integer without any additional text or formatting. The correct answer is '{correct_answer}'.{solution_suffix}{}",
                if target.solution.is_empty() {
                    ""
                } else {
                    " and ensure your final answer is a valid integer."
                }
            ),
        ),
    }
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

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", RunId::new()));
        fs::create_dir_all(&path).expect("temporary test directory can be created");
        path
    }

    fn block_on_tokio<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Runtime::new()
            .expect("tokio test runtime")
            .block_on(future)
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
        assert_eq!(dataset.proof.train_count, 3);
        assert_eq!(dataset.proof.validation_count, 1);
        assert_eq!(dataset.proof.test_count, 2);
        assert_eq!(dataset.proof.split_seed, None);
        assert!(!dataset.proof.test_repeated);
        assert_eq!(dataset.proof.materialized_cache, None);
        assert!(
            dataset
                .proof
                .source_splits
                .iter()
                .any(|source| source.role == "train"
                    && source.dataset == "deterministic"
                    && source.config == "default"
                    && source.split == "train"
                    && source.count == 3)
        );
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
    fn gepa_progress_reports_validation_signal_before_final_report() {
        let mut progress = AimeGepaProgress::default();
        let seed = progress
            .progress_line(&GepaEventSummary::SeedValidationCompleted {
                candidate_index: GepaCandidateIndex::new(0),
                metric_calls_delta: 45,
                score: "0.467".to_owned(),
            })
            .expect("seed validation emits progress");
        let accepted = progress
            .progress_line(&GepaEventSummary::AcceptedValidationCompleted {
                candidate_index: GepaCandidateIndex::new(1),
                metric_calls_delta: 44,
                score: "0.578".to_owned(),
            })
            .expect("accepted validation emits progress");
        let iteration = progress
            .progress_line(&GepaEventSummary::IterationStarted { iteration: 2 })
            .expect("iteration emits current signal");

        assert_eq!(
            seed,
            "progress_event=gepa_seed_validation_completed candidate_index=0 validation_score=0.467 current_best_validation_score=0.467 metric_calls_delta=45 full_validation_evals=1"
        );
        assert!(
            accepted.starts_with(
                "progress_event=gepa_accepted_validation_completed candidate_index=1 "
            )
        );
        assert!(accepted.contains("validation_score=0.578"));
        assert!(accepted.contains("baseline_validation_score=0.467"));
        assert!(accepted.contains("delta_vs_baseline=+0.111"));
        assert!(accepted.contains("previous_best_validation_score=0.467"));
        assert!(accepted.contains("delta_vs_previous_best=+0.111"));
        assert!(accepted.contains("signal=improved"));
        assert!(accepted.contains("current_best_validation_score=0.578"));
        assert!(accepted.contains("full_validation_evals=2"));
        assert!(iteration.contains("current_best_validation_score=0.578"));
        assert!(iteration.contains("baseline_validation_score=0.467"));
        assert!(iteration.contains("full_validation_evals=2"));
    }

    #[test]
    fn gepa_progress_reports_train_screen_delta() {
        let mut progress = AimeGepaProgress::default();
        progress
            .progress_line(&GepaEventSummary::ParentEvaluated {
                metric_calls_delta: 3,
                score: "0.333".to_owned(),
            })
            .expect("parent evaluation emits progress");
        let child = progress
            .progress_line(&GepaEventSummary::ChildEvaluated {
                metric_calls_delta: 3,
                score: "0.667".to_owned(),
            })
            .expect("child evaluation emits progress");

        assert_eq!(
            child,
            "progress_event=gepa_child_evaluated train_screen_score=0.667 parent_train_screen_score=0.333 delta_vs_parent=+0.334 signal=improved metric_calls_delta=3"
        );
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
        assert!(
            result
                .summary
                .evaluation
                .splits_reported
                .iter()
                .all(|split| split.candidates.len() == 2)
        );
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

        let gepa_report = run
            .gepa_report
            .as_ref()
            .expect("public optimize path exposes typed GEPA report");
        assert_eq!(gepa_report.best_index.map(GepaCandidateIndex::get), Some(1));
        assert!(gepa_report.best_candidate.is_some());
        assert_eq!(
            gepa_report
                .validation_best_index
                .map(GepaCandidateIndex::get),
            Some(1)
        );
        assert!(gepa_report.validation_best_candidate.is_some());
        assert_eq!(gepa_report.candidates[0].index.get(), 0);
        assert_eq!(gepa_report.candidates[0].parents, Vec::new());
        assert_eq!(gepa_report.candidates[1].index.get(), 1);
        assert_eq!(
            gepa_report.candidates[1].parents,
            vec![GepaCandidateIndex::new(0)]
        );
        assert!(
            gepa_report.candidates[1]
                .validation_subscores
                .iter()
                .any(|row| row.score > 0.0)
        );
        assert!(
            gepa_report
                .validation_frontier
                .iter()
                .any(|case| case.candidates.contains(&GepaCandidateIndex::new(1)))
        );
        assert!(!gepa_report.candidate_history.is_empty());
        assert_eq!(gepa_report.proposal_attempts.len(), 1);
        let attempt = &gepa_report.proposal_attempts[0];
        assert_eq!(attempt.attempt_index, 1);
        assert_eq!(attempt.parent_index.get(), 0);
        assert!(!attempt.parent_assessments.is_empty());
        assert!(!attempt.parent_cases.is_empty());
        assert_eq!(
            attempt.reflective_example_count,
            Some(attempt.parent_cases.len())
        );
        assert!(attempt.child.is_some());
        assert!(!attempt.child_assessments.is_empty());
        assert_eq!(attempt.parent_cases, attempt.child_cases);
        assert_eq!(attempt.accepted, Some(true));
        assert_eq!(attempt.admitted_index.map(GepaCandidateIndex::get), Some(1));
        assert_eq!(attempt.skip_reason, None);
        assert_eq!(gepa_report.total_metric_calls, 8);
        assert_eq!(gepa_report.full_validation_evals, 2);
        assert!(gepa_report.skip_perfect_score);
        assert!((gepa_report.perfect_score - 1.0).abs() < f64::EPSILON);
        assert!(
            gepa_report
                .events
                .iter()
                .any(|event| matches!(event, GepaEventSummary::FrontierUpdated))
        );
    }

    #[test]
    fn reference_gepa_requires_validation_instead_of_silent_train_only_fallback() {
        let config = AimeRunConfig::deterministic_smoke();
        let mut dataset = deterministic_dataset();
        dataset.validation.clear();
        dataset.test.clear();
        let error = block_on(try_run_aime(config, dataset)).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("requires a non-empty validation set")
        );
    }

    #[test]
    fn run_builder_requires_score_function() {
        let config = AimeRunConfig::deterministic_smoke();
        let solver_config = config.solver.clone();
        let dataset = deterministic_dataset();
        let side_infos = AimeSolverSideInfoStore::default();
        let reflective_dataset = dataset.reflective_dataset(side_infos.clone());
        let run_id = RunId::new();
        let run_dir = leaven::run::default_local_run_dir(run_id);
        let error = block_on(async {
            Box::pin(
                    leaven::prelude::optimize(AimePrompt::new(config.seed_prompt))
                        .train(dataset.train)
                        .runner(move |prompt, case| {
                            let solver_config = solver_config.clone();
                            let side_infos = side_infos.clone();
                            async move {
                                run_solver(prompt, case, None, solver_config, side_infos).await
                            }
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
    fn configured_gepa_aime_profile_matches_optimize_anything_knobs() {
        let config = AimeRunConfig::gepa_aime();

        assert_eq!(config.profile, AimeRunProfile::GepaAime);
        assert_eq!(config.gepa_profile, GepaProfile::OptimizeAnything);
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
    fn live_profile_evaluator_fanout_tracks_provider_concurrency() {
        let runtime = AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("sqlite"), None);
        let config = AimeRunConfig::live_openai_with_controls(
            AimeRunProfile::GepaAime,
            AimeDataSource::HuggingFaceCache,
            GEPA_AIME_METRIC_CALLS,
            AimeLmCachePolicies::from_values(Some("read-write"), Some("read-write")),
            runtime,
        );

        assert_eq!(
            config.evaluation_parallelism,
            runtime.max_concurrent_requests
        );
        assert_eq!(
            config.solver.runtime.max_concurrent_requests,
            runtime.max_concurrent_requests
        );
        assert_eq!(
            config.reflection.runtime.max_concurrent_requests,
            runtime.max_concurrent_requests
        );
    }

    #[test]
    fn p8_role_fingerprints_include_observed_openai_provider_runtime() {
        let runtime = AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("sqlite"), Some("120"));
        let timeout_runtime =
            AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("sqlite"), Some("600"));
        let base_config = AimeRunConfig::live_openai_with_controls(
            AimeRunProfile::GepaAime,
            AimeDataSource::HuggingFaceCache,
            GEPA_AIME_METRIC_CALLS,
            AimeLmCachePolicies::from_values(Some("read-write"), Some("read-write")),
            runtime,
        );
        let default_provider = openai_provider_fingerprint_for_runtime(runtime);
        let timeout_provider = openai_provider_fingerprint_for_runtime(timeout_runtime);
        let base_url_provider = OpenAiLm::new(apply_aime_openai_runtime_config(
            OpenAiConfig::new("test-key")
                .with_base_url("https://proxy.example.invalid/v1/responses"),
            runtime,
        ))
        .fingerprint();
        let retry_provider = OpenAiLm::new(apply_aime_openai_runtime_config(
            OpenAiConfig::new("test-key").with_retry_policy(OpenAiRetryPolicy::none()),
            runtime,
        ))
        .fingerprint();

        assert_ne!(default_provider, timeout_provider);
        assert_ne!(default_provider, base_url_provider);
        assert_ne!(default_provider, retry_provider);
        assert_ne!(
            aime_runner_fingerprint(&base_config.solver, Some(default_provider)),
            aime_runner_fingerprint(&base_config.solver, Some(timeout_provider))
        );
        assert_ne!(
            aime_runner_fingerprint(&base_config.solver, Some(default_provider)),
            aime_runner_fingerprint(&base_config.solver, Some(base_url_provider))
        );
        assert_ne!(
            aime_reflection_role_fingerprint(&base_config.reflection, default_provider),
            aime_reflection_role_fingerprint(&base_config.reflection, retry_provider)
        );
    }

    #[test]
    fn p8_role_fingerprints_ignore_lm_cache_replay_controls() {
        let runtime = AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("sqlite"), Some("600"));
        let eager_runtime =
            AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("eager-sqlite"), Some("600"));
        let read_write = AimeRunConfig::live_openai_with_controls(
            AimeRunProfile::GepaAime,
            AimeDataSource::HuggingFaceCache,
            GEPA_AIME_METRIC_CALLS,
            AimeLmCachePolicies::from_values(Some("read-write"), Some("read-write")),
            runtime,
        );
        let cache_only = AimeRunConfig::live_openai_with_controls(
            AimeRunProfile::GepaAime,
            AimeDataSource::HuggingFaceCache,
            GEPA_AIME_METRIC_CALLS,
            AimeLmCachePolicies::from_values(Some("cache-only"), Some("cache-only")),
            eager_runtime,
        );
        let provider = openai_provider_fingerprint_for_runtime(runtime);

        assert_eq!(
            aime_runner_fingerprint(&read_write.solver, Some(provider)),
            aime_runner_fingerprint(&cache_only.solver, Some(provider)),
            "switching from paid read-write to eager cache-only replay must not block resume"
        );
        assert_eq!(
            aime_reflection_role_fingerprint(&read_write.reflection, provider),
            aime_reflection_role_fingerprint(&cache_only.reflection, provider),
            "reflection role identity should track model/runtime/prompt shape, not cache transport"
        );
    }

    #[test]
    fn cache_only_openai_lm_uses_provider_identity_without_credentials() {
        let runtime = AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("sqlite"), Some("600"));
        let run_dir = unique_temp_dir("p8-aime-cache-only");
        let lm = cached_openai_lm(LmCachePolicy::CacheOnly, runtime, &run_dir, "live solver");

        assert_eq!(
            lm.fingerprint(),
            openai_provider_fingerprint_for_runtime(runtime)
        );
        let request = LmRequest::new("gpt-4.1-mini", Messages::from_user("cached?"));
        let error = block_on_tokio(lm.complete(request)).expect_err("empty cache must fail closed");
        assert!(
            matches!(error, LmError::Cache { .. }),
            "cache-only replay should fail as a cache miss, not as missing credentials: {error}"
        );

        let _ = fs::remove_dir_all(run_dir);
    }

    #[test]
    fn eager_sqlite_cache_prefers_run_dir_and_writes_workspace() {
        let root = unique_temp_dir("p8-aime-eager-cache");
        let run_dir = root.join("run");
        let workspace = root.join("workspace");
        let runtime = AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("sqlite"), Some("600"));
        let provider = openai_provider_fingerprint_for_runtime(runtime);
        let request = LmRequest::new("gpt-4.1-mini", Messages::from_user("cached request"));
        let key = LmCacheKey::for_request(provider, &request);
        let run_dir_cache = SqliteLmCache::open_run_dir(&run_dir).expect("run-dir cache opens");
        let workspace_cache =
            SqliteLmCache::open_workspace(&workspace).expect("workspace cache opens");
        let response = LmResponse::new(
            Message::new(Role::Assistant, "from run dir"),
            TokenUsage::default(),
        )
        .expect("assistant response");
        block_on_tokio(run_dir_cache.put(
            key,
            LmCacheEntry::new(key, provider, request.clone(), response.clone()),
        ))
        .expect("run-dir cache write");
        let stale_workspace_response = LmResponse::new(
            Message::new(Role::Assistant, "from workspace"),
            TokenUsage::default(),
        )
        .expect("assistant response");
        block_on_tokio(workspace_cache.put(
            key,
            LmCacheEntry::new(key, provider, request, stale_workspace_response),
        ))
        .expect("workspace cache write");

        let eager =
            AimeEagerSqliteLmCache::open_with_workspace(&run_dir, &workspace).expect("eager cache");
        let from_run_dir = block_on_tokio(eager.get(key))
            .expect("eager cache read")
            .expect("run-dir fallback hit");
        assert_eq!(from_run_dir.response, response);

        let workspace_request =
            LmRequest::new("gpt-4.1-mini", Messages::from_user("workspace request"));
        let workspace_key = LmCacheKey::for_request(provider, &workspace_request);
        let workspace_response = LmResponse::new(
            Message::new(Role::Assistant, "to workspace"),
            TokenUsage::default(),
        )
        .expect("assistant response");
        block_on_tokio(eager.put(
            workspace_key,
            LmCacheEntry::new(
                workspace_key,
                provider,
                workspace_request,
                workspace_response.clone(),
            ),
        ))
        .expect("eager cache write");
        let from_workspace = block_on_tokio(workspace_cache.get(workspace_key))
            .expect("workspace cache read")
            .expect("workspace write-through hit");
        assert_eq!(from_workspace.response, workspace_response);

        let _ = fs::remove_dir_all(root);
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
    fn p8_reflection_uses_upstream_optimize_anything_template() {
        const UPSTREAM_OPTIMIZE_ANYTHING_TEMPLATE: &str = r"I am optimizing a parameter in my system. The current parameter value is:
```
<curr_param>
```

Below is evaluation data showing how this parameter value performed across multiple test cases. The data contains performance metrics, diagnostic information, and other relevant details from the evaluation:
```
<side_info>
```

Your task is to propose a new, improved parameter value that can be used as a drop-in replacement for the current one.

Carefully analyze all the evaluation data provided above. Look for patterns that indicate what works and what doesn't. Pay special attention to:
- Performance metrics and how they correlate with parameter behavior
- Recurring issues, errors, or failure patterns across multiple test cases
- Successful patterns or behaviors that should be preserved or enhanced
- Any domain-specific requirements, constraints, or factual information revealed in the evaluation data
- Specific technical details that are crucial for understanding the parameter's role

Based on your analysis, propose a new parameter value that addresses the identified issues while maintaining or improving upon what works well. Your proposal should be directly informed by the patterns and insights from the evaluation data.

Provide the new parameter value within ``` blocks.";

        let config = AimeRunConfig::gepa_aime();
        let reflector = aime_reflector_config(&config.reflection);

        assert_eq!(
            reflector.prompt_template.as_deref(),
            Some(UPSTREAM_OPTIMIZE_ANYTHING_TEMPLATE)
        );
    }

    #[test]
    fn aime_solver_request_matches_dspy_chain_of_thought_chat_adapter() {
        let request = render_dspy_aime_chain_of_thought_request(
            "Solve the math problem carefully.",
            "What is 19 + 23?",
        );

        assert_eq!(
            request.system,
            "Your input fields are:\n1. `input` (str): The math problem to solve.\nYour output fields are:\n1. `reasoning` (str): \n2. `answer` (str): The final numerical answer.\nAll interactions will be structured in the following way, with the appropriate values filled in.\n\n[[ ## input ## ]]\n{input}\n\n[[ ## reasoning ## ]]\n{reasoning}\n\n[[ ## answer ## ]]\n{answer}\n\n[[ ## completed ## ]]\nIn adhering to this structure, your objective is: \n        Solve the math problem carefully."
        );
        assert_eq!(
            request.user,
            "[[ ## input ## ]]\nWhat is 19 + 23?\n\nRespond with the corresponding output fields, starting with the field `[[ ## reasoning ## ]]`, then `[[ ## answer ## ]]`, and then ending with the marker for `[[ ## completed ## ]]`."
        );
    }

    #[test]
    fn aime_solver_request_dedents_multiline_instructions_like_dspy_chat_adapter() {
        let request = render_dspy_aime_chain_of_thought_request(
            "    First line.\n      Preserve relative indent.",
            "What is 19 + 23?",
        );

        assert!(request.system.ends_with(
            "In adhering to this structure, your objective is: \n        First line.\n          Preserve relative indent."
        ));
    }

    #[test]
    fn aime_solver_json_fallback_request_matches_dspy_json_adapter() {
        let request = render_dspy_aime_json_adapter_request(
            "Solve the math problem carefully.",
            "What is 19 + 23?",
        );

        assert_eq!(
            request.system,
            "Your input fields are:\n1. `input` (str): The math problem to solve.\nYour output fields are:\n1. `reasoning` (str): \n2. `answer` (str): The final numerical answer.\nAll interactions will be structured in the following way, with the appropriate values filled in.\n\nInputs will have the following structure:\n\n[[ ## input ## ]]\n{input}\n\nOutputs will be a JSON object with the following fields.\n\n{\n  \"reasoning\": \"{reasoning}\",\n  \"answer\": \"{answer}\"\n}\nIn adhering to this structure, your objective is: \n        Solve the math problem carefully."
        );
        assert_eq!(
            request.user,
            "[[ ## input ## ]]\nWhat is 19 + 23?\n\nRespond with a JSON object in the following order of fields: `reasoning`, then `answer`."
        );
    }

    #[test]
    fn aime_solver_parser_preserves_reasoning_and_scores_only_answer_field() {
        let parsed = parse_dspy_aime_chain_of_thought_response(
            "[[ ## reasoning ## ]]\n19 + 23 = 42.\n\n[[ ## answer ## ]]\n42\n\n[[ ## completed ## ]]",
        )
        .expect("DSPy chat adapter output parses");

        assert_eq!(parsed.reasoning, "19 + 23 = 42.");
        assert_eq!(parsed.answer, "42");
        assert!(parsed.raw.trim_start().starts_with("[[ ## reasoning ## ]]"));
    }

    #[test]
    fn aime_solver_parser_accepts_dspy_same_line_field_content() {
        let parsed = parse_dspy_aime_chain_of_thought_response(
            "[[ ## reasoning ## ]] 19 + 23 = 42.\n[[ ## answer ## ]] 42\n[[ ## completed ## ]]",
        )
        .expect("DSPy chat adapter accepts same-line field content");

        assert_eq!(parsed.reasoning, "19 + 23 = 42.");
        assert_eq!(parsed.answer, "42");
    }

    #[test]
    fn aime_solver_parser_requires_all_dspy_output_fields() {
        let error = parse_dspy_aime_chain_of_thought_response(
            "[[ ## answer ## ]]\n42\n\n[[ ## completed ## ]]",
        )
        .expect_err("DSPy chat adapter requires the ChainOfThought reasoning field");

        assert_eq!(error, "missing DSPy `reasoning` field");
    }

    #[test]
    fn aime_solver_falls_back_to_json_adapter_after_chat_parse_failure() {
        #[derive(Clone, Debug)]
        struct ChatThenJsonLm {
            requests: Arc<Mutex<Vec<LmRequest>>>,
        }

        impl Lm for ChatThenJsonLm {
            fn id(&self) -> LmId {
                LmId::from("chat-then-json")
            }

            fn fingerprint(&self) -> Fingerprint {
                Fingerprint::from_bytes([19; 32])
            }

            async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
                let response = if request.output == OutputMode::Text {
                    "[[ ## answer ## ]]\n42\n\n[[ ## completed ## ]]"
                } else {
                    "{\"reasoning\":\"19 + 23 = 42.\",\"answer\":\"42\"}"
                };
                self.requests
                    .lock()
                    .expect("request log lock")
                    .push(request);
                let response = LmResponse::new(
                    Message::assistant(response),
                    TokenUsage {
                        input_tokens: 1,
                        cached_input_tokens: 0,
                        output_tokens: 1,
                        reasoning_tokens: 0,
                    },
                )
                .expect("assistant response is valid");
                Ok(Metered::new(response, Cost::llm_calls(1)))
            }
        }

        let requests = Arc::new(Mutex::new(Vec::new()));
        let lm = AimeInstrumentedLm::new(
            ChatThenJsonLm {
                requests: Arc::clone(&requests),
            },
            AimeLmTelemetry::new(LmCachePolicy::Never),
        );
        let config = AimeRunConfig::gepa_aime();

        let (output, cost) = block_on(complete_openai_solver_output(
            &lm,
            "Solve the math problem carefully.",
            "What is 19 + 23?",
            &config.solver,
        ))
        .expect("JSONAdapter fallback recovers the output");

        assert_eq!(output.reasoning, "19 + 23 = 42.");
        assert_eq!(output.answer, "42");
        assert_eq!(cost.llm_calls, 2);
        let requests = requests.lock().expect("request log lock");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].output, OutputMode::Text);
        assert!(matches!(requests[1].output, OutputMode::JsonSchema(_)));
        assert!(
            requests[1].messages.iter().any(|message| {
                message
                    .content()
                    .contains("Respond with a JSON object in the following order of fields")
            }),
            "fallback request should use DSPy JSONAdapter user instructions"
        );
    }

    #[test]
    fn aime_side_info_renders_upstream_optimize_anything_keys() {
        let rendered = aime_reflection_side_info_example(AimeReflectionSideInfo {
            score: 0.0,
            input: "What is 19 + 23?".to_owned(),
            prompt: "Solve carefully.".to_owned(),
            output: "44".to_owned(),
            reasoning: "I added incorrectly.".to_owned(),
            execution_feedback: "Your answer is incorrect. The correct answer is '42'.".to_owned(),
        });

        assert_eq!(
            rendered,
            vec![
                ("score".to_owned(), "0.0".into()),
                ("input".to_owned(), "What is 19 + 23?".into()),
                ("prompt".to_owned(), "Solve carefully.".into()),
                ("output".to_owned(), "44".into()),
                ("reasoning".to_owned(), "I added incorrectly.".into()),
                (
                    "execution_feedback".to_owned(),
                    "Your answer is incorrect. The correct answer is '42'.".into(),
                ),
            ]
        );
    }

    #[test]
    fn aime_full_reflection_prompt_renders_upstream_optimize_anything_markdown() {
        let config = AimeRunConfig::gepa_aime();
        let reflector = aime_reflector_config(&config.reflection);
        let artifact = AimePrompt::new("Solve carefully.");
        let surface = AimePromptSurface;
        let request = ReflectRequest::for_part(CandidateId::new(), "system", "system")
            .with_examples([{
                let mut case = ReflectiveCase::from_example(
                    ReflectiveValue::default(),
                    None,
                    None,
                    None,
                    String::new(),
                );
                case.runs[0].side_info =
                    aime_reflection_side_info_example(AimeReflectionSideInfo {
                        score: 0.0,
                        input: "What is 19 + 23?".to_owned(),
                        prompt: "Solve carefully.".to_owned(),
                        output: "44".to_owned(),
                        reasoning: "I added incorrectly.".to_owned(),
                        execution_feedback: "Your answer is incorrect. The correct answer is '42'."
                            .to_owned(),
                    });
                case
            }]);

        let lm_request = DefaultReflectionRenderer
            .render(ReflectionRenderInput::<
                RunProblem<AimePrompt, AimeInput, AimeTarget>,
                AimePromptSurface,
            > {
                request: &request,
                artifact: &artifact,
                surface: &surface,
                model: config.reflection.model.as_str().into(),
                config: &reflector,
            })
            .expect("AIME reflection prompt renders");
        let rendered = lm_request
            .messages
            .iter()
            .map(Message::content)
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(
            rendered,
            r"I am optimizing a parameter in my system. The current parameter value is:
```
Solve carefully.
```

Below is evaluation data showing how this parameter value performed across multiple test cases. The data contains performance metrics, diagnostic information, and other relevant details from the evaluation:
```
# Example 1
## score
0.0

## input
What is 19 + 23?

## prompt
Solve carefully.

## output
44

## reasoning
I added incorrectly.

## execution_feedback
Your answer is incorrect. The correct answer is '42'.


```

Your task is to propose a new, improved parameter value that can be used as a drop-in replacement for the current one.

Carefully analyze all the evaluation data provided above. Look for patterns that indicate what works and what doesn't. Pay special attention to:
- Performance metrics and how they correlate with parameter behavior
- Recurring issues, errors, or failure patterns across multiple test cases
- Successful patterns or behaviors that should be preserved or enhanced
- Any domain-specific requirements, constraints, or factual information revealed in the evaluation data
- Specific technical details that are crucial for understanding the parameter's role

Based on your analysis, propose a new parameter value that addresses the identified issues while maintaining or improving upon what works well. Your proposal should be directly informed by the patterns and insights from the evaluation data.

Provide the new parameter value within ``` blocks."
        );
    }

    #[test]
    fn aime_scorer_feedback_matches_upstream_gepa_aime_wording() {
        let run = block_on(run_deterministic_aime());
        let feedback = run
            .optimized
            .summary
            .evaluation
            .splits_reported
            .iter()
            .flat_map(|split| &split.candidates)
            .flat_map(|candidate| &candidate.cases)
            .map(|case| case.feedback.as_str())
            .collect::<Vec<_>>();

        assert!(
            feedback.iter().any(|line| line
                == &"Your answer is incorrect. The correct answer is '2'. Here's the full step-by-step solution:\n2^3 = 8 == 1 mod 7, so 2^10 == 2 mod 7.\n\nThink about what takeaways you can learn from this solution to improve your future answers and approach to similar problems"),
            "incorrect feedback should match upstream GEPA AIME wording"
        );
        assert!(
            feedback.iter().any(|line| line
                == &"Your answer is correct. The correct answer is '42'. Here's the full step-by-step solution:\n19 + 23 = 42.\n\nThink about what takeaways you can learn from this solution to improve your future answers and approach to similar problems"),
            "correct feedback should match upstream GEPA AIME wording"
        );
        assert!(
            !feedback.iter().any(|line| line.contains("incorrect; got")),
            "old Leaven scorer wording must not enter reflection side-info"
        );
    }

    #[test]
    fn aime_scorer_parse_failure_feedback_matches_upstream_gepa_aime_wording() {
        let target = AimeTarget {
            answer: AimeAnswer {
                integer: 42,
                raw: "42".to_owned(),
            },
            solution: "19 + 23 = 42.".to_owned(),
        };

        let (score, feedback) = aime_score_feedback(&target, "forty-two");

        assert!(score.abs() < f64::EPSILON);
        assert_eq!(
            feedback,
            "The final answer must be a valid integer and nothing else. You responded with 'forty-two', which couldn't be parsed as a python integer. Please ensure your answer is a valid integer without any additional text or formatting. The correct answer is '42'. Here's the full step-by-step solution:\n19 + 23 = 42.\n\nThink about what takeaways you can learn from this solution to improve your future answers and approach to similar problems and ensure your final answer is a valid integer."
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
        assert!(lines.iter().any(|line| line == "gepa_profile=reference"));
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
            line.starts_with("compatibility=schema=leaven-run.compatibility.v4")
                && line.contains(" run_kind=leaven-run.optimize ")
                && line.contains(" cache=cache:auto/")
                && line.contains(" lm_roles=2")
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
                .any(|line| line == "search_metric_calls_overshoot=0")
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
        assert!(lines.iter().any(|line| line == "gepa_report=available"));
        assert!(lines.iter().any(|line| line == "gepa_best_index=1"));
        assert!(
            lines
                .iter()
                .any(|line| line == "gepa_validation_best_index=1")
        );
        assert!(lines.iter().any(|line| line == "gepa_candidate_count=2"));
        assert!(
            lines
                .iter()
                .any(|line| line == "gepa_proposal_attempt_count=1")
        );
        assert!(lines.iter().any(|line| line == "gepa_accepted_count=1"));
        assert!(
            lines
                .iter()
                .any(|line| line == "gepa_accepted_unadmitted_count=0")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "gepa_full_validation_evals=2")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "gepa_search_metric_calls=8")
        );
        assert!(lines.iter().any(|line| {
            line == "lm_role_cost=reflection calls=1 prompt_tokens=37 cached_input_tokens=0 completion_tokens=11 reasoning_tokens=0 cost_llm_calls=1 cost_prompt_tokens=37 cost_completion_tokens=11"
        }));
        assert!(lines.iter().any(|line| {
            line == "lm_role_cache=reflection hits=0 misses=0 bypasses=1 bypass_policy_never=1 bypass_refresh=0 required_misses=0 read_errors=0 write_errors=0 other_errors=0 hit_cost_zero=true"
        }));
        assert_report_case_line(&lines, validation_id, "deterministic:default:validation:0");
        assert_report_case_line(&lines, test_id, "deterministic:default:test:0");
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("step-by-step solution"))
        );
    }

    fn assert_report_case_line(lines: &[String], case: leaven::kernel::CaseId, source_id: &str) {
        assert!(lines.iter().any(|line| {
            line.contains(&format!("report_case={case}"))
                && line.contains(&format!("source_id={source_id}"))
                && line.contains("score_state=present")
                && line.contains("output_ref=")
                && line.contains("feedback_ref=")
                && line.contains("trace_refs=")
                && line.contains("output_chars=")
                && line.contains("feedback_chars=")
        }));
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
    fn deterministic_run_dir_resume_restores_gepa_without_repeating_search_work() {
        let run_dir = std::env::temp_dir().join(format!(
            "leaven-p8-resume-{}-{}",
            std::process::id(),
            RunId::new()
        ));
        let mut config = AimeRunConfig::deterministic_smoke();
        config.budget = Budget::metric_calls(8);
        config.max_iterations = 2;
        config.evaluation_cache_policy = CachePolicy::Deterministic;
        config.run_dir = Some(run_dir.clone());

        let first = block_on(run_aime(config.clone(), deterministic_dataset()));
        assert_eq!(
            first.optimized.stop,
            leaven::run::OptimizationStopReason::BudgetReached
        );
        let first_report = first
            .gepa_report
            .as_ref()
            .expect("first run has GEPA report");
        assert_eq!(first_report.total_metric_calls, 8);
        assert_eq!(first_report.full_validation_evals, 2);
        assert_eq!(first_report.proposal_attempts.len(), 1);
        assert_eq!(first.role_reports.reflection.metrics.calls, 1);

        let resumed = block_on(run_aime(config.clone(), deterministic_dataset()));
        let resumed_report = resumed
            .gepa_report
            .as_ref()
            .expect("resumed run has GEPA report");
        assert_eq!(resumed.optimized.run_id, first.optimized.run_id);
        assert_eq!(
            resumed.optimized.stop,
            leaven::run::OptimizationStopReason::BudgetReached
        );
        assert_eq!(
            resumed_report.total_metric_calls,
            first_report.total_metric_calls
        );
        assert_eq!(
            resumed_report.full_validation_evals,
            first_report.full_validation_evals
        );
        assert_eq!(
            resumed_report.proposal_attempts.len(),
            first_report.proposal_attempts.len()
        );
        assert_eq!(
            gepa_frontier_signature(resumed_report),
            gepa_frontier_signature(first_report)
        );
        assert_eq!(
            resumed.role_reports.reflection.metrics.calls, 0,
            "restored GEPA must not call reflection again after budget-stopped checkpoint"
        );
        let resumed_report_json = p8_aime_report_json(&config, &resumed);
        assert_eq!(
            resumed_report_json["lm_roles"][1]["observed_requests_scope"],
            "process_local"
        );
        assert_eq!(
            resumed_report_json["lm_roles"][1]["observed_request_count"],
            0
        );
        assert_eq!(
            resumed_report_json["gepa_report"]["candidates"][0]["system_prompt_source"],
            "seed_config"
        );
        assert_eq!(
            resumed_report_json["gepa_report"]["candidates"][1]["system_prompt_source"],
            "unavailable_process_local_lm_telemetry"
        );
        assert_eq!(
            resumed_report_json["gepa_report"]["candidates"][1]["system_prompt"],
            serde_json::Value::Null
        );
        assert!(resumed.optimized.summary.storage.is_resumable());
        assert!(resumed.optimized.summary.cache.evaluation.durable);
        assert!(
            resumed.optimized.summary.cache.evaluation.hits > 0,
            "resumed final reporting should hit search-cache rows restored from the run dir"
        );
        assert!(
            resumed.optimized.summary.cache.evaluation.hit_cost_zero,
            "evaluation cache hits must report zero run cost"
        );

        std::fs::remove_dir_all(run_dir).unwrap();
    }

    #[test]
    fn metric_call_overshoot_reports_spent_minus_cap_without_clamping_spent() {
        assert_eq!(metric_calls_overshoot(Some(500), 498), 0);
        assert_eq!(metric_calls_overshoot(Some(500), 512), 12);
        assert_eq!(metric_calls_overshoot(None, 512), 0);
    }

    #[test]
    fn deterministic_metric_budget_overshoot_finishes_started_validation() {
        let mut config = AimeRunConfig::deterministic_smoke();
        config.budget = Budget::metric_calls(7);
        config.max_iterations = 2;
        let run = block_on(run_aime(config.clone(), deterministic_dataset()));
        let result = &run.optimized;

        assert_eq!(
            result.stop,
            leaven::run::OptimizationStopReason::BudgetReached
        );
        assert_eq!(
            result.summary.optimization_cost.metric_calls, 8,
            "GEPA checks max_metric_calls before the next optimizer step, so the accepted child's started validation finishes"
        );
        assert!(
            result
                .events
                .contains(&RunEventSummary::OptimizationStopping)
        );
        assert!(
            !result.events.contains(&RunEventSummary::Error),
            "metric-call overshoot should be a clean stop, not a hard budget refusal"
        );
        let lines = report_lines(&config, &run);
        assert!(
            lines
                .iter()
                .any(|line| line == "search_metric_calls_spent=8")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "search_metric_calls_overshoot=1")
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
        assert_eq!(
            parse_lm_cache_policy(LEAVEN_AIME_SOLVER_CACHE_POLICY, Some("cache-only")),
            LmCachePolicy::CacheOnly
        );
    }

    #[test]
    fn gepa_profile_parser_accepts_optimize_anything_reference_and_fast_certified() {
        assert_eq!(
            parse_gepa_profile(LEAVEN_AIME_GEPA_PROFILE, None),
            GepaProfile::OptimizeAnything
        );
        assert_eq!(
            parse_gepa_profile(LEAVEN_AIME_GEPA_PROFILE, Some("optimize-anything")),
            GepaProfile::OptimizeAnything
        );
        assert_eq!(
            parse_gepa_profile(LEAVEN_AIME_GEPA_PROFILE, Some("reference")),
            GepaProfile::Reference
        );
        assert_eq!(
            parse_gepa_profile(LEAVEN_AIME_GEPA_PROFILE, Some("fast-certified")),
            GepaProfile::FastCertified
        );
        assert_eq!(
            parse_gepa_profile(LEAVEN_AIME_GEPA_PROFILE, Some("fast_certified")),
            GepaProfile::FastCertified
        );
    }

    #[test]
    fn eager_sqlite_cache_reports_exact_read_before_workspace_write() {
        let run_dir = std::env::temp_dir()
            .join("leaven-p8-aime")
            .join(RunId::new().to_string());
        let storage = RunStorage::Stored {
            run_id: RunId::new(),
            run_dir: Some(run_dir.clone()),
            latest_checkpoint: None,
            resumability: RunResumability::Resumable,
        };
        let run_cache = SqliteLmCache::path_in_run_dir(&run_dir)
            .display()
            .to_string();
        let workspace_cache = SqliteLmCache::path_in_workspace(".").display().to_string();

        assert_eq!(
            report_lm_cache_path(AimeLmCacheBackend::EagerSqlite, &storage),
            workspace_cache
        );
        assert_eq!(
            report_lm_cache_read_paths(AimeLmCacheBackend::EagerSqlite, &storage),
            vec![run_cache.clone(), workspace_cache.clone()]
        );
        assert_eq!(
            report_lm_cache_read_paths_line(AimeLmCacheBackend::EagerSqlite, &storage),
            format!("{run_cache};{workspace_cache}")
        );
        assert_eq!(
            report_lm_cache_write_path(AimeLmCacheBackend::EagerSqlite, &storage),
            workspace_cache
        );
    }

    #[test]
    fn cache_only_live_replay_classification_does_not_claim_provider_proof() {
        let mut config = AimeRunConfig::gepa_aime();
        config.solver.cache_policy = LmCachePolicy::CacheOnly;
        config.reflection.cache_policy = LmCachePolicy::CacheOnly;
        let reports = AimeRoleReports::from_config(
            &config,
            AimeRoleRuntimeFingerprints::from_config(&config),
            AimeLmRoleMetrics::default(),
            AimeLmRoleMetrics::default(),
        );

        assert_eq!(
            proof_classification_for_report(&config, &reports),
            "cache_only_aime_replay_not_live_proof"
        );

        let mut run = block_on(run_aime(
            AimeRunConfig::deterministic_smoke(),
            deterministic_dataset(),
        ));
        run.role_reports = reports;
        assert!(
            report_lines(&config, &run)
                .iter()
                .any(|line| line == "proof_classification=cache_only_aime_replay_not_live_proof")
        );
        assert_eq!(
            p8_aime_report_json(&config, &run)["proof_classification"],
            "cache_only_aime_replay_not_live_proof"
        );
    }

    #[test]
    fn live_solver_failures_fail_closed_instead_of_empty_answer_scoring() {
        let mut config = AimeRunConfig::deterministic_smoke();
        config.solver.live = true;
        config.solver.cache_policy = LmCachePolicy::CacheOnly;
        config.solver.runtime.cache_backend = AimeLmCacheBackend::InMemory;
        config.reflection.live = false;
        config.reflection.cache_policy = LmCachePolicy::Never;
        config.evaluation_parallelism = NonZeroUsize::new(1).expect("one is non-zero");

        let error = block_on(try_run_aime(config.clone(), deterministic_dataset())).unwrap_err();
        let rendered = std::iter::successors(Some(&error as &dyn std::error::Error), |source| {
            source.source()
        })
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
        assert!(
            rendered.contains("runner function failed")
                || rendered.contains("AIME solver LM failed"),
            "{rendered}"
        );
        assert!(
            error_report_lines(&config, &error, Duration::ZERO)
                .iter()
                .any(|line| line.starts_with("p8_aime_gepa_failure_source_")),
            "CLI failure output should expose the safe source chain"
        );
        assert!(
            error_report_lines(&config, &error, Duration::from_millis(7))
                .iter()
                .any(|line| line == "p8_aime_gepa_failed_wall_time_ms=7"),
            "CLI failure output should expose wall time for cache-only/debug failures"
        );
        let lines = error_report_lines(&config, &error, Duration::from_millis(7));
        assert!(
            lines
                .iter()
                .any(|line| line == "proof_classification=cache_only_aime_replay_not_live_proof")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "solver_runtime=live=true model=deterministic-aime-solver cache_policy=cache-only cache_backend=in-memory cache_durable=false max_concurrent_requests=32 request_timeout_seconds=120")
        );
        let report = p8_aime_failure_report_json(&config, &error, Duration::from_millis(7));
        assert_eq!(report["schema"], "leaven.p8_aime.failure_report.v1");
        assert_eq!(report["run_profile"], config.profile.label());
        assert_eq!(report["gepa_profile"], config.gepa_profile.label());
        assert_eq!(
            report["proof_classification"],
            proof_classification_for_config(&config)
        );
        assert_eq!(report["wall_time_ms"], 7);
        assert!(
            report["error"]
                .as_str()
                .unwrap()
                .contains("optimizer failed")
        );
        assert_eq!(
            report["search_metric_call_cap"],
            serde_json::json!(config.budget.metric_calls)
        );
        assert_eq!(report["final_report_metric_call_cap"], "unlimited");
        assert_eq!(report["cache"]["lm_backend"], "in-memory");
        assert_eq!(
            report["cache"]["lm_read_paths"].as_array().unwrap().len(),
            0
        );
        assert_eq!(report["lm_roles"].as_array().unwrap().len(), 2);
        assert_eq!(report["lm_roles"][0]["role"], "solver");
        assert_eq!(
            report["lm_roles"][0]["runtime"]["cache_policy"],
            "cache-only"
        );
        assert_eq!(report["live_provider_proof"]["role_count"], 2);
        assert_eq!(report["live_provider_proof"]["all_roles_live"], false);
        assert_eq!(report["provider_failures"]["durable"]["count"], 0);
    }

    #[test]
    fn live_openai_runtime_config_defaults_to_sqlite_cache_and_names_provider_throttle() {
        let runtime = AimeOpenAiRuntimeConfig::from_values(Some("8"), None, Some("600"));

        assert_eq!(runtime.max_concurrent_requests.get(), 8);
        assert_eq!(runtime.cache_backend, AimeLmCacheBackend::Sqlite);
        assert_eq!(runtime.request_timeout_seconds, 600);
        assert!(runtime.cache_backend.is_durable());
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, None, None)
                .max_concurrent_requests
                .get(),
            GEPA_AIME_MAX_WORKERS
        );
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, None, None).request_timeout_seconds,
            GEPA_AIME_OPENAI_REQUEST_TIMEOUT_SECONDS
        );
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, Some("in-memory"), None).cache_backend,
            AimeLmCacheBackend::InMemory
        );
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, Some("eager"), None).cache_backend,
            AimeLmCacheBackend::EagerSqlite
        );
        assert_eq!(
            AimeOpenAiRuntimeConfig::from_values(None, Some("workspace-sqlite"), None)
                .cache_backend,
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
        config.solver.runtime =
            AimeOpenAiRuntimeConfig::from_values(Some("7"), Some("sqlite"), Some("600"));
        config.reflection.live = true;
        config.reflection.model = "reflection-model".to_owned();
        config.reflection.cache_policy = LmCachePolicy::Refresh;
        config.reflection.runtime = config.solver.runtime;
        let mut result = block_on(run_deterministic_aime());
        result.role_reports = AimeRoleReports::from_config(
            &config,
            AimeRoleRuntimeFingerprints::from_config(&config),
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
                .any(|line| { line == "comparison_reflection_model_alignment=model-delta" })
        );
        assert!(
            lines
                .iter()
                .any(|line| { line == "comparison_published_validation_score=0.578" })
        );
        assert!(
            lines.iter().any(|line| {
                line == "comparison_upstream_configured_search_metric_call_cap=500"
            })
        );
        assert!(
            lines
                .iter()
                .any(|line| { line == "comparison_upstream_checkpoint_metric_calls=621" })
        );
        assert!(
            lines
                .iter()
                .any(|line| { line == "comparison_upstream_checkpoint_candidate_count=10" })
        );
        assert!(
            lines
                .iter()
                .any(|line| { line == "comparison_upstream_run_log_available=false" })
        );
        assert!(lines.iter().any(|line| {
            line == "comparison_note=upstream_source_uses_serial_proposals_but_local_checkpoint_has_10_candidates_621_metric_calls_and_missing_run_log"
        }));
        assert!(lines.iter().any(|line| {
            line == "comparison_note=leaven_reflection_model_differs_from_upstream_aime_profile"
        }));
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
            lines.iter().any(|line| line.ends_with("/lm-cache.sqlite")
                && line.starts_with("lm_cache_read_paths="))
        );
        assert!(
            lines.iter().any(|line| line.ends_with("/lm-cache.sqlite")
                && line.starts_with("lm_cache_write_path="))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("optimizer_wall_time_ms="))
        );
        assert!(
            lines
                .iter()
                .any(|line| line == &format!("seed_system_prompt={BASELINE}"))
        );
        assert!(lines.iter().any(|line| line == "aime_train_count=3"));
        assert!(lines.iter().any(|line| line == "aime_validation_count=1"));
        assert!(lines.iter().any(|line| line == "aime_test_count=2"));
        assert!(lines.iter().any(|line| line == "aime_split_seed=none"));
        assert!(lines.iter().any(|line| line == "aime_test_repeated=false"));
        assert!(lines.iter().any(|line| line == "aime_cache_hash=none"));
        assert!(
            lines
                .iter()
                .any(|line| line == "openai_max_concurrent_requests=7")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "openai_request_timeout_seconds=600")
        );
        assert!(lines.iter().any(|line| line == "reflection_output=text"));
        assert!(
            lines
                .iter()
                .any(|line| line == "reflection_parser=plain-text-fenced")
        );
        assert!(lines.iter().any(|line| {
            line.starts_with(
                "lm_role_prompt_contract=solver renderer=dspy-chat-adapter-chain-of-thought-with-json-fallback upstream=dspy.ChatAdapter->JSONAdapter request_shape_fingerprint=",
            )
        }));
        assert!(lines.iter().any(|line| {
            line.starts_with(
                "lm_role_prompt_contract=reflection renderer=gepa-default-markdown-side-info upstream=gepa.optimize_anything request_shape_fingerprint=",
            )
        }));
        assert!(lines.iter().any(|line| {
            line.starts_with(
                "lm_role=solver provider=openai live=true model=solver-model runtime_fingerprint=",
            )
        }));
        assert!(lines.iter().any(|line| {
            line == "lm_role_runtime=solver cache_policy=read-write cache_backend=sqlite cache_durable=true max_concurrent_requests=7 request_timeout_seconds=600 output=dspy-chain-of-thought-with-json-fallback parser=dspy-chat-adapter-fields-or-json-adapter"
        }));
        assert!(lines.iter().any(|line| {
            line.starts_with(
                "lm_role=reflection provider=openai live=true model=reflection-model runtime_fingerprint=",
            )
        }));
        assert!(lines.iter().any(|line| {
            line == "lm_role_runtime=reflection cache_policy=refresh cache_backend=sqlite cache_durable=true max_concurrent_requests=7 request_timeout_seconds=600 output=text parser=plain-text-fenced"
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
    fn p8_aime_json_report_persists_role_and_proof_facts() {
        let config = AimeRunConfig::deterministic_smoke();
        let run = block_on(run_aime(config.clone(), deterministic_dataset()));
        let path = p8_aime_report_path(&run.optimized.summary.storage)
            .expect("stored deterministic P8 run has a P8 report path");
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("P8 AIME report JSON is written"))
                .expect("P8 AIME report JSON parses");

        assert_p8_report_identity(&report, &config);
        assert_p8_report_lm_roles(&report);
        assert_p8_report_gepa_summary(&report);
        assert_p8_report_run_summary_equivalence(&report, &config, &run);
        assert_p8_report_case_safety(&report);
        let lines = report_lines(&config, &run);
        assert!(
            lines
                .iter()
                .any(|line| line == &format!("p8_aime_json={}", path.display()))
        );
    }

    #[test]
    fn p8_comparison_model_alignment_tracks_upstream_reflection_override() {
        let mut config = AimeRunConfig::deterministic_smoke();
        config.profile = AimeRunProfile::GepaAime;
        config.reflection.model = "gpt-5.1".to_owned();
        let run = block_on(run_deterministic_aime());

        let report = p8_aime_report_json(&config, &run);
        let lines = report_lines(&config, &run);

        assert_eq!(
            report["comparison"]["upstream_reflection_model"],
            UPSTREAM_GEPA_AIME_REFLECTION_MODEL
        );
        assert_eq!(report["comparison"]["leaven_reflection_model"], "gpt-5.1");
        assert_eq!(
            report["comparison"]["reflection_model_alignment"],
            "upstream-matched"
        );
        assert_eq!(
            report["comparison"]["published_validation_score"],
            GEPA_CAIS_AIME_PUBLISHED_VALIDATION_SCORE
        );
        assert_eq!(
            report["comparison"]["upstream_configured_search_metric_call_cap"],
            GEPA_CAIS_AIME_CONFIGURED_SEARCH_CAP
        );
        assert_eq!(
            report["comparison"]["upstream_checkpoint_metric_calls"],
            GEPA_CAIS_AIME_CHECKPOINT_METRIC_CALLS
        );
        assert_eq!(
            report["comparison"]["upstream_checkpoint_candidate_count"],
            GEPA_CAIS_AIME_CHECKPOINT_CANDIDATES
        );
        assert_eq!(report["comparison"]["upstream_run_log_available"], false);
        assert!(
            report["comparison"]["notes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|note| note == "leaven_reflection_model_matches_upstream_aime_profile")
        );
        assert!(
            report["comparison"]["notes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|note| {
                    note == "upstream_source_uses_serial_proposals_but_local_checkpoint_has_10_candidates_621_metric_calls_and_missing_run_log"
                })
        );
        assert!(
            lines
                .iter()
                .any(|line| { line == "comparison_reflection_model_alignment=upstream-matched" })
        );
        assert!(lines.iter().any(|line| {
            line == "comparison_note=leaven_reflection_model_matches_upstream_aime_profile"
        }));
    }

    #[test]
    fn p8_aime_json_uses_checkpointed_gepa_report_events_after_resume() {
        let config = AimeRunConfig::deterministic_smoke();
        let mut run = block_on(run_aime(config.clone(), deterministic_dataset()));
        let checkpointed_events = run
            .gepa_report
            .as_ref()
            .expect("deterministic run emits a GEPA report")
            .events
            .len();
        run.gepa_events.clear();

        let report = p8_aime_report_json(&config, &run);

        assert_eq!(
            report["gepa_events"].as_array().unwrap().len(),
            checkpointed_events
        );
        assert_eq!(report["gepa_events"], report["gepa_report"]["events"]);
        assert!(
            report["gepa_events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["phase"] == "seed_validation_started")
        );
    }

    fn assert_p8_report_identity(report: &serde_json::Value, config: &AimeRunConfig) {
        assert_eq!(report["schema"], "leaven.p8_aime.report.v1");
        assert_eq!(report["run_profile"], config.profile.label());
        assert_eq!(report["gepa_profile"], config.gepa_profile.label());
        assert_eq!(
            report["proof_classification"],
            config.proof_classification()
        );
        assert_eq!(
            report["comparison_target"],
            config.profile.comparison_target()
        );
        assert_eq!(
            report["comparison"]["target"],
            config.profile.comparison_target()
        );
        assert_eq!(
            report["comparison_reflection_prompt"],
            config.profile.reflection_prompt_claim()
        );
        assert_eq!(
            report["comparison"]["upstream_reflection_model"],
            config.profile.upstream_reflection_model()
        );
        assert_eq!(
            report["comparison"]["leaven_reflection_model"],
            config.reflection.model
        );
        assert_eq!(
            report["comparison"]["reflection_model_alignment"],
            config
                .profile
                .reflection_model_alignment(&config.reflection.model)
        );
        assert_eq!(report["dataset"]["train_count"], 3);
        assert_eq!(report["dataset"]["validation_count"], 1);
        assert_eq!(report["dataset"]["test_count"], 2);
        assert_eq!(report["dataset"]["split_seed"], serde_json::Value::Null);
        assert_eq!(report["dataset"]["test_repeated"], false);
        assert_eq!(
            report["dataset"]["source_splits"][0]["dataset"],
            "deterministic"
        );
        assert!(report["run"]["optimizer_wall_time_ms"].is_number());
        assert_eq!(report["budget"]["search_metric_calls_overshoot"], 0);
    }

    fn assert_p8_report_lm_roles(report: &serde_json::Value) {
        assert_eq!(report["lm_roles"].as_array().unwrap().len(), 2);
        assert_eq!(report["lm_roles"][0]["role"], "solver");
        assert_eq!(report["lm_roles"][1]["role"], "reflection");
        assert_eq!(
            report["lm_roles"][0]["prompt_contract"]["renderer"],
            "dspy-chat-adapter-chain-of-thought-with-json-fallback"
        );
        assert_eq!(
            report["lm_roles"][1]["prompt_contract"]["renderer"],
            "gepa-default-markdown-side-info"
        );
        assert_eq!(
            report["lm_roles"][0]["runtime"]["request_timeout_seconds"],
            GEPA_AIME_OPENAI_REQUEST_TIMEOUT_SECONDS
        );
        assert_eq!(
            report["lm_roles"][1]["runtime"]["request_timeout_seconds"],
            GEPA_AIME_OPENAI_REQUEST_TIMEOUT_SECONDS
        );
        assert!(
            report["lm_roles"][0]["prompt_contract"]["request_shape_fingerprint"]
                .as_str()
                .is_some_and(|value| value.len() == 64)
        );
        assert!(
            report["lm_roles"][1]["prompt_contract"]["request_shape_fingerprint"]
                .as_str()
                .is_some_and(|value| value.len() == 64)
        );
        assert_eq!(
            report["lm_roles"][0]["prompt_contract"]["request_example"]["messages"][0]["role"],
            "system"
        );
        assert!(
            report["lm_roles"][0]["prompt_contract"]["request_example"]["messages"][0]["content"]
                .as_str()
                .is_some_and(|content| content.contains("Your input fields are:"))
        );
        assert_eq!(
            report["lm_roles"][0]["prompt_contract"]["request_example"]["messages"][1]["role"],
            "user"
        );
        assert_eq!(
            report["lm_roles"][1]["prompt_contract"]["request_example"]["messages"][0]["role"],
            "user"
        );
        assert_eq!(
            report["lm_roles"][0]["observed_requests_scope"],
            "process_local"
        );
        assert_eq!(
            report["lm_roles"][1]["observed_requests_scope"],
            "process_local"
        );
        assert_eq!(
            report["lm_roles"][1]["observed_request_count"],
            serde_json::json!(
                report["lm_roles"][1]["observed_requests"]
                    .as_array()
                    .unwrap()
                    .len()
            )
        );
        assert!(
            report["lm_roles"][1]["prompt_contract"]["request_example"]["messages"][0]["content"]
                .as_str()
                .is_some_and(|content| {
                    content.contains("I am optimizing a parameter in my system.")
                        && content.contains("## execution_feedback")
                })
        );
        let observed_reflection_requests = report["lm_roles"][1]["observed_requests"]
            .as_array()
            .expect("reflection observed requests are reported");
        assert!(!observed_reflection_requests.is_empty());
        assert_eq!(
            observed_reflection_requests[0]["messages"][0]["role"],
            "user"
        );
        assert!(
            observed_reflection_requests[0]["messages"][0]["content"]
                .as_str()
                .is_some_and(|content| {
                    content.contains("I am optimizing a parameter in my system.")
                        && content.contains("## execution_feedback")
                })
        );
        assert!(
            observed_reflection_requests[0]["response"]["assistant"]["content"]
                .as_str()
                .is_some_and(|content| {
                    content.starts_with("```\n") && content.contains("modular arithmetic")
                })
        );
        assert_eq!(report["lm_roles"][1]["metrics"]["cost"]["llm_calls"], 1);
        assert_eq!(
            report["lm_roles"][0]["metrics"]["durable_failures"]["count"],
            0
        );
        assert_eq!(report["live_provider_proof"]["role_count"], 2);
        assert_eq!(report["live_provider_proof"]["live_roles"], 0);
        assert_eq!(report["live_provider_proof"]["all_roles_live"], false);
        assert_eq!(
            report["live_provider_proof"]["roles"][0]["request_timeout_seconds"],
            GEPA_AIME_OPENAI_REQUEST_TIMEOUT_SECONDS
        );
        assert_eq!(report["provider_failures"]["count"], 0);
        assert_eq!(report["provider_failures"]["scope"], "process_local");
        assert_eq!(report["provider_failures"]["totals"]["count"], 0);
        assert_eq!(
            report["provider_failures"]["durable"]["scope"],
            "run_dir_jsonl"
        );
        assert_eq!(report["provider_failures"]["durable"]["count"], 0);
        assert_eq!(
            report["provider_failures"]["roles"][0]["failures"]["count"],
            0
        );
        assert_eq!(
            report["provider_failures"]["roles"][1]["failures"]["count"],
            0
        );
    }

    #[test]
    fn p8_provider_failure_report_includes_durable_resume_counts() {
        let path = std::env::temp_dir().join(format!(
            "leaven-p8-provider-failures-{}-{}.jsonl",
            std::process::id(),
            RunId::new()
        ));
        let _ = std::fs::remove_file(&path);
        let telemetry = AimeLmTelemetry::new(LmCachePolicy::Never)
            .with_durable_provider_failures(AimeLmRole::Solver, path.clone());
        telemetry.record(&Err(LmError::invalid_request(
            "OPENAI_API_KEY is required for live OpenAI AIME",
        )));

        let reports = AimeRoleReports::from_config(
            &AimeRunConfig::deterministic_smoke(),
            AimeRoleRuntimeFingerprints::from_config(&AimeRunConfig::deterministic_smoke()),
            AimeLmRoleMetrics::default(),
            AimeLmRoleMetrics::default(),
        )
        .with_durable_failures(AimeDurableProviderFailures::read(&path));
        let report = p8_provider_failures_json(&reports);

        assert_eq!(report["count"], 0);
        assert_eq!(report["totals"]["count"], 0);
        assert_eq!(report["durable"]["count"], 1);
        assert_eq!(report["durable"]["totals"]["missing_credentials"], 1);
        assert_eq!(
            report["durable"]["roles"][0]["failures"]["missing_credentials"],
            1
        );
        assert_eq!(report["durable"]["roles"][1]["failures"]["count"], 0);

        let lines = report_lm_role_lines(&reports.solver);
        assert!(lines.iter().any(|line| {
            line == "lm_role_failures=solver count=0 missing_credentials=0 authentication=0 rate_limit=0 retry_exhausted=0 malformed_provider_response=0 answer_parse=0 scorer_parse=0 budget_refusal=0 cache=0 transport=0 provider=0 unknown=0"
        }));
        assert!(lines.iter().any(|line| {
            line == "lm_role_durable_failures=solver scope=run_dir_jsonl count=1 missing_credentials=1 authentication=0 rate_limit=0 retry_exhausted=0 malformed_provider_response=0 answer_parse=0 scorer_parse=0 budget_refusal=0 cache=0 transport=0 provider=0 unknown=0"
        }));

        let _ = std::fs::remove_file(path);
    }

    fn assert_p8_report_run_summary_equivalence(
        report: &serde_json::Value,
        config: &AimeRunConfig,
        run: &AimeRunResult,
    ) {
        let result = &run.optimized;
        assert_eq!(report["seed"]["system_prompt"], config.seed_prompt);
        assert_eq!(report["run"]["id"], result.run_id.to_string());
        assert_eq!(
            report["run"]["storage"],
            report_run_storage(&result.summary.storage)
        );
        assert_eq!(
            report["run"]["resumable"],
            result.summary.storage.is_resumable()
        );
        assert_eq!(
            report["run"]["resumability"],
            report_resumability(&result.summary.storage)
        );
        assert_eq!(
            report["run"]["run_dir"],
            report_run_dir(&result.summary.storage)
        );
        assert_eq!(
            report["run"]["latest_checkpoint"],
            report_latest_checkpoint(&result.summary.storage)
        );
        assert_eq!(
            report["run"]["summary_json"],
            serde_json::to_value(
                result
                    .summary
                    .reports
                    .summary_json
                    .as_ref()
                    .map(|path| path.display().to_string())
            )
            .unwrap()
        );
        assert_eq!(
            report["best"]["system_prompt"],
            serde_json::to_value(result.best().map(|best| best.system.clone())).unwrap()
        );
        assert_eq!(
            report["cache"]["evaluation"]["backend"],
            result.summary.cache.evaluation.backend.as_str()
        );
        assert_eq!(
            report["cache"]["evaluation"]["durable"],
            result.summary.cache.evaluation.durable
        );
        assert_eq!(
            report["cache"]["evaluation"]["hits"],
            result.summary.cache.evaluation.hits
        );
        assert_eq!(
            report["cache"]["evaluation"]["misses"],
            result.summary.cache.evaluation.misses
        );
        assert_eq!(
            report["cache"]["evaluation"]["write_errors"],
            result.summary.cache.evaluation.write_errors
        );
        assert_eq!(
            report["cache"]["evaluation"]["hit_cost_zero"],
            result.summary.cache.evaluation.hit_cost_zero
        );
        assert_eq!(
            report["cache"]["lm_read_paths"],
            serde_json::to_value(report_lm_cache_read_paths(
                config.solver.runtime.cache_backend,
                &result.summary.storage
            ))
            .unwrap()
        );
        assert_eq!(
            report["cache"]["lm_write_path"],
            report_lm_cache_write_path(
                config.solver.runtime.cache_backend,
                &result.summary.storage
            )
        );
    }

    fn assert_p8_report_gepa_summary(report: &serde_json::Value) {
        let gepa_events = report["gepa_events"].as_array().unwrap();
        assert!(gepa_events.iter().any(|event| {
            event["phase"] == "profile_resolved"
                && event["profile"] == "reference"
                && event["proposal_count"] == 1
                && event["skip_perfect_score"] == true
        }));
        assert!(
            gepa_events
                .iter()
                .any(|event| event["phase"] == "parent_selected")
        );
        assert!(
            gepa_events
                .iter()
                .any(|event| event["phase"] == "reflective_dataset_built")
        );
        assert!(gepa_events.iter().any(|event| {
            event["phase"] == "seed_validation_completed"
                && event["metric_calls_delta"]
                    .as_u64()
                    .is_some_and(|calls| calls > 0)
                && event["score"]
                    .as_str()
                    .is_some_and(|score| !score.is_empty())
        }));
        assert!(gepa_events.iter().any(|event| {
            event["phase"] == "reflection_started"
                && event["records"].as_u64().is_some_and(|records| records > 0)
                && event["source_ref_count"]
                    .as_u64()
                    .is_some_and(|refs| refs > 0)
                && event["cases"]
                    .as_array()
                    .is_some_and(|cases| !cases.is_empty())
        }));
        assert!(gepa_events.iter().any(|event| {
            event["phase"] == "reflection_completed" && event["child"].as_str().is_some()
        }));
        assert!(gepa_events.iter().any(|event| {
            event["phase"] == "parent_evaluated"
                && event["metric_calls_delta"]
                    .as_u64()
                    .is_some_and(|calls| calls > 0)
                && event["score"]
                    .as_str()
                    .is_some_and(|score| !score.is_empty())
        }));
        assert!(gepa_events.iter().any(|event| {
            event["phase"] == "child_evaluated"
                && event["metric_calls_delta"]
                    .as_u64()
                    .is_some_and(|calls| calls > 0)
                && event["score"]
                    .as_str()
                    .is_some_and(|score| !score.is_empty())
        }));
        assert!(
            gepa_events
                .iter()
                .any(|event| event["phase"] == "proposal_accepted")
        );
        assert!(gepa_events.iter().any(|event| {
            event["phase"] == "accepted_validation_completed"
                && event["metric_calls_delta"]
                    .as_u64()
                    .is_some_and(|calls| calls > 0)
                && event["score"]
                    .as_str()
                    .is_some_and(|score| !score.is_empty())
        }));
        assert!(
            gepa_events
                .iter()
                .any(|event| event["phase"] == "candidate_admitted")
        );
        assert!(
            gepa_events
                .iter()
                .any(|event| event["phase"] == "optimization_ended")
        );
        let gepa_report = &report["gepa_report"];
        assert_eq!(gepa_report["profile"]["label"], "reference");
        assert_eq!(gepa_report["profile"]["train_minibatch_size"], 3);
        assert_eq!(gepa_report["profile"]["proposal_count"], 1);
        assert_eq!(gepa_report["profile"]["proposal_mode"], "serial");
        assert_eq!(
            gepa_report["profile"]["validation_policy"],
            "full-validation"
        );
        assert_eq!(
            gepa_report["profile"]["certification_mode"],
            "full-validation-before-admission"
        );
        assert_eq!(gepa_report["profile"]["skip_perfect_score"], true);
        assert_eq!(gepa_report["total_metric_calls"], 8);
        let event_metric_calls: u64 = gepa_events
            .iter()
            .filter_map(|event| event["metric_calls_delta"].as_u64())
            .sum();
        assert_eq!(event_metric_calls, 8);
        assert_eq!(gepa_report["full_validation_evals"], 2);
        assert_eq!(gepa_report["accepted_count"], 1);
        assert_eq!(gepa_report["accepted_unadmitted_count"], 0);
        assert_eq!(gepa_report["quality_summary"]["proposal_attempt_count"], 1);
        assert_eq!(gepa_report["quality_summary"]["screened_count"], 1);
        assert_eq!(
            gepa_report["quality_summary"]["screened_train_improved_count"],
            1
        );
        assert_eq!(gepa_report["quality_summary"]["accepted_count"], 1);
        assert_eq!(gepa_report["quality_summary"]["admitted_count"], 1);
        assert_eq!(
            gepa_report["quality_summary"]["accepted_validation_improved_count"],
            1
        );
        assert_eq!(gepa_report["reflection_summary"]["attempted_count"], 1);
        assert_eq!(
            gepa_report["reflection_summary"]["observed_request_count"],
            1
        );
        assert_eq!(
            gepa_report["reflection_summary"]["observed_response_count"],
            1
        );
        assert_eq!(
            gepa_report["reflection_summary"]["visible_prompt_unique_count"],
            1
        );
        assert_eq!(
            gepa_report["reflection_summary"]["visible_prompt_duplicate_count"],
            0
        );
        assert!(
            gepa_report["reflection_summary"]["request_chars"]["average"]
                .as_f64()
                .is_some_and(|chars| chars > 0.0)
        );
        assert!(
            gepa_report["reflection_summary"]["assistant_chars"]["average"]
                .as_f64()
                .is_some_and(|chars| chars > 0.0)
        );
        assert!(
            gepa_report["reflection_summary"]["proposed_text_chars"]["average"]
                .as_f64()
                .is_some_and(|chars| chars > 0.0)
        );
        assert_eq!(
            gepa_report["reflection_summary"]["accepted_proposed_text_chars"]["count"],
            1
        );
        assert!(
            gepa_report["reflection_summary"]["accepted_proposed_text_chars"]["average"]
                .as_f64()
                .is_some_and(|chars| chars > 0.0)
        );
        assert_eq!(
            gepa_report["reflection_summary"]["rejected_proposed_text_chars"]["count"],
            0
        );
        assert!(
            gepa_report["reflection_summary"]["rejected_proposed_text_chars"]["average"].is_null()
        );
        assert_eq!(gepa_report["skip_perfect_score"], true);
        assert_eq!(gepa_report["perfect_score"], 1.0);
        assert_eq!(gepa_report["best_index"], 1);
        assert!(gepa_report["candidates"].as_array().unwrap().len() >= 2);
        assert_eq!(gepa_report["candidates"][0]["index"], 0);
        assert_eq!(
            gepa_report["candidates"][0]["parents"],
            serde_json::json!([])
        );
        assert_eq!(gepa_report["candidates"][1]["index"], 1);
        assert_eq!(gepa_report["candidates"][0]["system_prompt"], BASELINE);
        assert_eq!(
            gepa_report["candidates"][0]["system_prompt_source"],
            "seed_config"
        );
        assert!(
            gepa_report["candidates"][1]["system_prompt"]
                .as_str()
                .is_some_and(|prompt| prompt.contains("modular arithmetic"))
        );
        assert_eq!(
            gepa_report["candidates"][1]["system_prompt_source"],
            "observed_reflection_response"
        );
        assert_eq!(
            gepa_report["candidates"][1]["parents"],
            serde_json::json!([0])
        );
        assert!(
            gepa_report["candidates"][1]["validation_subscores"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        );
        assert!(
            gepa_report["validation_frontier"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| row["candidates"]
                    .as_array()
                    .is_some_and(|members| members.iter().any(|member| member == 1))))
        );
        let attempts = gepa_report["proposal_attempts"].as_array().unwrap();
        assert!(!attempts.is_empty());
        assert_eq!(attempts[0]["attempt_index"], 1);
        assert_eq!(attempts[0]["reflection_request_index"], 0);
        assert_eq!(attempts[0]["reflection"]["request_index"], 0);
        assert_eq!(
            attempts[0]["reflection"]["model"],
            "deterministic-aime-reflector"
        );
        assert!(
            attempts[0]["reflection"]["assistant_text"]
                .as_str()
                .is_some_and(|text| text.starts_with("```\n"))
        );
        assert!(
            attempts[0]["reflection"]["proposed_text"]
                .as_str()
                .is_some_and(|text| text.contains("modular arithmetic"))
        );
        assert_eq!(attempts[0]["parent_index"], 0);
        assert_eq!(attempts[0]["child_index"], 1);
        assert_eq!(attempts[0]["child_validation_score"], 1.0);
        assert!(
            attempts[0]["parent_cases"]
                .as_array()
                .is_some_and(|cases| !cases.is_empty())
        );
        assert!(
            attempts[0]["child_cases"]
                .as_array()
                .is_some_and(|cases| !cases.is_empty())
        );
        assert_eq!(attempts[0]["accepted"], true);
        assert_eq!(attempts[0]["admitted"], true);
        assert_eq!(attempts[0]["admitted_index"], 1);
    }

    #[test]
    fn report_lines_surface_train_accepted_unadmitted_children() {
        let config = AimeRunConfig::deterministic_smoke();
        let mut run = block_on(run_deterministic_aime());
        let report = run.gepa_report.as_mut().expect("deterministic GEPA report");
        report.proposal_attempts[0].admitted_index = None;

        let lines = report_lines(&config, &run);
        assert!(
            lines
                .iter()
                .any(|line| line == "gepa_accepted_unadmitted_count=1")
        );

        let report = p8_aime_report_json(&config, &run);
        assert_eq!(
            report["gepa_report"]["accepted_unadmitted_count"],
            serde_json::json!(1)
        );
    }

    fn assert_p8_report_case_safety(report: &serde_json::Value) {
        let cases = report["cases"].as_array().unwrap();
        for split in ["train", "validation", "test"] {
            assert!(
                cases
                    .iter()
                    .any(|case| case["split"] == split && case["candidate_role"] == "baseline"),
                "missing baseline {split} case rows"
            );
            assert!(
                cases
                    .iter()
                    .any(|case| case["split"] == split && case["candidate_role"] == "optimized"),
                "missing optimized {split} case rows"
            );
        }
        assert!(report["cases"].as_array().unwrap().iter().all(|case| {
            case.get("source_id").is_some()
                && case.get("candidate_role").is_some()
                && case.get("score_state").is_some()
                && case.get("output_ref").is_some()
                && case.get("feedback_ref").is_some()
                && case
                    .get("trace_refs")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|trace_refs| !trace_refs.is_empty())
                && case.get("feedback_chars").is_some()
                && case.get("target_answer").is_none()
                && case.get("reference_solution").is_none()
        }));
        let case_deltas = report["case_deltas"]["cases"].as_array().unwrap();
        for split in ["train", "validation", "test"] {
            assert!(
                case_deltas
                    .iter()
                    .any(|case| case["split"] == split && case["outcome"] == "improved"),
                "missing improved {split} case delta"
            );
            assert!(
                report["case_deltas"]["summary"][split]["improved"]
                    .as_u64()
                    .is_some_and(|count| count > 0),
                "missing improved {split} summary count"
            );
            assert_eq!(
                report["case_deltas"]["summary"][split]["regressed"],
                serde_json::json!(0)
            );
        }
        assert!(case_deltas.iter().all(|case| {
            case.get("source_id").is_some()
                && case.get("baseline_score").is_some()
                && case.get("optimized_score").is_some()
                && case.get("score_delta").is_some()
                && case.get("target_answer").is_none()
                && case.get("reference_solution").is_none()
        }));
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
    fn solver_parser_failures_increment_answer_parse_telemetry() {
        let telemetry = AimeLmTelemetry::new(LmCachePolicy::Never);
        let lm = AimeInstrumentedLm::new(DeterministicReflectionLm, telemetry.clone());

        let error = parse_openai_solver_response(
            &lm,
            "not a dspy chain-of-thought response",
            &Cost::zero(),
        )
        .expect_err("malformed solver response should fail parser");

        assert!(
            error
                .to_string()
                .contains("AIME solver response did not match DSPy ChainOfThought fields")
        );
        let metrics = telemetry.snapshot();
        assert_eq!(metrics.failures.answer_parse, 1);
        assert_eq!(metrics.failures.total(), 1);
    }

    #[test]
    fn lm_cache_failures_distinguish_required_miss_read_and_write_errors() {
        let cache_only = AimeLmTelemetry::new(LmCachePolicy::CacheOnly);
        cache_only.record(&Err(LmError::cache("required lm cache entry was missing")));
        let metrics = cache_only.snapshot();
        assert_eq!(metrics.failures.cache, 1);
        assert_eq!(metrics.cache.required_misses, 1);
        assert_eq!(metrics.cache.read_errors, 0);
        assert_eq!(metrics.cache.write_errors, 0);

        let read = AimeLmTelemetry::new(LmCachePolicy::ReadWrite);
        read.record(&Err(LmError::cache(
            "lm cache backend failed during get: database is locked",
        )));
        let metrics = read.snapshot();
        assert_eq!(metrics.cache.read_errors, 1);
        assert_eq!(metrics.cache.write_errors, 0);

        let write = AimeLmTelemetry::new(LmCachePolicy::ReadWrite);
        write.record(&Err(LmError::cache(
            "lm cache backend failed during put: readonly database",
        )));
        let metrics = write.snapshot();
        assert_eq!(metrics.cache.read_errors, 0);
        assert_eq!(metrics.cache.write_errors, 1);
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
    fn p8_report_atomic_write_rejects_paths_without_file_names() {
        let error = write_p8_report_atomic(Path::new(""), b"{}", "write P8 AIME report json")
            .expect_err("P8 report atomic writes require a file path");

        assert!(matches!(
            error,
            leaven::run::OptimizeError::ReportStore { .. }
        ));
        assert_eq!(
            error.to_string(),
            "run report failed during write P8 AIME report json"
        );
    }

    #[test]
    fn p8_failure_report_writes_under_configured_run_dir() {
        let run_dir = std::env::temp_dir().join(format!(
            "leaven-p8-failure-report-{}-{}",
            std::process::id(),
            RunId::new()
        ));
        let mut config = AimeRunConfig::gepa_aime();
        config.run_dir = Some(run_dir.clone());
        config.solver.runtime.cache_backend = AimeLmCacheBackend::EagerSqlite;
        config.reflection.model = "gpt-5.1".to_owned();
        config.reflection.cache_policy = LmCachePolicy::CacheOnly;
        config.reflection.runtime.cache_backend = AimeLmCacheBackend::EagerSqlite;
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(
            aime_provider_failures_path(&run_dir),
            r#"{"schema":"leaven.p8_aime.provider_failure.v1","role":"reflection","kind":"cache"}"#,
        )
        .unwrap();
        let error = std::io::Error::other("synthetic failure");

        let path = write_p8_aime_failure_report(&config, &error, Duration::from_millis(9))
            .unwrap()
            .expect("configured run dir writes a failure report");

        assert_eq!(path, run_dir.join("reports").join("p8-aime-failure.json"));
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(report["schema"], "leaven.p8_aime.failure_report.v1");
        assert_eq!(report["error"], "synthetic failure");
        assert_eq!(report["wall_time_ms"], 9);
        assert_eq!(
            report["comparison"]["reflection_model_alignment"],
            "upstream-matched"
        );
        assert_eq!(report["gepa_profile"], "optimize-anything");
        assert_eq!(report["comparison"]["leaven_reflection_model"], "gpt-5.1");
        assert_eq!(report["cache"]["lm_backend"], "eager-sqlite");
        assert_eq!(report["cache"]["lm_path"], ".leaven/lm-cache.sqlite");
        assert_eq!(
            report["cache"]["lm_read_paths"],
            serde_json::json!([
                run_dir.join("lm-cache.sqlite").display().to_string(),
                ".leaven/lm-cache.sqlite"
            ])
        );
        assert_eq!(report["cache"]["lm_write_path"], ".leaven/lm-cache.sqlite");
        assert_eq!(report["lm_roles"].as_array().unwrap().len(), 2);
        assert_eq!(report["lm_roles"][1]["role"], "reflection");
        assert_eq!(report["lm_roles"][1]["model"], "gpt-5.1");
        assert_eq!(
            report["lm_roles"][1]["metrics"]["durable_failures"]["cache"],
            1
        );
        assert_eq!(report["live_provider_proof"]["role_count"], 2);
        assert_eq!(
            report["provider_failures"]["durable"]["scope"],
            "run_dir_jsonl"
        );
        assert_eq!(report["provider_failures"]["durable"]["count"], 1);
        assert_eq!(
            report["provider_failures"]["durable"]["roles"][1]["failures"]["cache"],
            1
        );
        let _ = std::fs::remove_dir_all(run_dir);
    }

    #[test]
    fn p8_failure_report_exposes_resume_compatibility_mismatch() {
        let mut config = AimeRunConfig::deterministic_smoke();
        config.run_dir = Some(unique_temp_dir("p8-compatibility-failure-report"));
        let error = OptimizeError::ResumeCompatibility(Box::new(
            ResumeCompatibilityError::RunnerFingerprintMismatch {
                stored: RuntimeFingerprint::new(Fingerprint::from_bytes([81; 32])),
                live: RuntimeFingerprint::new(Fingerprint::from_bytes([82; 32])),
            },
        ));

        let report = p8_aime_failure_report_json(&config, &error, Duration::from_millis(3));
        let lines = error_report_lines(&config, &error, Duration::from_millis(3));

        assert_eq!(report["resume_compatibility"]["kind"], "runner");
        assert_eq!(
            report["resume_compatibility"]["stored"],
            "5151515151515151515151515151515151515151515151515151515151515151"
        );
        assert_eq!(
            report["resume_compatibility"]["live"],
            "5252525252525252525252525252525252525252525252525252525252525252"
        );
        assert!(lines.iter().any(|line| {
            line == "resume_compatibility_mismatch=runner stored=5151515151515151515151515151515151515151515151515151515151515151 live=5252525252525252525252525252525252525252525252525252525252525252"
        }));
        let _ = std::fs::remove_dir_all(config.run_dir.as_ref().unwrap());
    }

    #[test]
    fn p8_start_report_writes_before_long_provider_work() {
        let run_dir = std::env::temp_dir().join(format!(
            "leaven-p8-start-report-{}-{}",
            std::process::id(),
            RunId::new()
        ));
        let mut config = AimeRunConfig::gepa_aime();
        config.run_dir = Some(run_dir.clone());
        config.solver.model = GEPA_AIME_SOLVER_MODEL.to_owned();
        config.solver.runtime = AimeOpenAiRuntimeConfig::default_for_p8();
        config.reflection.model = "gpt-5.1".to_owned();
        config.reflection.runtime = AimeOpenAiRuntimeConfig::default_for_p8();
        let started_at = UNIX_EPOCH + Duration::from_millis(1234);

        let path = write_p8_aime_start_report(&config, started_at)
            .unwrap()
            .expect("configured run dir writes a start report");

        assert_eq!(path, run_dir.join("reports").join("p8-aime-start.json"));
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(report["schema"], "leaven.p8_aime.start_report.v1");
        assert_eq!(report["run_profile"], "gepa-aime");
        assert_eq!(report["gepa_profile"], "optimize-anything");
        assert_eq!(
            report["proof_classification"],
            "full_live_aime_reproduction_attempt"
        );
        assert_eq!(report["started_unix_ms"], 1234);
        assert_eq!(report["search_metric_call_cap"], GEPA_AIME_METRIC_CALLS);
        assert_eq!(report["solver_runtime"]["model"], GEPA_AIME_SOLVER_MODEL);
        assert_eq!(report["reflection_runtime"]["model"], "gpt-5.1");
        assert_eq!(
            report["comparison"]["reflection_model_alignment"],
            "upstream-matched"
        );
        assert_eq!(
            report["comparison"]["upstream_checkpoint_metric_calls"],
            GEPA_CAIS_AIME_CHECKPOINT_METRIC_CALLS
        );
        assert_eq!(report["comparison"]["upstream_run_log_available"], false);
        assert_eq!(
            report["comparison"]["upstream_reflection_model"],
            UPSTREAM_GEPA_AIME_REFLECTION_MODEL
        );
        assert_eq!(report["solver_runtime"]["request_timeout_seconds"], 120);
        let _ = std::fs::remove_dir_all(run_dir);
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
        assert_eq!(dataset.proof.train_count, 1);
        assert_eq!(dataset.proof.validation_count, 1);
        assert_eq!(dataset.proof.test_count, 1);
        assert_eq!(dataset.proof.split_seed, Some(0));
        assert!(!dataset.proof.test_repeated);
        let cache_proof = dataset
            .proof
            .materialized_cache
            .as_ref()
            .expect("loaded cache records materialized cache proof");
        assert_eq!(cache_proof.path, path.display().to_string());
        assert_eq!(cache_proof.sha256.len(), 64);
        assert!(cache_proof.sha256.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert!(cache_proof.bytes > 0);
        assert_eq!(dataset.proof.source_splits.len(), 3);
        assert!(dataset.proof.source_splits.iter().any(|source| {
            source.role == "test"
                && source.dataset == "MathArena/aime_2025"
                && source.config == "default"
                && source.split == "train"
                && source.count == 1
        }));
        std::fs::remove_file(path).unwrap();
    }
}
