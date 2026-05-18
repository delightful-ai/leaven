mod doctor;
mod fixture;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, error::ErrorKind};
use doctor::{DoctorCommand, OutputFormat};

fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(CliError::Parse(error))
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            print!("{error}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<String, CliError> {
    let cli = Cli::try_parse_from(std::iter::once("leaven".to_owned()).chain(args))
        .map_err(CliError::Parse)?;
    cli.into_doctor_command().run().map_err(CliError::Doctor)
}

#[derive(Debug, Parser)]
#[command(name = "leaven", about = "Leaven operator diagnostics.")]
struct Cli {
    #[command(subcommand)]
    command: Option<TopCommand>,
}

impl Cli {
    fn into_doctor_command(self) -> DoctorCommand {
        match self.command {
            None => DoctorCommand::Summary,
            Some(TopCommand::Doctor { command }) => doctor_command(command),
        }
    }
}

#[derive(Debug, Subcommand)]
enum TopCommand {
    /// Run local Leaven diagnostics.
    Doctor {
        #[command(subcommand)]
        command: Option<DoctorSubcommand>,
    },
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Subcommand)]
enum DoctorSubcommand {
    /// Render the agent-facing proposal-stage input for the GEPA skill-bank slice.
    ProposalRender(ProposalDoctorArgs),
    /// Materialize the current `SkillBank` input into a temp workspace and report files.
    ProposalMaterialize(ProposalDoctorArgs),
    /// Simulate one workspace edit, parse it, and apply it through `RunContext`.
    ProposalRoundtrip(ProposalDoctorArgs),
}

#[derive(Debug, Args)]
struct ProposalDoctorArgs {
    /// Emit JSON instead of text.
    #[arg(long, conflicts_with = "text")]
    json: bool,
    /// Emit text output.
    #[arg(long)]
    text: bool,
    /// Serialized `SkillBankReflectionInput<String>` debug/run state.
    #[arg(long)]
    input_json: Option<PathBuf>,
}

impl ProposalDoctorArgs {
    fn format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Text
        }
    }
}

fn doctor_command(command: Option<DoctorSubcommand>) -> DoctorCommand {
    match command {
        None => DoctorCommand::Summary,
        Some(DoctorSubcommand::ProposalRender(args)) => DoctorCommand::ProposalRender {
            format: args.format(),
            input_json: args.input_json,
        },
        Some(DoctorSubcommand::ProposalMaterialize(args)) => DoctorCommand::ProposalMaterialize {
            format: args.format(),
            input_json: args.input_json,
        },
        Some(DoctorSubcommand::ProposalRoundtrip(args)) => DoctorCommand::ProposalRoundtrip {
            format: args.format(),
            input_json: args.input_json,
        },
    }
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Parse(#[from] clap::Error),
    #[error(transparent)]
    Doctor(#[from] doctor::DoctorError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use leaven_artifact_skill::{SkillBank, SkillFile, SkillFolder, SkillName, SkillPath};

    use super::run;

    #[test]
    fn doctor_summary_names_proposal_render() {
        let output = run(std::iter::once("doctor").map(str::to_owned)).unwrap();

        assert!(output.contains("proposal-render"));
        assert!(output.contains("proposal-materialize"));
        assert!(output.contains("proposal-roundtrip"));
        assert!(output.contains("render-only"));
    }

    #[test]
    fn proposal_render_text_includes_artifact_and_reflection_examples() {
        let output = run(["doctor", "proposal-render"].into_iter().map(str::to_owned)).unwrap();

        assert!(output.contains("Stage: gepa.reflect.proposal"));
        assert!(output.contains("target/current"));
        assert!(output.contains("ReflectionWorkspace"));
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
                .contains("target/current")
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
            "target/current/.agents/skills/rust-test-debugging/SKILL.md"
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

    #[test]
    fn proposal_roundtrip_edits_requested_skill_part() {
        let mut input = crate::fixture::fixture_reflection_input();
        let requested = SkillName::new("custom").unwrap();
        let original = input
            .artifact
            .get(&SkillName::new("rust-test-debugging").unwrap())
            .unwrap()
            .clone();
        let mut custom_entries = BTreeMap::new();
        custom_entries.insert(
            SkillPath::skill_md(),
            SkillFile::text(
                "---\nname: custom\ndescription: Custom skill. Use when custom testing.\n---\nCustom body.\n",
            ),
        );
        input.artifact = SkillBank::from_folders([
            original,
            SkillFolder::from_entries(requested, custom_entries).unwrap(),
        ])
        .unwrap();
        input.part = "custom/SKILL.md".to_owned();
        input.part_label = "custom/SKILL.md".to_owned();
        let path =
            std::env::temp_dir().join(format!("leaven-doctor-input-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();

        let output = run([
            "doctor".to_owned(),
            "proposal-roundtrip".to_owned(),
            "--json".to_owned(),
            "--input-json".to_owned(),
            path.display().to_string(),
        ])
        .unwrap();
        let _ = std::fs::remove_file(&path);
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(
            value["edited_path"],
            "target/current/.agents/skills/custom/SKILL.md"
        );
    }

    #[test]
    fn clap_rejects_conflicting_output_flags() {
        let error = run(["doctor", "proposal-render", "--json", "--text"]
            .into_iter()
            .map(str::to_owned))
        .unwrap_err()
        .to_string();

        assert!(error.contains("cannot be used with"));
    }
}
