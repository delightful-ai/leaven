mod doctor;
mod fixture;

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
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--json" => format = OutputFormat::Json,
                    "--text" => format = OutputFormat::Text,
                    other => return Err(CliError::UnknownFlag(other.to_owned())),
                }
            }
            Ok(DoctorCommand::ProposalRender { format })
        }
        Some("-h" | "--help" | "help") => Ok(DoctorCommand::Help),
        Some(other) => Err(CliError::UnknownDoctor(other.to_owned())),
    }
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("unknown command `{0}`\n\n{HELP}")]
    UnknownCommand(String),
    #[error("unknown doctor command `{0}`\n\n{HELP}")]
    UnknownDoctor(String),
    #[error("unknown flag `{0}`\n\n{HELP}")]
    UnknownFlag(String),
    #[error(transparent)]
    Doctor(#[from] doctor::DoctorError),
}

const HELP: &str = "\
Usage:
  leaven doctor
  leaven doctor proposal-render [--text|--json]

Doctor commands:
  proposal-render   Render the agent-facing proposal-stage input for the GEPA skill-bank slice.
";

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn doctor_summary_names_proposal_render() {
        let output = run(["doctor"].into_iter().map(str::to_owned)).unwrap();

        assert!(output.contains("proposal-render"));
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
}
