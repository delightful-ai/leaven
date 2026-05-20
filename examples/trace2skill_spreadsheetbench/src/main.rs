use std::{
    io::{self, ErrorKind},
    path::PathBuf,
};

use trace2skill_spreadsheetbench::{
    compare_trace2skill_one_case_answer, inspect_trace2skill_one_case,
    render_trace2skill_one_case_prompt, Trace2SkillOneCaseComparisonInput, Trace2SkillOneCaseInput,
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
    golden_workbook: PathBuf,
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
        let mut golden_workbook = defaults.golden_workbook;
        let mut args = args.into_iter().map(Into::into);
        let _program = args.next();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--inspect-one-case" => set_mode(&mut mode, Mode::InspectOneCase)?,
                "--render-one-case-prompt" => set_mode(&mut mode, Mode::RenderOneCasePrompt)?,
                "--compare-one-case-answer" => set_mode(&mut mode, Mode::CompareOneCaseAnswer)?,
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
                }
                "--golden-workbook" => {
                    golden_workbook = next_path(&mut args, "--golden-workbook")?;
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
            golden_workbook,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    InspectOneCase,
    RenderOneCasePrompt,
    CompareOneCaseAnswer,
}

struct Defaults {
    case_file: PathBuf,
    spreadsheet_dir: PathBuf,
    system_prompt_file: PathBuf,
    released_skill_file: PathBuf,
    output_workbook: PathBuf,
    golden_workbook: PathBuf,
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

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, message.into())
}

const USAGE: &str = "usage: trace2skill_spreadsheetbench \
    (--inspect-one-case | --render-one-case-prompt | --compare-one-case-answer) \
    [--case PATH] [--spreadsheet-dir PATH] [--system-prompt PATH] [--released-skill PATH] \
    [--output-workbook PATH] [--golden-workbook PATH]";
