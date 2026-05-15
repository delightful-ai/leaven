use std::{collections::BTreeMap, num::NonZeroUsize, path::Path};

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
use leaven_lm_openai::OpenAiLm;
use serde::{Deserialize, Serialize};

const BASELINE: &str = "Solve the math problem carefully. Break down the steps and provide the final answer as a single number.";
const OPTIMIZED: &str = "Solve with modular arithmetic when useful. Verify arithmetic before the final answer. Provide only the final integer.";
const GEPA_AIME_METRIC_CALLS: u64 = 500;
const GEPA_AIME_MAX_WORKERS: usize = 32;
const GEPA_AIME_MAX_OUTPUT_TOKENS: u32 = 32_000;
// GEPA AIME is controlled by max_metric_calls, not max_iterations. This is a
// Leaven-local safety ceiling until the GEPA loop has a native budget stopper.
const GEPA_AIME_INTERNAL_ITERATION_CEILING: usize = 500;
const GEPA_AIME_SOLVER_MODEL: &str = "gpt-4.1-mini";
const GEPA_AIME_REFLECTION_MODEL: &str = "gpt-5.4-mini";
const DETERMINISTIC_SMOKE_METRIC_CALLS: u64 = 512;
const DETERMINISTIC_SMOKE_ITERATIONS: usize = 1;

type AimeLmReflector =
    LmBackedReflector<AimeReflectionLm, DefaultReflectionRenderer, PlainTextEditParser>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config = AimeRunConfig::configured();
    let result = run_configured_aime(config.clone()).await;
    println!("run_profile={}", config.profile.label());
    println!(
        "baseline_train_score={:.3}",
        result.report.baseline_train_score
    );
    println!(
        "optimized_train_score={:.3}",
        result.report.optimized_train_score
    );
    println!(
        "validation_score={:.3}",
        result
            .report
            .validation_score
            .expect("validation is configured")
    );
    println!(
        "baseline_heldout_test_score={:.3}",
        result
            .report
            .baseline_test_score
            .expect("test is configured")
    );
    println!(
        "heldout_test_score={:.3}",
        result.report.test_score.expect("test is configured")
    );
    println!(
        "report_splits={}",
        result.report.evaluation.splits_reported.len()
    );
    println!(
        "budget_metric_calls={}",
        result.report.budget.spent.metric_calls
    );
    println!("budget_llm_calls={}", result.report.budget.spent.llm_calls);
    println!("best_system_prompt={}", result.best().system);
    println!("events={}", result.report.events.join(","));
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
            },
            reflection: AimeReflectionConfig {
                live: std::env::var_os("LEAVEN_AIME_LIVE_OPENAI_REFLECTION").is_some(),
                model: aime_reflection_model_name(),
                sampling: SamplingOptions::default().with_reasoning_effort(ReasoningEffort::Medium),
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
            },
            reflection: AimeReflectionConfig {
                live: false,
                model: "deterministic-aime-reflector".to_owned(),
                sampling: SamplingOptions::default().with_reasoning_effort(ReasoningEffort::Medium),
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
}

#[derive(Clone, Debug)]
struct AimeReflectionConfig {
    live: bool,
    model: String,
    sampling: SamplingOptions,
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
        AimeReflectionLm::OpenAi(
            OpenAiLm::from_env(&model).expect("OPENAI_API_KEY is required for live reflection"),
        )
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
    OpenAi(OpenAiLm),
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
            problem: "Find the remainder when 2^10 is divided by 7.".to_owned(),
            answer: 2,
            solution: "2^3 = 8 == 1 mod 7, so 2^10 == 2 mod 7.".to_owned(),
            needs_modular: true,
        },
        AimeCase {
            problem: "What is 19 + 23?".to_owned(),
            answer: 42,
            solution: "19 + 23 = 42.".to_owned(),
            needs_modular: false,
        },
        AimeCase {
            problem: "Find the remainder when 5^4 is divided by 13.".to_owned(),
            answer: 1,
            solution: "5^2 = 25 == -1 mod 13, so 5^4 == 1.".to_owned(),
            needs_modular: true,
        },
    ];
    let validation = vec![AimeCase {
        problem: "Find the remainder when 3^6 is divided by 7.".to_owned(),
        answer: 1,
        solution: "3^6 = 729 == 1 mod 7.".to_owned(),
        needs_modular: true,
    }];
    let test = vec![
        AimeCase {
            problem: "Find the remainder when 4^5 is divided by 9.".to_owned(),
            answer: 7,
            solution: "4^3 == 1 mod 9, so 4^5 == 4^2 == 7.".to_owned(),
            needs_modular: true,
        },
        AimeCase {
            problem: "What is 31 - 8?".to_owned(),
            answer: 23,
            solution: "31 - 8 = 23.".to_owned(),
            needs_modular: false,
        },
    ];
    (train, validation, test)
}

fn aime_solver_lm(config: &AimeSolverConfig) -> Option<OpenAiLm> {
    if config.live {
        Some(OpenAiLm::from_env(&config.model).expect("OPENAI_API_KEY is required for live solver"))
    } else {
        None
    }
}

async fn run_solver(
    prompt: AimePrompt,
    case: AimeCase,
    solver: Option<OpenAiLm>,
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
            format!("problem: {}", case.problem),
            format!("system_prompt: {}", prompt.system),
            format!("solver_answer: {answer}"),
        ],
    )
}

async fn run_openai_solver(
    solver: OpenAiLm,
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

        assert_score(result.report.baseline_train_score, 0.0);
        assert_score(result.report.optimized_train_score, 1.0);
        assert_optional_score(result.report.baseline_validation_score, 0.0);
        assert_optional_score(result.report.validation_score, 1.0);
        assert_optional_score(result.report.baseline_test_score, 0.0);
        assert_optional_score(result.report.test_score, 1.0);
        assert_eq!(result.report.evaluation.splits_reported.len(), 3);
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

        assert_score(result.report.baseline_train_score, 0.0);
        assert_score(result.report.optimized_train_score, 1.0);
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
        assert_eq!(
            config.solver.sampling.temperature.map(FiniteF64::as_f64),
            Some(1.0)
        );
        assert_eq!(
            config.solver.sampling.max_output_tokens,
            Some(GEPA_AIME_MAX_OUTPUT_TOKENS)
        );
        assert_eq!(config.reflection.model, aime_reflection_model_name());
        assert_eq!(
            config.reflection.sampling.reasoning_effort,
            Some(ReasoningEffort::Medium)
        );
    }

    #[test]
    fn aime_cache_loading_preserves_train_validation_test_roles() {
        let path =
            std::env::temp_dir().join(format!("leaven-aime-cache-{}.json", std::process::id()));
        let cache = AimeDatasetCache {
            train: vec![AimeCase {
                problem: "train".to_owned(),
                answer: 1,
                solution: "train solution".to_owned(),
                needs_modular: true,
            }],
            validation: vec![AimeCase {
                problem: "validation".to_owned(),
                answer: 2,
                solution: "validation solution".to_owned(),
                needs_modular: true,
            }],
            test: vec![AimeCase {
                problem: "test".to_owned(),
                answer: 3,
                solution: "test solution".to_owned(),
                needs_modular: true,
            }],
        };
        std::fs::write(&path, serde_json::to_vec(&cache).unwrap()).unwrap();

        let (train, validation, test) = cases_from_cache(&path);

        assert_eq!(train[0].problem, "train");
        assert_eq!(validation[0].problem, "validation");
        assert_eq!(test[0].problem, "test");
        std::fs::remove_file(path).unwrap();
    }
}
