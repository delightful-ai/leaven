use std::{collections::BTreeMap, num::NonZeroUsize, path::Path, time::Duration};

use leaven::prelude::*;
use leaven::{
    SurfaceError, SurfaceFingerprint, kernel::Metered, stdlib::populations::ParetoFrontier,
};
use leaven_gepa::{
    DefaultReflectionRenderer, LmBackedReflector, LmBackedReflectorConfig, PlainTextEditParser,
};
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

type CachedOpenAiLm = CachedLm<OpenAiLm, InMemoryLmCache>;
type AimeLmReflector =
    LmBackedReflector<AimeReflectionLm, DefaultReflectionRenderer, PlainTextEditParser>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config = AimeRunConfig::configured();
    let result = run_configured_aime(config.clone()).await;
    for line in report_lines(&config, &result) {
        println!("{line}");
    }
}

fn report_lines(config: &AimeRunConfig, result: &OptimizeResult<AimePrompt>) -> Vec<String> {
    let mut lines = vec![
        format!("run_profile={}", config.profile.label()),
        format!(
            "baseline_train_score={}",
            report_score(result.report.baseline_train_score)
        ),
        format!(
            "optimized_train_score={}",
            report_score(result.report.optimized_train_score)
        ),
        format!(
            "validation_score={}",
            report_score(result.report.validation_score)
        ),
        format!(
            "baseline_heldout_test_score={}",
            report_score(result.report.baseline_test_score)
        ),
        format!(
            "heldout_test_score={}",
            report_score(result.report.test_score)
        ),
        "test_score_use=final_report_only".to_owned(),
        format!(
            "report_splits={}",
            result.report.evaluation.splits_reported.len()
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
        format!(
            "stop_reason={}",
            report_stop_reason(result.report.stop_reason)
        ),
        format!(
            "optimization_metric_calls={}",
            result.report.optimization_cost.metric_calls
        ),
        format!(
            "final_report_metric_calls={}",
            result.report.final_report_cost.metric_calls
        ),
        format!(
            "budget_metric_calls={}",
            result.report.budget.spent.metric_calls
        ),
        format!("budget_llm_calls={}", result.report.budget.spent.llm_calls),
        format!("best_system_prompt={}", result.best().system),
        format!("events={}", result.report.events.join(",")),
    ];
    for split in &result.report.evaluation.splits_reported {
        for candidate in &split.candidates {
            for case in &candidate.cases {
                lines.push(format!(
                    "report_case={} split={:?} source_id={} score={:.3} feedback_chars={} trace_lines={}",
                    case.case_id,
                    split.role,
                    report_source_id(&case.trace),
                    case.score,
                    case.feedback.len(),
                    case.trace.len()
                ));
            }
        }
    }
    lines
}

fn report_source_id(trace: &[String]) -> &str {
    trace
        .iter()
        .find_map(|line| line.strip_prefix("source_id: "))
        .unwrap_or("absent")
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
        leaven::run::OptimizationStopReason::BudgetExceeded => "budget_exceeded",
        leaven::run::OptimizationStopReason::StopperTriggered => "stopper_triggered",
        leaven::run::OptimizationStopReason::External => "external",
        leaven::run::OptimizationStopReason::Error => "error",
    }
}

#[cfg(test)]
async fn run_deterministic_aime() -> OptimizeResult<AimePrompt> {
    let config = AimeRunConfig::deterministic_smoke();
    let (train, validation, test) = deterministic_cases();
    run_aime(config, train, validation, test).await
}

async fn run_configured_aime(config: AimeRunConfig) -> OptimizeResult<AimePrompt> {
    let (train, validation, test) = configured_cases();
    run_aime(config, train, validation, test).await
}

