mod doctor;
mod fixture;

use std::path::PathBuf;
use std::process::ExitCode;

use doctor::{DoctorCommand, OutputFormat};

fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<String, CliError> {
    let command = parse(args)?;
    command.run().map_err(CliError::Doctor)
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<DoctorCommand, CliError> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        None | Some("doctor") => parse_doctor(args),
        Some("-h" | "--help" | "help") => Ok(DoctorCommand::Help),
        Some(other) => Err(CliError::UnknownCommand(other.to_owned())),
    }
}

fn parse_doctor(mut args: impl Iterator<Item = String>) -> Result<DoctorCommand, CliError> {
    match args.next().as_deref() {
        None => Ok(DoctorCommand::Summary),
        Some("proposal-render") => {
            let mut format = OutputFormat::Text;
            let mut input_json = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--json" => format = OutputFormat::Json,
                    "--text" => format = OutputFormat::Text,
                    "--input-json" => {
                        let Some(path) = args.next() else {
                            return Err(CliError::MissingFlagValue("--input-json"));
                        };
                        input_json = Some(PathBuf::from(path));
                    }
                    other => return Err(CliError::UnknownFlag(other.to_owned())),
                }
            }
            Ok(DoctorCommand::ProposalRender { format, input_json })
        }
        Some("proposal-materialize") => {
            let (format, input_json) = parse_proposal_flags(args)?;
            Ok(DoctorCommand::ProposalMaterialize { format, input_json })
        }
        Some("proposal-roundtrip") => {
            let (format, input_json) = parse_proposal_flags(args)?;
            Ok(DoctorCommand::ProposalRoundtrip { format, input_json })
        }
        Some("-h" | "--help" | "help") => Ok(DoctorCommand::Help),
        Some(other) => Err(CliError::UnknownDoctor(other.to_owned())),
    }
}

fn parse_proposal_flags(
    mut args: impl Iterator<Item = String>,
) -> Result<(OutputFormat, Option<PathBuf>), CliError> {
    let mut format = OutputFormat::Text;
    let mut input_json = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => format = OutputFormat::Json,
            "--text" => format = OutputFormat::Text,
            "--input-json" => {
                let Some(path) = args.next() else {
                    return Err(CliError::MissingFlagValue("--input-json"));
                };
                input_json = Some(PathBuf::from(path));
            }
            other => return Err(CliError::UnknownFlag(other.to_owned())),
        }
    }
    Ok((format, input_json))
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("unknown command `{0}`\n\n{HELP}")]
    UnknownCommand(String),
    #[error("unknown doctor command `{0}`\n\n{HELP}")]
    UnknownDoctor(String),
    #[error("unknown flag `{0}`\n\n{HELP}")]
    UnknownFlag(String),
    #[error("{0} requires a path\n\n{HELP}")]
    MissingFlagValue(&'static str),
    #[error(transparent)]
    Doctor(#[from] doctor::DoctorError),
}

const HELP: &str = "\
Usage:
  leaven doctor
  leaven doctor proposal-render [--text|--json] [--input-json PATH]
  leaven doctor proposal-materialize [--text|--json] [--input-json PATH]
  leaven doctor proposal-roundtrip [--text|--json] [--input-json PATH]

Doctor commands:
  proposal-render   Render the agent-facing proposal-stage input for the GEPA skill-bank slice.
  proposal-materialize   Materialize the current SkillBank input into a temp workspace and report files.
  proposal-roundtrip   Simulate one workspace edit, parse it, and apply it through RunContext.
";

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn doctor_summary_names_proposal_render() {
        let output = run(["doctor"].into_iter().map(str::to_owned)).unwrap();

        assert!(output.contains("proposal-render"));
        assert!(output.contains("proposal-materialize"));
        assert!(output.contains("proposal-roundtrip"));
        assert!(output.contains("render-only"));
    }

    #[test]
    fn proposal_render_text_includes_artifact_and_reflection_examples() {
        let output = run(["doctor", "proposal-render"].into_iter().map(str::to_owned)).unwrap();

        assert!(output.contains("Stage: gepa.reflect.proposal"));
        assert!(output.contains(".agents/skills/rust-test-debugging/SKILL.md"));
        assert!(output.contains("## Feedback"));
        assert!(output.contains("No agent session is executed"));
    }

    #[test]
    fn proposal_render_json_is_machine_readable() {
        let output = run(["doctor", "proposal-render", "--json"]
            .into_iter()
            .map(str::to_owned))
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["stage"], "gepa.reflect.proposal");
        assert_eq!(value["proof"], "render_only");
        assert!(
            value["agent_request"]["instructions"]["task"]
                .as_str()
                .unwrap()
                .contains("## Feedback")
        );
    }

    #[test]
    fn proposal_render_reads_input_json() {
        let mut input = crate::fixture::fixture_reflection_input();
        input.part_label = "custom/SKILL.md".to_owned();
        let path =
            std::env::temp_dir().join(format!("leaven-doctor-input-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();

        let output = run([
            "doctor".to_owned(),
            "proposal-render".to_owned(),
            "--json".to_owned(),
            "--input-json".to_owned(),
            path.display().to_string(),
        ])
        .unwrap();
        let _ = std::fs::remove_file(&path);
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["part"], "custom/SKILL.md");
    }

    #[test]
    fn proposal_materialize_reports_workspace_files() {
        let output = run(["doctor", "proposal-materialize", "--json"]
            .into_iter()
            .map(str::to_owned))
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["stage"], "gepa.reflect.materialize");
        assert_eq!(
            value["files"][0]["path"],
            ".agents/skills/rust-test-debugging/SKILL.md"
        );
    }

    #[test]
    fn proposal_roundtrip_applies_simulated_workspace_edit() {
        let output = run(["doctor", "proposal-roundtrip", "--json"]
            .into_iter()
            .map(str::to_owned))
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["stage"], "gepa.reflect.roundtrip");
        assert_eq!(value["proof"], "simulated_agent_apply");
        assert_eq!(value["proposal_count"], 1);
        assert!(value["child"].as_str().unwrap().len() > 10);
    }
}
