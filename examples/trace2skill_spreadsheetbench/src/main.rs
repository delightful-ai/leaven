use std::{
    io::{self, ErrorKind},
    path::PathBuf,
};

use trace2skill_spreadsheetbench::{
    Trace2SkillOneCaseAnalystFanoutInput, Trace2SkillOneCaseComparisonInput,
    Trace2SkillOneCaseInput, Trace2SkillOneCaseRunInput, Trace2SkillOneCaseRunScoringInput,
    compare_trace2skill_one_case_answer, inspect_trace2skill_one_case,
    prepare_trace2skill_one_case_analyst_fanout, prepare_trace2skill_one_case_run,
    render_trace2skill_one_case_prompt, score_trace2skill_one_case_run,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse()?;
    let input = Trace2SkillOneCaseInput {
        case_file: &args.case_file,
        spreadsheet_dir: &args.spreadsheet_dir,
        system_prompt_file: &args.system_prompt_file,
        released_skill_file: &args.released_skill_file,
    };

    match args.mode {
        Mode::InspectOneCase => {
            println!(
                "{}",
                serde_json::to_string_pretty(&inspect_trace2skill_one_case(input)?)?
            );
        }
        Mode::RenderOneCasePrompt => {
            println!("{}", render_trace2skill_one_case_prompt(input)?);
        }
        Mode::CompareOneCaseAnswer => {
            let report = compare_trace2skill_one_case_answer(Trace2SkillOneCaseComparisonInput {
                case_file: &args.case_file,
                candidate_workbook: &args.output_workbook,
                golden_workbook: &args.golden_workbook,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Mode::PrepareOneCaseRun => {
            let run_dir = args
                .run_dir
                .as_deref()
                .ok_or_else(|| invalid_input(format!("--run-dir is required\n\n{USAGE}")))?;
            let output_workbook = args
                .output_workbook_was_set
                .then_some(args.output_workbook.as_path());
            let report = prepare_trace2skill_one_case_run(Trace2SkillOneCaseRunInput {
                case: input,
                run_dir,
                output_workbook,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Mode::ScoreOneCaseRun => {
            let run_dir = args
                .run_dir
                .as_deref()
                .ok_or_else(|| invalid_input(format!("--run-dir is required\n\n{USAGE}")))?;
            let model_id = args
                .model_id
                .as_deref()
                .ok_or_else(|| invalid_input(format!("--model-id is required\n\n{USAGE}")))?;
            let transcript_file = args.transcript_file.as_deref().ok_or_else(|| {
                invalid_input(format!("--transcript-file is required\n\n{USAGE}"))
            })?;
            let report = score_trace2skill_one_case_run(Trace2SkillOneCaseRunScoringInput {
                run_dir,
                model_id,
                transcript_file,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Mode::PrepareOneCaseAnalystFanout => {
            let run_dir = args
                .run_dir
                .as_deref()
                .ok_or_else(|| invalid_input(format!("--run-dir is required\n\n{USAGE}")))?;
            let report = prepare_trace2skill_one_case_analyst_fanout(
                Trace2SkillOneCaseAnalystFanoutInput {
                    run_dir,
                    upstream_prompt_dir: &args.upstream_prompt_dir,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

#[derive(Debug)]
struct CliArgs {
    mode: Mode,
    case_file: PathBuf,
    spreadsheet_dir: PathBuf,
    system_prompt_file: PathBuf,
    released_skill_file: PathBuf,
    output_workbook: PathBuf,
    output_workbook_was_set: bool,
    golden_workbook: PathBuf,
    run_dir: Option<PathBuf>,
    model_id: Option<String>,
    transcript_file: Option<PathBuf>,
    upstream_prompt_dir: PathBuf,
}

impl CliArgs {
    fn parse() -> Result<Self, io::Error> {
        Self::parse_from(std::env::args())
    }

    fn parse_from<I, S>(args: I) -> Result<Self, io::Error>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let defaults = Defaults::new();
        let mut mode = None;
        let mut case_file = defaults.case_file;
        let mut spreadsheet_dir = defaults.spreadsheet_dir;
        let mut system_prompt_file = defaults.system_prompt_file;
        let mut released_skill_file = defaults.released_skill_file;
        let mut output_workbook = defaults.output_workbook;
        let mut output_workbook_was_set = false;
        let mut golden_workbook = defaults.golden_workbook;
        let mut run_dir = None;
        let mut model_id = None;
        let mut transcript_file = None;
        let mut upstream_prompt_dir = defaults.upstream_prompt_dir;
        let mut args = args.into_iter().map(Into::into);
        let _program = args.next();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--inspect-one-case" => set_mode(&mut mode, Mode::InspectOneCase)?,
                "--render-one-case-prompt" => set_mode(&mut mode, Mode::RenderOneCasePrompt)?,
                "--compare-one-case-answer" => set_mode(&mut mode, Mode::CompareOneCaseAnswer)?,
                "--prepare-one-case-run" => set_mode(&mut mode, Mode::PrepareOneCaseRun)?,
                "--score-one-case-run" => set_mode(&mut mode, Mode::ScoreOneCaseRun)?,
                "--prepare-one-case-analyst-fanout" => {
                    set_mode(&mut mode, Mode::PrepareOneCaseAnalystFanout)?;
                }
                "--case" => case_file = next_path(&mut args, "--case")?,
                "--spreadsheet-dir" => {
                    spreadsheet_dir = next_path(&mut args, "--spreadsheet-dir")?;
                }
                "--system-prompt" => {
                    system_prompt_file = next_path(&mut args, "--system-prompt")?;
                }
                "--released-skill" => {
                    released_skill_file = next_path(&mut args, "--released-skill")?;
                }
                "--output-workbook" => {
                    output_workbook = next_path(&mut args, "--output-workbook")?;
                    output_workbook_was_set = true;
                }
                "--golden-workbook" => {
                    golden_workbook = next_path(&mut args, "--golden-workbook")?;
                }
                "--run-dir" => {
                    run_dir = Some(next_path(&mut args, "--run-dir")?);
                }
                "--model-id" => {
                    model_id = Some(next_string(&mut args, "--model-id")?);
                }
                "--transcript-file" => {
                    transcript_file = Some(next_path(&mut args, "--transcript-file")?);
                }
                "--upstream-prompt-dir" => {
                    upstream_prompt_dir = next_path(&mut args, "--upstream-prompt-dir")?;
                }
                "--help" | "-h" => return Err(invalid_input(USAGE)),
                other => {
                    return Err(invalid_input(format!(
                        "unknown argument {other}\n\n{USAGE}"
                    )));
                }
            }
        }

        Ok(Self {
            mode: mode.ok_or_else(|| invalid_input(USAGE))?,
            case_file,
            spreadsheet_dir,
            system_prompt_file,
            released_skill_file,
            output_workbook,
            output_workbook_was_set,
            golden_workbook,
            run_dir,
            model_id,
            transcript_file,
            upstream_prompt_dir,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    InspectOneCase,
    RenderOneCasePrompt,
    CompareOneCaseAnswer,
    PrepareOneCaseRun,
    ScoreOneCaseRun,
    PrepareOneCaseAnalystFanout,
}

struct Defaults {
    case_file: PathBuf,
    spreadsheet_dir: PathBuf,
    system_prompt_file: PathBuf,
    released_skill_file: PathBuf,
    output_workbook: PathBuf,
    golden_workbook: PathBuf,
    upstream_prompt_dir: PathBuf,
}

impl Defaults {
    fn new() -> Self {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let spreadsheet_dir =
            repo_root.join("tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1");
        Self {
            case_file: repo_root.join(
                "tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/dataset_first_case.json",
            ),
            output_workbook: spreadsheet_dir.join("13-1_output.xlsx"),
            golden_workbook: spreadsheet_dir.join("1_13-1_golden.xlsx"),
            spreadsheet_dir,
            system_prompt_file: repo_root.join(
                "tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_skill_preloaded_full_system_v1.txt",
            ),
            released_skill_file: repo_root.join(
                "tmp/repros/trace2skill-upstream/released_skills/trace2skill-xlsx-35B-combined/SKILL.md",
            ),
            upstream_prompt_dir: repo_root
                .join("tmp/repros/trace2skill-upstream/skill_evolver/prompts"),
        }
    }
}

fn set_mode(mode: &mut Option<Mode>, next: Mode) -> Result<(), io::Error> {
    if mode.replace(next).is_some() {
        return Err(invalid_input(format!(
            "choose only one Trace2Skill one-case mode\n\n{USAGE}"
        )));
    }
    Ok(())
}

fn next_path(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf, io::Error> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| invalid_input(format!("{flag} requires a path\n\n{USAGE}")))
}

fn next_string(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, io::Error> {
    args.next()
        .ok_or_else(|| invalid_input(format!("{flag} requires a value\n\n{USAGE}")))
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, message.into())
}

const USAGE: &str = "usage: trace2skill_spreadsheetbench \
    (--inspect-one-case | --render-one-case-prompt | --compare-one-case-answer | --prepare-one-case-run | --score-one-case-run | --prepare-one-case-analyst-fanout) \
    [--case PATH] [--spreadsheet-dir PATH] [--system-prompt PATH] [--released-skill PATH] \
    [--output-workbook PATH] [--golden-workbook PATH] [--run-dir PATH] \
    [--model-id ID] [--transcript-file PATH] [--upstream-prompt-dir PATH]";