async fn run_aime(
    config: AimeRunConfig,
    train: Vec<AimeCase>,
    validation: Vec<AimeCase>,
    test: Vec<AimeCase>,
) -> OptimizeResult<AimePrompt> {
    let solver = aime_solver_lm(&config.solver);
    let solver_config = config.solver.clone();
    leaven::optimize(AimePrompt::new(config.seed_prompt))
        .train(train)
        .validation(validation)
        .test(test)
        .runner(move |prompt, case| {
            let solver = solver.clone();
            let solver_config = solver_config.clone();
            async move { run_solver(prompt, case, solver, solver_config).await }
        })
        .score(score_answer)
        .evaluation_parallelism(config.evaluation_parallelism)
        .using(
            Gepa::builder()
                .surface(AimePromptSurface)
                .population(
                    ParetoFrontier::by_case()
                        .partition_filter(std::collections::BTreeSet::from([PartitionId::from(
                            "TRAIN",
                        )]))
                        .build(),
                )
                .reflector(aime_lm_reflector(&config.reflection))
                .max_iterations(config.max_iterations),
        )
        .budget(config.budget)
        .run()
        .await
        .expect("AIME GEPA run succeeds")
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

fn aime_lm_reflector(config: &AimeReflectionConfig) -> AimeLmReflector {
    let model = config.model.clone();
    let lm = if config.live {
        AimeReflectionLm::OpenAi(cached_openai_lm(
            config.cache_policy,
            config.runtime,
            "live reflection",
        ))
    } else {
        AimeReflectionLm::Deterministic(DeterministicReflectionLm)
    };
    LmBackedReflector::new(lm, model, DefaultReflectionRenderer, PlainTextEditParser).with_config(
        LmBackedReflectorConfig {
            sampling: config.sampling.clone(),
            output: leaven_lm::OutputMode::Text,
            prompt_template: None,
        },
    )
}

fn aime_reflection_model_name() -> String {
    std::env::var("LEAVEN_AIME_REFLECTION_MODEL")
        .unwrap_or_else(|_| GEPA_AIME_REFLECTION_MODEL.to_owned())
}

#[derive(Clone)]
enum AimeReflectionLm {
    Deterministic(DeterministicReflectionLm),
    OpenAi(CachedOpenAiLm),
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

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AimeCase {
    source_id: String,
    problem: String,
    answer: i64,
    solution: String,
    needs_modular: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AimeDatasetCache {
    train: Vec<AimeCase>,
    validation: Vec<AimeCase>,
    test: Vec<AimeCase>,
}

fn configured_cases() -> (Vec<AimeCase>, Vec<AimeCase>, Vec<AimeCase>) {
    match std::env::var("LEAVEN_AIME_CACHE") {
        Ok(path) => cases_from_cache(Path::new(&path)),
        Err(_) => deterministic_cases(),
    }
}

fn cases_from_cache(path: &Path) -> (Vec<AimeCase>, Vec<AimeCase>, Vec<AimeCase>) {
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
    (cache.train, cache.validation, cache.test)
}

fn deterministic_cases() -> (Vec<AimeCase>, Vec<AimeCase>, Vec<AimeCase>) {
    let train = vec![
        AimeCase {
            source_id: "deterministic:train:0".to_owned(),
            problem: "Find the remainder when 2^10 is divided by 7.".to_owned(),
            answer: 2,
            solution: "2^3 = 8 == 1 mod 7, so 2^10 == 2 mod 7.".to_owned(),
            needs_modular: true,
        },
        AimeCase {
            source_id: "deterministic:train:1".to_owned(),
            problem: "What is 19 + 23?".to_owned(),
            answer: 42,
            solution: "19 + 23 = 42.".to_owned(),
            needs_modular: false,
        },
        AimeCase {
            source_id: "deterministic:train:2".to_owned(),
            problem: "Find the remainder when 5^4 is divided by 13.".to_owned(),
            answer: 1,
            solution: "5^2 = 25 == -1 mod 13, so 5^4 == 1.".to_owned(),
            needs_modular: true,
        },
    ];
    let validation = vec![AimeCase {
        source_id: "deterministic:validation:0".to_owned(),
        problem: "Find the remainder when 3^6 is divided by 7.".to_owned(),
        answer: 1,
        solution: "3^6 = 729 == 1 mod 7.".to_owned(),
        needs_modular: true,
    }];
    let test = vec![
        AimeCase {
            source_id: "deterministic:test:0".to_owned(),
            problem: "Find the remainder when 4^5 is divided by 9.".to_owned(),
            answer: 7,
            solution: "4^3 == 1 mod 9, so 4^5 == 4^2 == 7.".to_owned(),
            needs_modular: true,
        },
        AimeCase {
            source_id: "deterministic:test:1".to_owned(),
            problem: "What is 31 - 8?".to_owned(),
            answer: 23,
            solution: "31 - 8 = 23.".to_owned(),
            needs_modular: false,
        },
    ];
    (train, validation, test)
}

fn aime_solver_lm(config: &AimeSolverConfig) -> Option<CachedOpenAiLm> {
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
) -> CachedOpenAiLm {
    let config = OpenAiConfig::from_env()
        .unwrap_or_else(|source| panic!("OPENAI_API_KEY is required for {role}: {source}"))
        .with_throttle_policy(OpenAiThrottlePolicy::new(
            runtime.max_concurrent_requests,
            Duration::ZERO,
        ));
    let inner = OpenAiLm::new(config);
    match runtime.cache_backend {
        AimeLmCacheBackend::InMemory => {
            CachedLm::new(inner, InMemoryLmCache::default(), cache_policy)
        }
    }
}

async fn run_solver(
    prompt: AimePrompt,
    case: AimeCase,
    solver: Option<CachedOpenAiLm>,
    solver_config: AimeSolverConfig,
) -> RunOutput {
    if let Some(solver) = solver {
        return run_openai_solver(solver, &prompt, &case, &solver_config).await;
    }
    let has_modular = prompt.system.contains("modular arithmetic");
    let verifies = prompt.system.contains("Verify arithmetic");
    let correct = (!case.needs_modular || has_modular) && verifies;
    let answer = if correct {
        case.answer
    } else {
        case.answer + 1
    };
    RunOutput::new(
        answer.to_string(),
        vec![
            format!("source_id: {}", case.source_id),
            format!("problem: {}", case.problem),
            format!("system_prompt: {}", prompt.system),
            format!("solver_answer: {answer}"),
        ],
    )
}

async fn run_openai_solver(
    solver: CachedOpenAiLm,
    prompt: &AimePrompt,
    case: &AimeCase,
    solver_config: &AimeSolverConfig,
) -> RunOutput {
    let request = LmRequest::new(
        solver_config.model.clone(),
        Messages::new()
            .with_system(prompt.system.clone())
            .with_user(format!(
                "Problem:\n{}\n\nReturn only the final numerical answer.",
                case.problem
            )),
    )
    .with_sampling(solver_config.sampling.clone());
    match solver.complete(request).await {
        Ok(metered) => {
            let answer = metered.value.assistant.content().trim().to_owned();
            RunOutput::new(
                answer.clone(),
                vec![
                    "provider: openai-responses".to_owned(),
                    format!("model: {}", solver_config.model),
                    format!("source_id: {}", case.source_id),
                    format!("problem: {}", case.problem),
                    format!("system_prompt: {}", prompt.system),
                    format!("solver_answer: {answer}"),
                ],
            )
            .with_cost(metered.cost)
        }
        Err(source) => RunOutput::new(
            String::new(),
            vec![
                "provider: openai-responses".to_owned(),
                format!("model: {}", solver_config.model),
                format!("source_id: {}", case.source_id),
                format!("problem: {}", case.problem),
                format!("openai_error: {source}"),
            ],
        ),
    }
}

fn openai_model_name() -> String {
    std::env::var("LEAVEN_OPENAI_MODEL").unwrap_or_else(|_| GEPA_AIME_SOLVER_MODEL.to_owned())
}

async fn score_answer(ctx: ScoreContext<AimePrompt, AimeCase>) -> Result<Score, ScoreError> {
    let parsed = ctx.output.output.parse::<i64>();
    let score = match parsed {
        Ok(answer) if answer == ctx.case.answer => {
            Score::new(1.0, format!("correct.{}", solution_feedback(&ctx.case)))
        }
        Ok(answer) => Score::new(
            0.0,
            format!(
                "incorrect; got {answer}, expected {}.{}",
                ctx.case.answer,
                solution_feedback(&ctx.case)
            ),
        ),
        Err(_) => Score::new(
            0.0,
            format!(
                "final answer must parse as an integer; expected {}.{}",
                ctx.case.answer,
                solution_feedback(&ctx.case)
            ),
        ),
    };
    Ok(score)
}

fn solution_feedback(case: &AimeCase) -> String {
    format!(
        " Here's the full step-by-step solution:\n{}\n\nThink about what takeaways you can learn from this solution to improve your future answers and approach to similar problems.",
        case.solution
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
    fn deterministic_aime_acceptance_shows_public_gepa_improvement() {
        let result = block_on(run_deterministic_aime());

        assert_eq!(result.report.baseline_train_score, None);
        assert_eq!(result.report.optimized_train_score, None);
        assert_optional_score(result.report.baseline_validation_score, 0.0);
        assert_optional_score(result.report.validation_score, 1.0);
        assert_optional_score(result.report.baseline_test_score, 0.0);
        assert_optional_score(result.report.test_score, 1.0);
        assert_eq!(result.report.evaluation.splits_reported.len(), 2);
        assert!(result.report.budget.spent.metric_calls > 0);
        assert_eq!(result.report.budget.spent.llm_calls, 1);
        assert_eq!(result.report.budget.spent.prompt_tokens, 37);
        assert_eq!(result.report.budget.spent.completion_tokens, 11);
        assert!(
            result
                .report
                .evaluation
                .splits_reported
                .iter()
                .flat_map(|split| &split.candidates)
                .flat_map(|candidate| &candidate.cases)
                .any(|case| !case.feedback.is_empty() && !case.trace.is_empty())
        );
        assert!(result.best().system.contains("modular arithmetic"));
        assert!(
            result
                .report
                .events
                .iter()
                .any(|event| event == "proposal_recorded")
        );
        assert!(
            result
                .report
                .events
                .iter()
                .any(|event| event == "evaluation_completed")
        );
        assert!(
            result
                .report
                .events
                .iter()
                .any(|event| event == "optimization_ended")
        );
    }

    #[test]
    fn train_only_run_reports_absent_validation_and_test_scores() {
        let config = AimeRunConfig::deterministic_smoke();
        let (train, _, _) = deterministic_cases();
        let result = block_on(run_aime(config, train, Vec::new(), Vec::new()));

        assert_eq!(result.report.baseline_train_score, None);
        assert_eq!(result.report.optimized_train_score, None);
        assert_eq!(result.report.baseline_validation_score, None);
        assert_eq!(result.report.validation_score, None);
        assert_eq!(result.report.baseline_test_score, None);
        assert_eq!(result.report.test_score, None);
        assert_eq!(result.report().events.len(), result.report.events.len());
        assert_eq!(result.best(), &result.best_artifact);
    }

    #[test]
    fn run_builder_requires_score_function() {
        let config = AimeRunConfig::deterministic_smoke();
        let solver_config = config.solver.clone();
        let (train, _, _) = deterministic_cases();
        let error = block_on(async {
            leaven::optimize(AimePrompt::new(config.seed_prompt))
                .train(train)
                .runner(move |prompt, case| {
                    let solver_config = solver_config.clone();
                    async move { run_solver(prompt, case, None, solver_config).await }
                })
                .using(
                    Gepa::builder()
                        .surface(AimePromptSurface)
                        .reflector(aime_lm_reflector(&config.reflection)),
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

        assert!(
            lines
                .iter()
                .any(|line| line == "optimization_metric_calls=6")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "final_report_metric_calls=6")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "test_score_use=final_report_only")
        );
        assert!(lines.iter().any(|line| {
            line.contains("report_case=case:3")
                && line.contains("source_id=deterministic:validation:0")
                && line.contains("feedback_chars=")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("report_case=case:4")
                && line.contains("source_id=deterministic:test:0")
                && line.contains("feedback_chars=")
        }));
    }

    #[test]
    fn deterministic_metric_call_budget_stops_gepa_cleanly_before_second_step() {
        let mut config = AimeRunConfig::deterministic_smoke();
        config.budget = Budget::metric_calls(6);
        config.max_iterations = 2;
        let (train, validation, test) = deterministic_cases();
        let result = block_on(run_aime(config.clone(), train, validation, test));

        assert_eq!(
            result.report.stop_reason,
            leaven::run::OptimizationStopReason::BudgetExceeded
        );
        assert_eq!(result.report.optimization_cost.metric_calls, 6);
        assert!(
            result
                .report
                .events
                .iter()
                .any(|event| event == "optimization_stopping")
        );
        assert!(
            !result.report.events.iter().any(|event| event == "error"),
            "metric-call stop should be a clean public stop, not an optimizer error"
        );
        let lines = report_lines(&config, &result);
        assert!(
            lines
                .iter()
                .any(|line| line == "stop_reason=budget_exceeded")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "optimization_metric_calls=6")
        );
    }

    #[test]
    fn cache_policy_parser_keeps_solver_and_reflection_roles_independent() {
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
    fn public_report_preserves_case_ids_and_aime_source_ids() {
        let result = block_on(run_deterministic_aime());

        let cases = result
            .report
            .evaluation
            .splits_reported
            .iter()
            .flat_map(|split| &split.candidates)
            .flat_map(|candidate| &candidate.cases)
            .collect::<Vec<_>>();

        assert!(
            cases.iter().any(|case| {
                case.case_id == CaseId::from_index(3)
                    && case
                        .trace
                        .iter()
                        .any(|line| line == "source_id: deterministic:validation:0")
            }),
            "expected deterministic validation case id and source id in public report"
        );
        assert!(
            cases.iter().any(|case| {
                case.case_id == CaseId::from_index(4)
                    && case
                        .trace
                        .iter()
                        .any(|line| line == "source_id: deterministic:test:0")
            }),
            "expected deterministic test case id and source id in public report"
        );
    }

    #[test]
    fn aime_cache_loading_preserves_train_validation_test_roles() {
        let path =
            std::env::temp_dir().join(format!("leaven-aime-cache-{}.json", std::process::id()));
        let cache = AimeDatasetCache {
            train: vec![AimeCase {
                source_id: "AI-MO/aimo-validation-aime:default:train:17".to_owned(),
                problem: "train".to_owned(),
                answer: 1,
                solution: "train solution".to_owned(),
                needs_modular: true,
            }],
            validation: vec![AimeCase {
                source_id: "AI-MO/aimo-validation-aime:default:train:42".to_owned(),
                problem: "validation".to_owned(),
                answer: 2,
                solution: "validation solution".to_owned(),
                needs_modular: true,
            }],
            test: vec![AimeCase {
                source_id: "MathArena/aime_2025:default:train:3".to_owned(),
                problem: "test".to_owned(),
                answer: 3,
                solution: "test solution".to_owned(),
                needs_modular: true,
            }],
        };
        std::fs::write(&path, serde_json::to_vec(&cache).unwrap()).unwrap();

        let (train, validation, test) = cases_from_cache(&path);

        assert_eq!(
            train[0].source_id,
            "AI-MO/aimo-validation-aime:default:train:17"
        );
        assert_eq!(train[0].problem, "train");
        assert_eq!(
            validation[0].source_id,
            "AI-MO/aimo-validation-aime:default:train:42"
        );
        assert_eq!(validation[0].problem, "validation");
        assert_eq!(test[0].source_id, "MathArena/aime_2025:default:train:3");
        assert_eq!(test[0].problem, "test");
        std::fs::remove_file(path).unwrap();
    }
}
