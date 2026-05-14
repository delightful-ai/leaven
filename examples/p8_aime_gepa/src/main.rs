use std::{
    collections::BTreeMap,
    path::Path,
    process::{Command, Stdio},
};

use futures::executor::block_on;
use leaven::prelude::*;
use leaven::stdlib::populations::ParetoFrontier;
use leaven::{ProposalBatchSemantics, SurfaceError, SurfaceFingerprint, kernel::Metered};
use leaven_agent::{AgentSession, FakeAgentAction, FakeAgentRuntime};
use leaven_stage::{AgentStageCallContext, ProposerSlot, StageOutputParseError, StageOutputParser};
use leaven_workspace::{WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde::{Deserialize, Serialize};

const BASELINE: &str =
    "Solve the math problem carefully. Provide the final answer as a single number.";
const OPTIMIZED: &str = "Solve with modular arithmetic when useful. Verify arithmetic before the final answer. Provide only the final integer.";

fn main() {
    block_on(async {
        let result = run_configured_aime().await;
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
        println!("best_system_prompt={}", result.best().system);
        println!("events={}", result.report.events.join(","));
    });
}

#[cfg(test)]
async fn run_deterministic_aime() -> OptimizeResult<AimePrompt> {
    let (train, validation, test) = deterministic_cases();
    run_aime(train, validation, test).await
}

async fn run_configured_aime() -> OptimizeResult<AimePrompt> {
    let (train, validation, test) = configured_cases();
    run_aime(train, validation, test).await
}

async fn run_aime(
    train: Vec<AimeCase>,
    validation: Vec<AimeCase>,
    test: Vec<AimeCase>,
) -> OptimizeResult<AimePrompt> {
    leaven::optimize(AimePrompt::new(BASELINE))
        .train(train)
        .validation(validation)
        .test(test)
        .runner(run_solver)
        .score(|ctx| score_answer(&ctx))
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
                .reflector(aime_stage_reflector())
                .max_iterations(1),
        )
        .budget(Budget::metric_calls(512))
        .run()
        .await
        .expect("deterministic AIME GEPA run succeeds")
}

fn aime_stage_reflector() -> leaven_gepa::GepaStageProposer<FakeAgentRuntime, AimeProposalParser> {
    leaven_gepa::gepa_stage_proposer(
        LocalWorkspaceFactory::temp(),
        FakeAgentRuntime::new(vec![
            FakeAgentAction::ReadFile {
                path: WorkspacePath::new("focus/request.json").unwrap(),
            },
            FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/proposal.json").unwrap(),
                bytes: serde_json::to_vec(&AimeProposalOutput {
                    system: OPTIMIZED.to_owned(),
                })
                .unwrap(),
            },
        ]),
        AimeProposalParser,
        Default::default(),
    )
}

struct AimeProposalParser;

impl<P> StageOutputParser<P, ProposerSlot<leaven_gepa::ReflectRequest>> for AimeProposalParser
where
    P: OptimizationProblem<Artifact = AimePrompt, ProposalAnnotations = ()>,
{
    async fn parse(
        &self,
        workspace: &mut WorkspaceView<'_>,
        _session: &AgentSession,
        plan: &leaven_stage::parser::ErasedStagePlan,
        _ctx: AgentStageCallContext,
    ) -> Result<Metered<ProposalBatch<P>>, StageOutputParseError> {
        let bytes = workspace.read_file(&WorkspacePath::new("output/proposal.json").unwrap())?;
        let parsed: AimeProposalOutput = serde_json::from_slice(&bytes)?;
        let request: leaven_gepa::ReflectRequest =
            serde_json::from_value(plan.request_json.clone())?;
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![
                    Proposal::mutate(
                        request.parent,
                        AimePromptChange {
                            system: parsed.system,
                        },
                    )
                    .informed_by(request.selected_feedback.source_refs())
                    .build(),
                ],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}

#[derive(Deserialize, Serialize)]
struct AimeProposalOutput {
    system: String,
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
    type Edit = AimePromptChange;

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
        Ok(edit)
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

fn run_solver(prompt: &AimePrompt, case: &AimeCase) -> RunOutput {
    if std::env::var_os("LEAVEN_AIME_LIVE_OPENAI").is_some() {
        return run_openai_solver(prompt, case);
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

fn run_openai_solver(prompt: &AimePrompt, case: &AimeCase) -> RunOutput {
    let python = std::env::var("LEAVEN_OPENAI_PYTHON").unwrap_or_else(|_| "python3".to_owned());
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/openai_solver.py");
    let output = Command::new(&python)
        .arg(script)
        .env("LEAVEN_AIME_SYSTEM_PROMPT", &prompt.system)
        .env("LEAVEN_AIME_PROBLEM", &case.problem)
        .stdin(Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let answer = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            RunOutput::new(
                answer.clone(),
                vec![
                    "provider: openai-responses".to_owned(),
                    format!("model: {}", openai_model_name()),
                    format!("problem: {}", case.problem),
                    format!("system_prompt: {}", prompt.system),
                    format!("solver_answer: {answer}"),
                ],
            )
        }
        Ok(output) => RunOutput::new(
            String::new(),
            vec![
                "provider: openai-responses".to_owned(),
                format!("model: {}", openai_model_name()),
                format!("problem: {}", case.problem),
                format!("openai_status: {}", output.status),
                format!("openai_stderr: {}", String::from_utf8_lossy(&output.stderr)),
            ],
        ),
        Err(source) => RunOutput::new(
            String::new(),
            vec![
                "provider: openai-responses".to_owned(),
                format!("model: {}", openai_model_name()),
                format!("problem: {}", case.problem),
                format!("openai_spawn_error: {source}"),
            ],
        ),
    }
}

fn openai_model_name() -> String {
    std::env::var("LEAVEN_OPENAI_MODEL").unwrap_or_else(|_| "gpt-4.1-mini".to_owned())
}

fn score_answer(ctx: &ScoreContext<'_, AimePrompt, AimeCase>) -> Score {
    let parsed = ctx.output.output.parse::<i64>();
    match parsed {
        Ok(answer) if answer == ctx.case.answer => Score::new(
            1.0,
            format!(
                "correct; expected {}. {}",
                ctx.case.answer, ctx.case.solution
            ),
        ),
        Ok(answer) => Score::new(
            0.0,
            format!(
                "incorrect; got {answer}, expected {}. {}",
                ctx.case.answer, ctx.case.solution
            ),
        ),
        Err(_) => Score::new(
            0.0,
            format!(
                "final answer must parse as an integer; expected {}. {}",
                ctx.case.answer, ctx.case.solution
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
        let (train, _, _) = deterministic_cases();
        let result = block_on(run_aime(train, Vec::new(), Vec::new()));

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
        let (train, _, _) = deterministic_cases();
        let error = block_on(async {
            leaven::optimize(AimePrompt::new(BASELINE))
                .train(train)
                .runner(run_solver)
                .using(
                    Gepa::builder()
                        .surface(AimePromptSurface)
                        .reflector(aime_stage_reflector()),
                )
                .budget(Budget::metric_calls(8))
                .run()
                .await
        })
        .unwrap_err();

        assert!(error.to_string().contains("score function is required"));
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
