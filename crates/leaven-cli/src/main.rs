mod doctor;
mod fixture;
mod seam;
mod serve;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, error::ErrorKind};
use doctor::{DoctorCommand, OutputFormat};
use seam::{SeamProfileCommand, SeamServeCommand};
use serve::ServeCommand;

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
    match cli.command {
        None => Ok(DoctorCommand::Summary.run()?),
        Some(TopCommand::Doctor { command }) => Ok(doctor_command(command).run()?),
        Some(TopCommand::Run { command }) => run_command(command),
        Some(TopCommand::Seam { command }) => seam_command(command),
        Some(TopCommand::Serve(args)) => Ok(args.into_command().run()?),
    }
}

#[derive(Debug, Parser)]
#[command(name = "leaven", about = "Leaven operator diagnostics.")]
struct Cli {
    #[command(subcommand)]
    command: Option<TopCommand>,
}

#[derive(Debug, Subcommand)]
enum TopCommand {
    /// Run local Leaven diagnostics.
    Doctor {
        #[command(subcommand)]
        command: Option<DoctorSubcommand>,
    },
    /// Run the public seam server.
    Seam {
        #[command(subcommand)]
        command: SeamSubcommand,
    },
    /// Inspect completed Leaven runs.
    Run {
        #[command(subcommand)]
        command: RunSubcommand,
    },
    /// Run the engine as the ACP client over inherited stdio (the wire the SDK spawns).
    Serve(ServeArgs),
}

#[derive(Debug, Subcommand)]
enum SeamSubcommand {
    /// Export the locked Leaven public seam profile as JSON.
    Profile(SeamProfileArgs),
    /// Serve the locked Leaven public seam over inherited stdio.
    Serve(SeamServeArgs),
}

#[derive(Debug, Subcommand)]
enum RunSubcommand {
    /// Export Rust-owned inspection JSON from a local run directory.
    Inspect(RunInspectArgs),
    /// Export Rust-owned blob bytes from a local run directory.
    Blob(RunBlobArgs),
    /// Export Rust-owned evidence bytes from a local run directory.
    Evidence(RunEvidenceArgs),
}

#[derive(Debug, Args)]
struct RunInspectArgs {
    /// Local Leaven run directory to inspect.
    #[arg(long)]
    run_dir: PathBuf,
}

#[derive(Debug, Args)]
struct RunBlobArgs {
    /// Local Leaven run directory containing the blob store.
    #[arg(long)]
    run_dir: PathBuf,
    /// Blob store name from a Rust-owned readback reference.
    #[arg(long)]
    store: String,
    /// Blob key from a Rust-owned readback reference.
    #[arg(long)]
    key: String,
}

#[derive(Debug, Args)]
struct RunEvidenceArgs {
    /// Local Leaven run directory containing the evidence store.
    #[arg(long)]
    run_dir: PathBuf,
    /// Evidence store name from a Rust-owned readback reference.
    #[arg(long)]
    store: String,
    /// Evidence key from a Rust-owned readback reference.
    #[arg(long)]
    key: String,
}

#[derive(Debug, Args)]
struct SeamProfileArgs {
    /// Repo root the locked public-seam package loads from.
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

impl SeamProfileArgs {
    fn into_command(self) -> SeamProfileCommand {
        SeamProfileCommand { root: self.root }
    }
}

#[derive(Debug, Args)]
struct SeamServeArgs {
    /// Run the public seam server over this process's own stdin/stdout.
    #[arg(long, required = true)]
    stdio: bool,
    /// Repo root the locked public-seam package loads from.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// JSON config for executable seam providers.
    #[arg(long)]
    config: Option<PathBuf>,
}

impl SeamServeArgs {
    fn into_command(self) -> SeamServeCommand {
        // `--stdio` is the only supported transport mode; clap requires it.
        debug_assert!(self.stdio, "clap requires --stdio");
        SeamServeCommand {
            root: self.root,
            config: self.config,
        }
    }
}

#[derive(Debug, Args)]
struct ServeArgs {
    /// Run the bidirectional client loop over this process's own stdin/stdout.
    #[arg(long, required = true)]
    stdio: bool,
    /// Repo root the locked public-seam package loads from.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// The optimize plan file (seed + cases + loop config + named reward/reflect).
    #[arg(long)]
    plan: PathBuf,
    /// Where the `Optimized` result JSON is written (stdout is the JSON-RPC channel).
    #[arg(long)]
    out: PathBuf,
}

impl ServeArgs {
    fn into_command(self) -> ServeCommand {
        // `--stdio` is the only supported transport mode; clap requires it.
        debug_assert!(self.stdio, "clap requires --stdio");
        ServeCommand {
            root: self.root,
            plan: self.plan,
            out: self.out,
        }
    }
}

#[derive(Debug, Subcommand)]
enum DoctorSubcommand {
    /// Render the agent-facing proposal-stage input for the GEPA skill-bank slice.
    #[command(name = "proposal-render")]
    Render(ProposalDoctorArgs),
    /// Materialize the current `SkillBank` input into a temp workspace and report files.
    #[command(name = "proposal-materialize")]
    Materialize(ProposalDoctorArgs),
    /// Simulate one workspace edit, parse it, and apply it through `RunContext`.
    #[command(name = "proposal-roundtrip")]
    Roundtrip(ProposalDoctorArgs),
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
        Some(DoctorSubcommand::Render(args)) => DoctorCommand::ProposalRender {
            format: args.format(),
            input_json: args.input_json,
        },
        Some(DoctorSubcommand::Materialize(args)) => DoctorCommand::ProposalMaterialize {
            format: args.format(),
            input_json: args.input_json,
        },
        Some(DoctorSubcommand::Roundtrip(args)) => DoctorCommand::ProposalRoundtrip {
            format: args.format(),
            input_json: args.input_json,
        },
    }
}

