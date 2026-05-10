use std::collections::BTreeMap;

use futures::executor::block_on;
use leaven::prelude::*;
use leaven::stdlib::populations::ParetoFrontier;
use leaven::{SurfaceError, SurfaceFingerprint};

const BASELINE: &str =
    "Solve the math problem carefully. Provide the final answer as a single number.";
const OPTIMIZED: &str = "Solve with modular arithmetic when useful. Verify arithmetic before the final answer. Provide only the final integer.";

fn main() {
    block_on(async {
        let result = run_deterministic_aime().await;
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
            "heldout_test_score={:.3}",
            result.report.test_score.expect("test is configured")
        );
        println!("best_system_prompt={}", result.best().system);
        println!("events={}", result.report.events.join(","));
    });
}

async fn run_deterministic_aime() -> OptimizeResult<AimePrompt> {
    let (train, validation, test) = deterministic_cases();
    leaven::optimize(AimePrompt::new(BASELINE))
        .train(train)
        .validation(validation)
        .test(test)
        .runner(run_solver)
        .score(score_answer)
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
                .reflector(ReflectiveMutation::new(AimePromptEdit::ReplaceSystem(
                    OPTIMIZED.to_owned(),
                )))
                .max_iterations(1),
        )
        .budget(Budget::metric_calls(64))
        .run()
        .await
        .expect("deterministic AIME GEPA run succeeds")
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
enum AimePromptEdit {
    ReplaceSystem(String),
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
    type Edit = AimePromptEdit;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(leaven_kernel::Fingerprint::from_bytes([8; 32]))
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
        let AimePromptEdit::ReplaceSystem(system) = edit;
        Ok(AimePromptChange { system })
    }
}

#[derive(Clone, Debug)]
struct AimeCase {
    problem: &'static str,
    answer: i64,
    solution: &'static str,
    needs_modular: bool,
}

fn deterministic_cases() -> (Vec<AimeCase>, Vec<AimeCase>, Vec<AimeCase>) {
    let train = vec![
        AimeCase {
            problem: "Find the remainder when 2^10 is divided by 7.",
            answer: 2,
            solution: "2^3 = 8 == 1 mod 7, so 2^10 == 2 mod 7.",
            needs_modular: true,
        },
        AimeCase {
            problem: "What is 19 + 23?",
            answer: 42,
            solution: "19 + 23 = 42.",
            needs_modular: false,
        },
        AimeCase {
            problem: "Find the remainder when 5^4 is divided by 13.",
            answer: 1,
            solution: "5^2 = 25 == -1 mod 13, so 5^4 == 1.",
            needs_modular: true,
        },
    ];
    let validation = vec![AimeCase {
        problem: "Find the remainder when 3^6 is divided by 7.",
        answer: 1,
        solution: "3^6 = 729 == 1 mod 7.",
        needs_modular: true,
    }];
    let test = vec![
        AimeCase {
            problem: "Find the remainder when 4^5 is divided by 9.",
            answer: 7,
            solution: "4^3 == 1 mod 9, so 4^5 == 4^2 == 7.",
            needs_modular: true,
        },
        AimeCase {
            problem: "What is 31 - 8?",
            answer: 23,
            solution: "31 - 8 = 23.",
            needs_modular: false,
        },
    ];
    (train, validation, test)
}

fn run_solver(prompt: &AimePrompt, case: &AimeCase) -> RunOutput {
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

fn score_answer(ctx: ScoreContext<'_, AimePrompt, AimeCase>) -> Score {
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

    #[test]
    fn deterministic_aime_acceptance_shows_public_gepa_improvement() {
        let result = block_on(run_deterministic_aime());

        assert_eq!(result.report.baseline_train_score, 0.0);
        assert_eq!(result.report.optimized_train_score, 1.0);
        assert_eq!(result.report.validation_score, Some(1.0));
        assert_eq!(result.report.test_score, Some(1.0));
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
}