fn seam_command(command: SeamSubcommand) -> Result<String, CliError> {
    match command {
        SeamSubcommand::Profile(args) => Ok(args.into_command().run()?),
        SeamSubcommand::Serve(args) => Ok(args.into_command().run()?),
    }
}

fn run_command(command: RunSubcommand) -> Result<String, CliError> {
    match command {
        RunSubcommand::Inspect(args) => {
            let export = leaven_run::export_local_run_inspection(args.run_dir)?;
            serde_json::to_string_pretty(&export)
                .map(|mut output| {
                    output.push('\n');
                    output
                })
                .map_err(CliError::from)
        }
        RunSubcommand::Blob(args) => {
            let export = leaven_run::export_local_run_blob(args.run_dir, args.store, args.key)?;
            serde_json::to_string_pretty(&export)
                .map(|mut output| {
                    output.push('\n');
                    output
                })
                .map_err(CliError::from)
        }
        RunSubcommand::Evidence(args) => {
            let export = leaven_run::export_local_run_evidence(args.run_dir, args.store, args.key)?;
            serde_json::to_string_pretty(&export)
                .map(|mut output| {
                    output.push('\n');
                    output
                })
                .map_err(CliError::from)
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Parse(#[from] clap::Error),
    #[error(transparent)]
    Doctor(#[from] doctor::DoctorError),
    #[error(transparent)]
    Seam(#[from] seam::SeamCommandError),
    #[error(transparent)]
    RunInspection(#[from] leaven_run::RunInspectionExportError),
    #[error(transparent)]
    Serve(#[from] serve::ServeError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use bytes::Bytes;
    use leaven_artifact_skill::{SkillBank, SkillFile, SkillFolder, SkillName, SkillPath};
    use leaven_engine::{
        GraphSnapshotRef, RunCheckpoint, StageJournalSnapshot, StateFormat,
        WorkspaceJournalSnapshot,
    };
    use leaven_gepa_agentic_skill::SkillBankReflectionInput;
    use leaven_kernel::{BlobRef, BudgetSnapshot, EvidenceRef, Fingerprint, now};
    use leaven_store::{BlobStore, BlobWrite, CheckpointBytes, CheckpointStore, EvidenceStore};
    use leaven_store_file::{FileEvidenceStore, FileStore};
    use serde::{Deserialize, Serialize};

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
        let path = write_input_json(&input);

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
        let path = write_input_json(&input);

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

    #[test]
    fn run_inspect_exports_rust_owned_checkpoint_and_graph_blob_readback() {
        let run_dir =
            std::env::temp_dir().join(format!("leaven-cli-run-inspect-{}", uuid::Uuid::new_v4()));
        let store = FileStore::open(&run_dir).unwrap();
        let graph_blob = BlobStore::put(
            &store,
            BlobWrite {
                bytes: Bytes::from_static(
                    br#"{"run_id":"run_cli","candidates":[{}],"proposal_batches":[],"proposals":[],"apply_attempts":[],"evaluation_requests":[],"assessments":[],"events":[{}]}"#,
                ),
                content_type: Some("application/json".to_owned()),
            },
        )
        .unwrap();
        let mut checkpoint = RunCheckpoint::new(
            leaven_kernel::RunId::new(),
            now(),
            GraphSnapshotRef {
                schema: Fingerprint::from_bytes([6; 32]),
                format: StateFormat::Json,
                bytes: graph_blob,
            },
            BudgetSnapshot::default(),
        );
        checkpoint.artifact_refs.push(BlobRef {
            store: "file".to_owned(),
            key: "artifact.blob".to_owned(),
        });
        checkpoint.evidence_refs.push(EvidenceRef {
            store: "evidence".to_owned(),
            key: "evidence.json".to_owned(),
        });
        checkpoint.stage_journal = StageJournalSnapshot {
            entries: vec![BlobRef {
                store: "file".to_owned(),
                key: "stage.blob".to_owned(),
            }],
        };
        checkpoint.workspace_journal = WorkspaceJournalSnapshot {
            entries: vec![BlobRef {
                store: "file".to_owned(),
                key: "workspace.blob".to_owned(),
            }],
        };
        let checkpoint_id = CheckpointStore::put(
            &store,
            CheckpointBytes(Bytes::from(serde_json::to_vec(&checkpoint).unwrap())),
        )
        .unwrap();
        CheckpointStore::mark_latest(&store, checkpoint_id).unwrap();

        let output = run([
            "run".to_owned(),
            "inspect".to_owned(),
            "--run-dir".to_owned(),
            run_dir.display().to_string(),
        ])
        .unwrap();
        let _ = std::fs::remove_dir_all(&run_dir);
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["schema_version"], "leaven.run_inspection_export.v1");
        assert_eq!(value["latest_checkpoint"], checkpoint_id.to_string());
        assert_eq!(
            value["checkpoint"]["artifact_refs"][0]["key"],
            "artifact.blob"
        );
        assert_eq!(
            value["checkpoint"]["evidence_refs"][0]["key"],
            "evidence.json"
        );
        assert_eq!(
            value["checkpoint"]["stage_journal_refs"][0]["key"],
            "stage.blob"
        );
        assert_eq!(
            value["checkpoint"]["workspace_journal_refs"][0]["key"],
            "workspace.blob"
        );
        assert_eq!(value["graph"]["run_id"], "run_cli");
        assert_eq!(value["graph"]["candidate_count"], 1);
        assert_eq!(value["graph"]["event_count"], 1);
        assert!(value["graph"]["bytes"].as_u64().unwrap() > 0);
    }

    #[test]
    fn run_blob_exports_rust_owned_blob_bytes() {
        let run_dir =
            std::env::temp_dir().join(format!("leaven-cli-run-blob-{}", uuid::Uuid::new_v4()));
        let store = FileStore::open(&run_dir).unwrap();
        let blob = BlobStore::put(
            &store,
            BlobWrite {
                bytes: Bytes::from_static(b"cli blob bytes\n"),
                content_type: Some("text/plain".to_owned()),
            },
        )
        .unwrap();

        let output = run([
            "run".to_owned(),
            "blob".to_owned(),
            "--run-dir".to_owned(),
            run_dir.display().to_string(),
            "--store".to_owned(),
            blob.store,
            "--key".to_owned(),
            blob.key,
        ])
        .unwrap();
        let _ = std::fs::remove_dir_all(&run_dir);
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["schema_version"], "leaven.run_blob_export.v1");
        assert_eq!(value["bytes"], "cli blob bytes\n".len());
        assert_eq!(value["content_base64"], "Y2xpIGJsb2IgYnl0ZXMK");
        assert_eq!(
            value["sha256"],
            "cfd235c82802e972fe7fb349eda659b4a7cdbfae9f556b1be8a3ab8998928120"
        );
    }

    #[test]
    fn run_evidence_exports_rust_owned_evidence_bytes() {
        let run_dir =
            std::env::temp_dir().join(format!("leaven-cli-run-evidence-{}", uuid::Uuid::new_v4()));
        let evidence_store =
            FileEvidenceStore::<CliTestEvidence>::open("leaven-run", run_dir.join("evidence"))
                .unwrap();
        let evidence_ref = evidence_store
            .put(CliTestEvidence {
                case_id: "case_cli".to_owned(),
                candidate_id: "cand_cli".to_owned(),
                score: 1.0,
            })
            .unwrap();

        let output = run([
            "run".to_owned(),
            "evidence".to_owned(),
            "--run-dir".to_owned(),
            run_dir.display().to_string(),
            "--store".to_owned(),
            evidence_ref.store,
            "--key".to_owned(),
            evidence_ref.key,
        ])
        .unwrap();
        let _ = std::fs::remove_dir_all(&run_dir);
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["schema_version"], "leaven.run_evidence_export.v1");
        assert_eq!(value["bytes"], 73);
        assert_eq!(
            value["sha256"],
            "99724bc98aa63966b8da9f30b42afe4126073364c38c123a897ea1ebf2812c42"
        );
        assert_eq!(
            value["content_base64"],
            "ewogICJjYXNlX2lkIjogImNhc2VfY2xpIiwKICAiY2FuZGlkYXRlX2lkIjogImNhbmRfY2xpIiwKICAic2NvcmUiOiAxLjAKfQ=="
        );
    }

    fn write_input_json(input: &SkillBankReflectionInput<String>) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("leaven-doctor-input-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, serde_json::to_vec(input).unwrap()).unwrap();
        path
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct CliTestEvidence {
        case_id: String,
        candidate_id: String,
        score: f64,
    }

    impl leaven_store::Evidence for CliTestEvidence {}
}
